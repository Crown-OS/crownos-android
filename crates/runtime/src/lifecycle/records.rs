use std::time::{Duration, Instant};

use agent_wire::{Record, WIRE_VERSION};

use super::idle::IdleStep;
use super::phase::GsfWait;
use super::{AfterStop, BootStage, Effect, Lifecycle, Link, Milestone, Phase, Run};

/// The warm-up waits this long for Google services to get an id.
const GSF_WAIT: Duration = Duration::from_secs(60);
const GSF_QUERY_INTERVAL: Duration = Duration::from_secs(10);

impl Lifecycle {
    /// One record from the agent.
    pub fn agent_record(&mut self, record: Record, now: Instant) {
        match record {
            Record::Hello { wire, bundle } => self.hello(wire, &bundle),
            Record::Ready { boot_ms } => self.ready(boot_ms, now),
            Record::TaskOpened { task, package } => {
                self.tasks.insert(task, Some(package));
                self.idle.disarm();
                self.effects.push(Effect::StatusChanged);
            }
            Record::TaskClosed { task } => {
                self.tasks.remove(&task);
                self.windows_changed(now);
            }
            Record::Tasks { tasks } => {
                self.tasks.retain(|task, _| tasks.contains(task));
                for task in tasks {
                    self.tasks.entry(task).or_insert(None);
                }
                self.windows_changed(now);
            }
            Record::KeepAlive { reasons } => {
                if self.idle.keep_alive(reasons.any(), now) == IdleStep::Stop {
                    log::info!("nothing keeps Android awake, stopping");
                    self.begin_stop(AfterStop::Rest);
                } else {
                    self.effects.push(Effect::StatusChanged);
                }
            }
            Record::GsfId { id } => self.gsf_id(id),
            Record::LaunchFailed { package, reason } => {
                self.effects.push(Effect::FailSplash { package, reason });
            }
            Record::InstallResult { path, ok, message } => {
                self.report(Milestone::InstallFinished { path, ok, message });
            }
            other => log::warn!("ignoring agent record {:#06x}", other.kind()),
        }
    }

    fn windows_changed(&mut self, now: Instant) {
        if self.tasks.is_empty() {
            if !self.idle.is_armed() {
                self.idle.arm(now);
            }
        } else {
            self.idle.disarm();
        }
        self.effects.push(Effect::StatusChanged);
    }

    fn hello(&mut self, wire: u32, bundle: &str) {
        let handshaking = match &self.phase {
            Phase::Booting(boot) => boot.stage == BootStage::Handshake,
            Phase::Running(run) => run.link == Link::Handshake,
            _ => false,
        };
        if !handshaking {
            return;
        }
        if wire != WIRE_VERSION {
            self.effects.push(Effect::DropAgent);
            self.trigger_safe_mode(&format!(
                "crown-agent speaks wire version {wire}, not {WIRE_VERSION}."
            ));
            return;
        }
        let expected = self
            .installed()
            .and_then(|installed| installed.bundle.as_deref());
        if !self.in_safe_mode() && expected != Some(bundle) {
            let reason = format!(
                "Android loaded bundle {bundle:?} instead of {:?}.",
                expected.unwrap_or_default()
            );
            self.trigger_safe_mode(&reason);
            return;
        }
        match &mut self.phase {
            Phase::Booting(boot) => boot.stage = BootStage::Ready,
            Phase::Running(run) => {
                run.link = Link::AwaitReady;
                self.effects.push(Effect::SendAgent(Record::ListTasks));
            }
            _ => {}
        }
    }

    fn ready(&mut self, boot_ms: u32, now: Instant) {
        match &self.phase {
            Phase::Booting(boot) if boot.stage == BootStage::Ready => {
                log::info!("Android is ready after {boot_ms} ms");
                let safe_mode = boot.safe_mode;
                self.boot_succeeded();
                self.phase = Phase::Running(Run {
                    safe_mode,
                    link: Link::Up,
                    gsf_wait: None,
                    served_errands: false,
                });
                self.effects.push(Effect::HoldSleepInhibitor);
                self.effects.push(Effect::SendAgent(Record::ListTasks));
                self.idle.arm(now);
                self.perform_errands();
                if self.is_warming_up() {
                    self.warm_up_ready(now);
                }
                self.effects.push(Effect::StatusChanged);
            }
            Phase::Running(run) if run.link == Link::AwaitReady => {
                if let Phase::Running(run) = &mut self.phase {
                    run.link = Link::Up;
                }
                self.idle.arm_if_idle(self.tasks.is_empty(), now);
                self.perform_errands();
                self.effects.push(Effect::StatusChanged);
            }
            _ => {}
        }
    }

    fn boot_succeeded(&mut self) {
        self.persisted.failed_boots = 0;
        let installed_bundle = self
            .installed()
            .and_then(|installed| installed.bundle.clone());
        if self
            .persisted
            .safe_mode_bundle
            .as_ref()
            .is_some_and(|bundle| Some(bundle) != installed_bundle.as_ref())
        {
            log::info!("a new bundle is installed, leaving safe mode at the next boot");
            self.persisted.safe_mode_bundle = None;
            self.effects.push(Effect::SetSafeMode(false));
        }
        self.persist();
    }

    fn perform_errands(&mut self) {
        if self.errands.is_empty() {
            return;
        }
        if let Phase::Running(run) = &mut self.phase {
            run.served_errands = true;
        }
        let errands = std::mem::take(&mut self.errands);
        self.effects
            .extend(errands.into_iter().map(Effect::Perform));
    }

    fn warm_up_ready(&mut self, now: Instant) {
        let wants_gsf = self
            .installed()
            .is_some_and(|installed| installed.google_play)
            && self.persisted.gsf_id.is_none();
        if !wants_gsf {
            self.finish_warm_up();
            return;
        }
        if let Phase::Running(run) = &mut self.phase {
            run.gsf_wait = Some(GsfWait {
                until: now + GSF_WAIT,
                next_query: now + GSF_QUERY_INTERVAL,
            });
        }
        self.effects.push(Effect::SendAgent(Record::QueryGsfId));
    }

    pub(super) fn poll_gsf_wait(&mut self, now: Instant) {
        let Phase::Running(run) = &mut self.phase else {
            return;
        };
        let Some(wait) = &mut run.gsf_wait else {
            return;
        };
        if now >= wait.until {
            run.gsf_wait = None;
            self.finish_warm_up();
        } else if now >= wait.next_query {
            wait.next_query = now + GSF_QUERY_INTERVAL;
            self.effects.push(Effect::SendAgent(Record::QueryGsfId));
        }
    }

    fn gsf_id(&mut self, id: String) {
        if id.is_empty() {
            return;
        }
        if self.persisted.gsf_id.as_deref() != Some(&id) {
            self.persisted.gsf_id = Some(id.clone());
            self.persist();
        }
        self.report(Milestone::GsfId { id });
        if let Phase::Running(run) = &mut self.phase
            && run.gsf_wait.take().is_some()
        {
            self.finish_warm_up();
        }
    }

    fn finish_warm_up(&mut self) {
        if !self.is_warming_up() {
            return;
        }
        self.persisted.warmed_up = true;
        self.persist();
        self.report(Milestone::WarmUpFinished);
        let served = matches!(&self.phase, Phase::Running(run) if run.served_errands);
        if self.tasks.is_empty() && !served {
            self.begin_stop(AfterStop::Rest);
        }
    }
}
