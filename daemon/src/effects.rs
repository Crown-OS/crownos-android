//! Carrying out what the lifecycle asks for.

use std::time::Instant;

use agent_wire::Record;
use crownos_ipc::Server;
use runtime::host::data_home;
use runtime::lifecycle::{BootFailure, Effect, Errand, Milestone};
use splash::{Scale, SplashRequest, geometry::MAX_LAUNCH};

use crate::daemon::Daemon;
use crate::host_worker::Job;
use crate::notify::SetupNotice;
use crate::notify::content::{install_card, safe_mode_card};
use crate::util::uuid;

/// The Android intent behind `crown-android settings`.
const APP_DETAILS: &str = "android.settings.APPLICATION_DETAILS_SETTINGS";

impl Daemon {
    /// Applies the lifecycle's effects until it has none left; applying one
    /// may produce more.
    pub fn settle(&mut self, server: &mut Server, now: Instant) {
        loop {
            let effects = self.lifecycle.take_effects();
            if effects.is_empty() {
                return;
            }
            for effect in effects {
                self.apply(server, effect, now);
            }
        }
    }

    fn apply(&mut self, server: &mut Server, effect: Effect, now: Instant) {
        match effect {
            Effect::StatusChanged => self.emit_status(server),
            Effect::ShowSplash { package } => self.show_splash(&package, now),
            Effect::FailSplash { package, reason } => {
                if let Some(splashes) = self.wayland.connected()
                    && let Err(error) = splashes.fail(&package, &reason, now)
                {
                    log::warn!("could not show why {package} failed: {error}");
                }
            }
            Effect::StartSession { venus } => match self.prepare_session(now) {
                Ok(()) => self.host.submit(Job::StartSession { venus }),
                Err(reason) => self.lifecycle.session_failed(BootFailure::Host(reason)),
            },
            Effect::StopSession { venus } => self.host.submit(Job::StopSession { venus }),
            Effect::AbandonSession => self.session = None,
            Effect::SendAgent(record) => self.send_agent(&record, now),
            Effect::DropAgent => self.agent = None,
            Effect::Perform(errand) => self.perform(errand, now),
            Effect::SetSafeMode(enabled) => self.host.submit(Job::SetSafeMode(enabled)),
            Effect::Freeze => self.host.submit(Job::Freeze),
            Effect::Thaw { venus } => self.host.submit(Job::Thaw { venus }),
            Effect::HoldSleepInhibitor => self.host.submit(Job::HoldSleepInhibitor),
            Effect::ReleaseSleepInhibitor => self.host.submit(Job::ReleaseSleepInhibitor),
            Effect::Persist(persisted) => {
                if let Err(error) = persisted.store(&self.env.paths.state_file()) {
                    log::warn!("{error}");
                }
            }
            Effect::Report(milestone) => self.report(server, milestone),
        }
    }

    /// The data home and the security context, before the host worker
    /// starts the session unit that uses both.
    fn prepare_session(&mut self, now: Instant) -> Result<(), String> {
        let paths = &self.env.paths;
        data_home::prepare(paths).map_err(|error| {
            format!(
                "could not prepare {}: {error}",
                paths.session_data_home().display()
            )
        })?;
        let instance = uuid::random_v4().map_err(|error| error.to_string())?;
        let socket = paths.wayland_socket();
        let splashes = self
            .wayland
            .get(now)
            .ok_or("there is no Wayland compositor to show Android on")?;
        let context = splashes
            .listen_security_context(&socket, &instance)
            .map_err(|error| format!("could not create the Wayland socket for Android: {error}"))?;
        log::info!(
            "Waydroid session {instance} on {}",
            context.socket().display()
        );
        self.session = Some(context);
        Ok(())
    }

    fn show_splash(&mut self, package: &str, now: Instant) {
        let info = desktop_entries::app_info(&self.env.paths.waydroid_applications(), package);
        let title = info.as_ref().map_or(package, |info| info.name.as_str());
        let request = SplashRequest {
            package,
            title,
            icon: info.as_ref().and_then(|info| info.icon.as_deref()),
        };
        let Some(splashes) = self.wayland.get(now) else {
            return;
        };
        if let Err(error) = splashes.open(&request, now) {
            log::warn!("could not open the splash of {package}: {error}");
        }
    }

    pub fn send_agent(&mut self, record: &Record, now: Instant) {
        let Some(link) = &mut self.agent else {
            log::debug!("no agent for {:#06x}", record.kind());
            return;
        };
        if let Err(error) = link.send(record) {
            log::warn!("{error}");
            self.agent = None;
            self.lifecycle.agent_lost(now);
        }
    }

    fn perform(&mut self, errand: Errand, now: Instant) {
        let record = match errand {
            Errand::Launch { package, token } => {
                let size = self.wayland.connection().map_or_else(
                    || MAX_LAUNCH.to_physical(Scale::ONE),
                    splash::Splashes::launch_pixels,
                );
                Record::Launch {
                    package,
                    width: size.width,
                    height: size.height,
                    token: token.unwrap_or_default(),
                }
            }
            Errand::OpenUri { uri } => Record::OpenUri { uri },
            Errand::Install { guest_path } => Record::InstallPackage { path: guest_path },
            Errand::AppSettings { package } => {
                self.host.submit(Job::LaunchIntent {
                    action: APP_DETAILS,
                    uri: format!("package:{package}"),
                });
                return;
            }
        };
        self.send_agent(&record, now);
    }

    fn report(&mut self, server: &mut Server, milestone: Milestone) {
        match milestone {
            Milestone::WarmUpStarted => self.notifications.show_setup(SetupNotice::Finishing),
            Milestone::WarmUpFinished => {
                let google_play = self
                    .lifecycle
                    .installed()
                    .is_some_and(|installed| installed.google_play);
                self.notifications
                    .show_setup(SetupNotice::Ready { google_play });
            }
            Milestone::WarmUpFailed { reason } => {
                self.notifications
                    .show_setup(SetupNotice::WarmUpFailed { reason });
            }
            Milestone::SafeModeEntered { reason } => {
                self.notifications.post(&safe_mode_card(&reason))
            }
            Milestone::BootFailed { reason } => log::warn!("Android did not start: {reason}"),
            Milestone::InstallFinished { path, ok, message } => {
                let file_name = path.rsplit('/').next().unwrap_or(&path).to_owned();
                let _ = std::fs::remove_file(self.env.paths.inbox().join(&file_name));
                let message = if message.is_empty() {
                    file_name
                } else {
                    message
                };
                self.notifications.post(&install_card(ok, &message));
            }
            Milestone::GsfId { id } => self.gsf_queries.answer_all(server, Some(&id)),
        }
    }
}
