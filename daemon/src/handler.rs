//! The `crownos-android` methods: contract 4 and the supplement.

use std::time::Instant;

use agent_wire::Record;
use android_protocol::{LaunchOutcome, RuntimeState, RuntimeStatus, StopReason, android};
use crownos_ipc::{Error, Kind, Message, MethodMsg, PeerId, RemoteError, Server, postcard};
use desktop_entries::is_package_name;
use runtime::paths::guest_inbox_path;
use runtime::{Errand, supplement};

use crate::daemon::{Daemon, GSF_REPLY};
use crate::deferred::ReplyTo;
use crate::diagnose;
use crate::notice::Notice;
use crate::setup::knows_package;

const ANSWERED_LATER: &str = "answered once the work is done";
const URI_SCHEMES: [&str; 2] = ["market:", "https:"];

impl Daemon {
    /// Routes one request. Requests whose answer waits for a worker or the
    /// agent are taken out first.
    pub fn dispatch(
        &mut self,
        server: &mut Server,
        peer: PeerId,
        message: Message,
    ) -> Result<(), Error> {
        let reply = ReplyTo {
            peer,
            req_id: message.req_id,
        };
        let request = message.kind == Kind::Request;
        match message.selector {
            android::diagnose::SELECTOR if request => {
                self.start_diagnose(reply);
                Ok(())
            }
            android::google_registration_id::SELECTOR if request => {
                self.query_gsf_id(server, reply, Instant::now());
                Ok(())
            }
            android::stop::SELECTOR if request => {
                match postcard::from_bytes::<android::stop>(&message.payload) {
                    Ok(android::stop { reason }) => self.stop_then_reply(server, reply, reason),
                    Err(_) => server.drop_peer(peer),
                }
                Ok(())
            }
            supplement::install_package::SELECTOR
            | supplement::open_uri::SELECTOR
            | supplement::open_app_settings::SELECTOR => {
                supplement::dispatch(self, server, peer, message)
            }
            _ => android::dispatch(self, server, peer, message),
        }
    }

    fn start_diagnose(&mut self, reply: ReplyTo) {
        let report = diagnose::Report {
            state: self.describe(),
            archive_dir: self.env.paths.diagnose_dir(),
            system_root: self.env.system_root.clone(),
        };
        let notifier = self.notifier.clone();
        let spawned = std::thread::Builder::new()
            .name("diagnose".into())
            .spawn(move || {
                let archive = diagnose::write(&report)
                    .map(|path| path.display().to_string())
                    .map_err(|error| error.to_string());
                notifier.notify(Notice::Diagnosed { reply, archive });
            });
        match spawned {
            Ok(_) => self.diagnoses += 1,
            Err(error) => log::warn!("could not start diagnostics: {error}"),
        }
    }

    /// Stops Android and answers once the stop sequence has finished, so the
    /// caller sees no Waydroid process left when the call returns.
    fn stop_then_reply(&mut self, server: &mut Server, reply: ReplyTo, reason: StopReason) {
        self.lifecycle.stop(reason);
        if self.lifecycle.state() == RuntimeState::Stopping {
            self.pending_stops.push(reply);
        } else {
            let _ = reply.send(server, Ok(&()));
        }
    }

    fn query_gsf_id(&mut self, server: &mut Server, reply: ReplyTo, now: Instant) {
        let known = self.lifecycle.persisted().gsf_id.clone();
        if known.is_some() || !self.agent_up() {
            let _ = reply.send(server, Ok(&known));
            return;
        }
        self.send_agent(&Record::QueryGsfId, now);
        self.gsf_queries.push(reply, now + GSF_REPLY);
    }

    /// The daemon's state as text, for diagnostics.
    fn describe(&self) -> String {
        format!(
            "status: {:#?}\ninstallation: {:#?}\nunsupported: {:?}\npersisted: {:#?}\nsetup: {:#?}\nagent connected: {}\n",
            self.status(),
            self.lifecycle.installation(),
            self.lifecycle.unsupported_reason(),
            self.lifecycle.persisted(),
            self.setup_status,
            self.agent.is_some(),
        )
    }

    fn submit(&mut self, errand: Errand) -> LaunchOutcome {
        self.lifecycle.submit(errand, Instant::now())
    }
}

fn refused(message: impl Into<String>) -> RemoteError {
    RemoteError::handler(message)
}

impl android::Handler for Daemon {
    fn launch(
        &mut self,
        package: String,
        activation_token: Option<String>,
    ) -> Result<LaunchOutcome, RemoteError> {
        if !is_package_name(&package) {
            return Ok(LaunchOutcome::UnknownPackage);
        }
        let warmed_up = self.lifecycle.persisted().warmed_up;
        if warmed_up
            && self.lifecycle.installed().is_some()
            && !knows_package(&self.env.paths, &package)
        {
            return Ok(LaunchOutcome::UnknownPackage);
        }
        Ok(self.submit(Errand::Launch {
            package,
            token: activation_token.filter(|token| !token.is_empty()),
        }))
    }

    fn stop(&mut self, reason: StopReason) -> Result<(), RemoteError> {
        self.lifecycle.stop(reason);
        Ok(())
    }

    fn status(&mut self) -> Result<RuntimeStatus, RemoteError> {
        Ok(Daemon::status(self))
    }

    fn diagnose(&mut self) -> Result<String, RemoteError> {
        Err(refused(ANSWERED_LATER))
    }

    fn google_registration_id(&mut self) -> Result<Option<String>, RemoteError> {
        Err(refused(ANSWERED_LATER))
    }
}

impl supplement::Handler for Daemon {
    fn install_package(&mut self, file_name: String) -> Result<LaunchOutcome, RemoteError> {
        let guest_path = guest_inbox_path(&file_name).ok_or_else(|| refused("not a file name"))?;
        if !self.env.paths.inbox().join(&file_name).is_file() {
            return Err(refused(format!("{file_name} is not in the inbox")));
        }
        Ok(self.submit(Errand::Install { guest_path }))
    }

    fn open_uri(&mut self, uri: String) -> Result<LaunchOutcome, RemoteError> {
        if !URI_SCHEMES.iter().any(|scheme| uri.starts_with(scheme))
            || uri.chars().any(char::is_control)
        {
            return Err(refused("only market: and https: links open in Android"));
        }
        Ok(self.submit(Errand::OpenUri { uri }))
    }

    fn open_app_settings(&mut self, package: String) -> Result<LaunchOutcome, RemoteError> {
        if !is_package_name(&package) {
            return Ok(LaunchOutcome::UnknownPackage);
        }
        Ok(self.submit(Errand::AppSettings { package }))
    }
}
