//! The runtime as a pure state machine. Every method takes the current time,
//! so tests drive it with a fake clock, and queues [`Effect`]s that the
//! daemon drains with [`Lifecycle::take_effects`].

mod effect;
mod errand;
mod failure;
mod idle;
mod phase;
mod records;
#[cfg(test)]
mod tests;

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use agent_wire::Record;
use android_protocol::{LaunchOutcome, RuntimeState, StopReason};

pub use effect::{Effect, Milestone};
pub use errand::{Errand, SETTINGS};
pub use idle::{KEEP_ALIVE_INTERVAL, KEEP_ALIVE_REPLY};

use crate::{Installation, Installed, Persisted};
use idle::{IdleStep, IdleTimer};
use phase::{AfterStop, Boot, BootStage, Link, Phase, Run};

/// Android must report `Ready` this long after a boot starts.
pub const READY_TIMEOUT: Duration = Duration::from_secs(120);
/// A running Android whose agent stays away this long is stopped.
pub const AGENT_RECONNECT_GRACE: Duration = Duration::from_secs(30);
/// Boots in a row without `Ready` that turn on safe mode.
pub const FAILED_BOOTS_FOR_SAFE_MODE: u32 = 3;

/// Why a host session could not start.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BootFailure {
    /// The container already runs a session of this uid.
    OtherUser {
        uid: u32,
    },
    Host(String),
}

pub struct Lifecycle {
    installation: Installation,
    phase: Phase,
    persisted: Persisted,
    idle: IdleTimer,
    /// Open tasks and their packages, when known.
    tasks: BTreeMap<u32, Option<String>>,
    errands: Vec<Errand>,
    effects: Vec<Effect>,
}

impl Lifecycle {
    /// A stopped runtime that has not looked at the installation yet.
    pub fn new(persisted: Persisted, idle_minutes: u32) -> Self {
        Self {
            installation: Installation::NotInstalled,
            phase: Phase::Stopped,
            persisted,
            idle: IdleTimer::new(idle_minutes),
            tasks: BTreeMap::new(),
            errands: Vec::new(),
            effects: Vec::new(),
        }
    }

    pub fn take_effects(&mut self) -> Vec<Effect> {
        std::mem::take(&mut self.effects)
    }

    pub fn state(&self) -> RuntimeState {
        match &self.phase {
            Phase::Booting(_) => RuntimeState::Starting,
            Phase::Running(run) if run.safe_mode => RuntimeState::SafeMode,
            Phase::Running(_) => RuntimeState::Running,
            Phase::Stopping(_) => RuntimeState::Stopping,
            Phase::Refused { .. } => RuntimeState::Unsupported,
            Phase::Stopped => match self.installation {
                Installation::NotInstalled => RuntimeState::NotInstalled,
                Installation::SettingUp => RuntimeState::SettingUp,
                Installation::Unsupported { .. } => RuntimeState::Unsupported,
                Installation::Installed(_) => RuntimeState::Stopped,
            },
        }
    }

    /// Why the runtime is unsupported, when it is.
    pub fn unsupported_reason(&self) -> Option<&str> {
        match (&self.phase, &self.installation) {
            (Phase::Refused { message }, _) => Some(message),
            (Phase::Stopped, Installation::Unsupported { detail }) => Some(detail),
            _ => None,
        }
    }

    pub fn windows(&self) -> u32 {
        u32::try_from(self.tasks.len()).unwrap_or(u32::MAX)
    }

    pub fn installed(&self) -> Option<&Installed> {
        self.installation.installed()
    }

    pub fn installation(&self) -> &Installation {
        &self.installation
    }

    pub fn persisted(&self) -> &Persisted {
        &self.persisted
    }

    pub fn has_task(&self, package: &str) -> bool {
        self.tasks
            .values()
            .any(|open| open.as_deref() == Some(package))
    }

    /// Whether Android, or the way to or from it, is up.
    pub fn is_active(&self) -> bool {
        matches!(
            self.phase,
            Phase::Booting(_) | Phase::Running(_) | Phase::Stopping(_)
        )
    }

    /// Whether the first boot after setup is still due.
    pub fn is_warming_up(&self) -> bool {
        self.installed().is_some() && !self.persisted.warmed_up
    }

    /// Whether the daemon should try to connect to the agent socket.
    pub fn wants_agent(&self) -> bool {
        match &self.phase {
            Phase::Booting(boot) => boot.stage == BootStage::Agent,
            Phase::Running(run) => matches!(run.link, Link::Lost { .. }),
            _ => false,
        }
    }

    /// When Android stops for being idle, if it is counting down.
    pub fn idle_deadline(&self) -> Option<Instant> {
        match &self.phase {
            Phase::Running(run) if run.link == Link::Up => self.idle.deadline(),
            _ => None,
        }
    }

    /// The next moment [`Self::tick`] has work.
    pub fn next_wakeup(&self) -> Option<Instant> {
        match &self.phase {
            Phase::Booting(boot) => Some(boot.deadline),
            Phase::Running(run) => {
                let lost = match run.link {
                    Link::Lost { since } => Some(since + AGENT_RECONNECT_GRACE),
                    Link::Up | Link::Handshake | Link::AwaitReady => None,
                };
                let gsf = run.gsf_wait.map(|wait| wait.next_query.min(wait.until));
                let idle = (run.link == Link::Up)
                    .then(|| self.idle.next_wakeup())
                    .flatten();
                [lost, gsf, idle].into_iter().flatten().min()
            }
            _ => None,
        }
    }

    pub fn set_installation(&mut self, installation: Installation, now: Instant) {
        if installation == self.installation {
            return;
        }
        let setting_up_again = installation == Installation::SettingUp;
        self.installation = installation;
        if setting_up_again && self.persisted.warmed_up {
            self.persisted.warmed_up = false;
            self.persist();
        }
        if self.installed().is_none() {
            self.errands.clear();
            if matches!(self.phase, Phase::Booting(_) | Phase::Running(_)) {
                self.begin_stop(AfterStop::Rest);
            } else if matches!(self.phase, Phase::Refused { .. }) {
                self.phase = Phase::Stopped;
            }
        } else if self.is_warming_up() && matches!(self.phase, Phase::Stopped) {
            self.begin_boot(now, false);
        }
        self.effects.push(Effect::StatusChanged);
    }

    /// Takes over an Android that runs without this daemon, for example after
    /// the daemon restarted, as a boot whose agent is about to connect.
    pub fn adopt(&mut self, now: Instant) {
        if !matches!(self.phase, Phase::Stopped) || self.installed().is_none() {
            return;
        }
        self.phase = Phase::Booting(Boot {
            deadline: now + READY_TIMEOUT,
            stage: BootStage::Agent,
            safe_mode: self.persisted.safe_mode_bundle.is_some(),
        });
        self.effects.push(Effect::StatusChanged);
    }

    /// Runs the first boot after setup again, after it failed.
    pub fn resume_warm_up(&mut self, now: Instant) {
        if self.is_warming_up() && matches!(self.phase, Phase::Stopped | Phase::Refused { .. }) {
            self.begin_boot(now, false);
        }
    }

    pub fn set_idle_minutes(&mut self, minutes: u32, now: Instant) {
        self.idle.set_minutes(minutes, now);
        self.effects.push(Effect::StatusChanged);
    }

    /// Accepts work for Android, starting it when it is stopped.
    pub fn submit(&mut self, errand: Errand, now: Instant) -> LaunchOutcome {
        match self.installation {
            Installation::NotInstalled => return LaunchOutcome::NotInstalled,
            Installation::SettingUp => return LaunchOutcome::SettingUp,
            Installation::Unsupported { .. } => return LaunchOutcome::Unsupported,
            Installation::Installed(_) => {}
        }
        if let Some(package) = errand
            .splash_package()
            .filter(|package| !self.has_task(package))
        {
            self.effects.push(Effect::ShowSplash {
                package: package.to_owned(),
            });
        }
        match &mut self.phase {
            Phase::Running(run) if run.link == Link::Up => {
                run.served_errands = true;
                if self.idle.is_armed() {
                    self.idle.arm(now);
                }
                self.effects.push(Effect::Perform(errand));
                LaunchOutcome::Opening
            }
            Phase::Running(_) => {
                self.errands.push(errand);
                LaunchOutcome::Opening
            }
            Phase::Booting(_) | Phase::Stopping(_) => {
                self.errands.push(errand);
                LaunchOutcome::Starting
            }
            Phase::Stopped | Phase::Refused { .. } => {
                self.errands.push(errand);
                self.begin_boot(now, false);
                LaunchOutcome::Starting
            }
        }
    }

    pub fn stop(&mut self, reason: StopReason) {
        if !matches!(self.phase, Phase::Booting(_) | Phase::Running(_)) {
            return;
        }
        log::info!("stopping Android: {reason:?}");
        self.fail_errands("Android was stopped");
        self.begin_stop(AfterStop::Rest);
    }

    /// The host worker started the container and the session unit.
    pub fn session_started(&mut self) {
        if let Phase::Booting(boot) = &mut self.phase
            && boot.stage == BootStage::Session
        {
            boot.stage = BootStage::Agent;
        }
    }

    pub fn session_failed(&mut self, failure: BootFailure) {
        if !matches!(self.phase, Phase::Booting(_)) {
            return;
        }
        match failure {
            BootFailure::OtherUser { uid } => {
                let message = format!("Android is in use by another user (uid {uid}).");
                self.fail_errands(&message);
                self.effects.push(Effect::AbandonSession);
                self.phase = Phase::Refused { message };
                self.effects.push(Effect::StatusChanged);
            }
            BootFailure::Host(reason) => self.boot_failed(reason),
        }
    }

    /// The host worker finished the stop sequence.
    pub fn stopped(&mut self, now: Instant) {
        let Phase::Stopping(after) = self.phase else {
            return;
        };
        self.phase = Phase::Stopped;
        match after {
            AfterStop::Restart { safe_mode } if self.installed().is_some() => {
                self.begin_boot(now, safe_mode);
            }
            _ if !self.errands.is_empty() && self.installed().is_some() => {
                self.begin_boot(now, false);
            }
            _ => self.errands.clear(),
        }
        self.effects.push(Effect::StatusChanged);
    }

    pub fn agent_connected(&mut self) {
        match &mut self.phase {
            Phase::Booting(boot) if boot.stage == BootStage::Agent => {
                boot.stage = BootStage::Handshake;
            }
            Phase::Running(run) if matches!(run.link, Link::Lost { .. }) => {
                run.link = Link::Handshake;
            }
            _ => return,
        }
        let bundle = self
            .installed()
            .and_then(|installed| installed.bundle.clone())
            .unwrap_or_default();
        self.effects.push(Effect::SendAgent(Record::Hello {
            wire: agent_wire::WIRE_VERSION,
            bundle,
        }));
    }

    pub fn agent_lost(&mut self, now: Instant) {
        match &mut self.phase {
            Phase::Booting(boot)
                if matches!(boot.stage, BootStage::Handshake | BootStage::Ready) =>
            {
                boot.stage = BootStage::Agent;
            }
            Phase::Running(run) if !matches!(run.link, Link::Lost { .. }) => {
                log::warn!("lost the agent connection");
                run.link = Link::Lost { since: now };
            }
            _ => {}
        }
    }

    pub fn prepare_for_sleep(&mut self, sleeping: bool) {
        let running = matches!(self.phase, Phase::Running(_));
        if sleeping {
            if running {
                self.effects.push(Effect::Freeze);
            }
            self.effects.push(Effect::ReleaseSleepInhibitor);
        } else if running {
            let venus = self.venus();
            self.effects.push(Effect::Thaw { venus });
            self.effects.push(Effect::HoldSleepInhibitor);
        }
    }

    pub fn tick(&mut self, now: Instant) {
        match &mut self.phase {
            Phase::Booting(boot) if now >= boot.deadline => {
                self.trigger_safe_mode("Android did not finish starting within two minutes.");
            }
            Phase::Running(run) => {
                if let Link::Lost { since } = run.link {
                    if now >= since + AGENT_RECONNECT_GRACE {
                        self.report(Milestone::BootFailed {
                            reason: "Android stopped responding.".into(),
                        });
                        self.fail_errands("Android stopped responding.");
                        self.begin_stop(AfterStop::Rest);
                    }
                    return;
                }
                if run.link != Link::Up {
                    return;
                }
                self.poll_gsf_wait(now);
                self.poll_idle(now);
            }
            _ => {}
        }
    }

    fn poll_idle(&mut self, now: Instant) {
        match self.idle.poll(now) {
            IdleStep::Wait => {}
            IdleStep::Query => self.send(Record::QueryKeepAlive),
            IdleStep::Stop => {
                log::info!("no Android window for the idle timeout, stopping");
                self.begin_stop(AfterStop::Rest);
            }
        }
    }

    fn begin_boot(&mut self, now: Instant, safe_mode: bool) {
        let safe_mode = safe_mode || self.persisted.safe_mode_bundle.is_some();
        self.phase = Phase::Booting(Boot {
            deadline: now + READY_TIMEOUT,
            stage: BootStage::Session,
            safe_mode,
        });
        if self.is_warming_up() {
            self.report(Milestone::WarmUpStarted);
        }
        let venus = self.venus();
        self.effects.push(Effect::StartSession { venus });
        self.effects.push(Effect::StatusChanged);
    }

    fn begin_stop(&mut self, after: AfterStop) {
        let link_up = match &self.phase {
            Phase::Running(run) => run.link == Link::Up,
            Phase::Booting(boot) => boot.stage == BootStage::Ready,
            _ => false,
        };
        if link_up {
            self.send(Record::Shutdown);
        }
        self.effects.push(Effect::DropAgent);
        self.effects.push(Effect::ReleaseSleepInhibitor);
        self.idle.disarm();
        self.tasks.clear();
        self.phase = Phase::Stopping(after);
        let venus = self.venus();
        self.effects.push(Effect::StopSession { venus });
        self.effects.push(Effect::StatusChanged);
    }

    fn venus(&self) -> bool {
        self.installed().is_some_and(Installed::venus)
    }

    fn in_safe_mode(&self) -> bool {
        let phase_safe = match &self.phase {
            Phase::Booting(boot) => boot.safe_mode,
            Phase::Running(run) => run.safe_mode,
            _ => false,
        };
        phase_safe || self.persisted.safe_mode_bundle.is_some()
    }

    fn fail_errands(&mut self, reason: &str) {
        for errand in std::mem::take(&mut self.errands) {
            match errand {
                Errand::Install { guest_path } => self.report(Milestone::InstallFinished {
                    path: guest_path,
                    ok: false,
                    message: reason.to_owned(),
                }),
                other => {
                    if let Some(package) = other.splash_package() {
                        self.effects.push(Effect::FailSplash {
                            package: package.to_owned(),
                            reason: reason.to_owned(),
                        });
                    }
                }
            }
        }
    }

    fn send(&mut self, record: Record) {
        self.effects.push(Effect::SendAgent(record));
    }

    fn report(&mut self, milestone: Milestone) {
        self.effects.push(Effect::Report(milestone));
    }

    fn persist(&mut self) {
        self.effects.push(Effect::Persist(self.persisted.clone()));
    }
}
