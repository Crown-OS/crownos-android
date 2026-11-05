//! The daemon's state: the lifecycle and the adapters that carry out its
//! effects.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use android_protocol::{RuntimeState, RuntimeStatus, android};
use crownos_ipc::Server;
use runtime::agent::AgentLink;
use runtime::host::Buses;
use runtime::watch::DirWatch;
use runtime::{Installation, Lifecycle, Paths, Persisted};
use setup_state::SetupStatus;
use splash::{SecurityContext, Tone};

use crate::deferred::{GsfQueries, ReplyTo};
use crate::host_worker::{Host, HostWorker};
use crate::notice::Notifier;
use crate::notify::Notifications;
use crate::util::time::unix_ms;
use crate::wayland::{Reach, Wayland};

/// How often a connection to the agent socket is tried while Android boots.
pub const AGENT_RETRY: Duration = Duration::from_millis(250);
/// How long `google_registration_id` waits for the agent.
pub const GSF_REPLY: Duration = Duration::from_secs(5);

/// The crownos-ipc socket directory inside `$XDG_RUNTIME_DIR`.
const IPC_SOCKET_DIR: &str = "crownos";

/// The directories a daemon reads and writes.
pub struct Environment {
    pub paths: Paths,
    /// `/` on a real system; tests use a fake tree.
    pub system_root: PathBuf,
}

/// What the daemon talks to.
pub struct Services {
    pub host: Box<dyn Host>,
    /// D-Bus for notifications and the compositor's environment; `None` in tests.
    pub buses: Option<Arc<Buses>>,
    /// Whether to connect to the compositor.
    pub wayland: bool,
    pub tone: Tone,
}

/// The directories whose changes the daemon follows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Topic {
    /// `/var/lib/crownos/android`: the marker and `setup.json`.
    Setup,
    /// `/etc/crownos`: `android.ron`.
    SystemConfig,
    /// Waydroid's desktop entries.
    Apps,
}

pub struct Daemon {
    pub env: Environment,
    pub lifecycle: Lifecycle,
    pub host: HostWorker,
    pub notifier: Notifier,
    pub wayland: Wayland,
    pub watch: DirWatch<Topic>,
    pub notifications: Notifications,
    pub session: Option<SecurityContext>,
    pub agent: Option<AgentLink>,
    pub next_agent_attempt: Instant,
    pub setup_status: Option<SetupStatus>,
    /// Whether the menu currently carries Android's entries.
    pub menu_enabled: Option<bool>,
    pub gsf_queries: GsfQueries,
    /// `stop` calls answered once the stop sequence finishes.
    pub pending_stops: Vec<ReplyTo>,
    pub diagnoses: usize,
    last_status: Option<RuntimeStatus>,
}

impl Daemon {
    pub fn new(
        env: Environment,
        services: Services,
        notifier: Notifier,
        idle_minutes: u32,
    ) -> anyhow::Result<Self> {
        let persisted = Persisted::load(&env.paths.state_file());
        let reach = services.wayland.then(|| Reach {
            runtime_dir: env.paths.runtime_dir.clone(),
            buses: services.buses.clone(),
            tone: services.tone,
        });
        Ok(Self {
            lifecycle: Lifecycle::new(persisted, idle_minutes),
            host: HostWorker::spawn(services.host, notifier.clone())?,
            notifications: Notifications::new(
                notifier.clone(),
                services.buses,
                env.paths.runtime_dir.join(IPC_SOCKET_DIR),
            ),
            notifier,
            wayland: Wayland::new(reach),
            watch: DirWatch::new()?,
            session: None,
            agent: None,
            next_agent_attempt: Instant::now(),
            setup_status: None,
            menu_enabled: None,
            gsf_queries: GsfQueries::default(),
            pending_stops: Vec::new(),
            diagnoses: 0,
            last_status: None,
            env,
        })
    }

    /// Reads the installation, syncs the menu and adopts an Android that is
    /// already running, for example after the daemon restarted.
    pub fn start(&mut self, server: &mut Server, now: Instant) {
        self.wayland.get(now);
        self.follow_directories();
        self.reload_installation(server, now);
        self.adopt_running_android(now);
        self.settle(server, now);
    }

    fn adopt_running_android(&mut self, now: Instant) {
        if self.lifecycle.installed().is_none() || self.lifecycle.is_active() {
            return;
        }
        if let Ok(link) = AgentLink::connect(&self.env.paths.agent_socket()) {
            log::info!("Android is already running, taking it over");
            self.lifecycle.adopt(now);
            self.agent = Some(link);
            self.lifecycle.agent_connected();
        }
    }

    pub fn status(&self) -> RuntimeStatus {
        let installed = self.lifecycle.installed();
        RuntimeStatus {
            state: self.lifecycle.state(),
            windows: self.lifecycle.windows(),
            idle_stop_at_ms: self.lifecycle.idle_deadline().map(unix_ms),
            tier: installed.and_then(|installed| installed.tier),
            bundle_version: installed.and_then(|installed| installed.bundle.clone()),
        }
    }

    /// Emits `RuntimeChanged` when the status differs from the last one sent.
    pub fn emit_status(&mut self, server: &mut Server) {
        let status = self.status();
        if self.last_status.as_ref() == Some(&status) {
            return;
        }
        if let Err(error) = server.emit(&android::RuntimeChanged {
            status: status.clone(),
        }) {
            log::warn!("could not emit RuntimeChanged: {error}");
        }
        self.last_status = Some(status);
    }

    /// Whether Android runs with its agent connected.
    pub fn agent_up(&self) -> bool {
        self.agent.is_some()
            && matches!(
                self.lifecycle.state(),
                RuntimeState::Running | RuntimeState::SafeMode
            )
    }

    /// Whether anything keeps the daemon from exiting besides clients.
    pub fn is_busy(&self) -> bool {
        self.lifecycle.is_active()
            || matches!(self.lifecycle.installation(), Installation::SettingUp)
            || self.host.is_busy()
            || self.diagnoses > 0
            || !self.gsf_queries.is_empty()
            || !self.pending_stops.is_empty()
            || self.wayland.is_busy()
            || self.notifications.awaits_user()
    }

    /// The next moment the event loop must wake for the daemon.
    pub fn next_wakeup(&self) -> Option<Instant> {
        let agent = (self.lifecycle.wants_agent() && self.agent.is_none())
            .then_some(self.next_agent_attempt);
        [
            self.lifecycle.next_wakeup(),
            self.wayland
                .connection()
                .and_then(splash::Splashes::next_deadline),
            self.wayland.next_retry(),
            self.gsf_queries.next_deadline(),
            agent,
        ]
        .into_iter()
        .flatten()
        .min()
    }
}
