//! The daemon in-process: a crownos-ipc round trip of every method against
//! a fake host and a fake system tree. No D-Bus, Wayland or Android.

#![allow(clippy::expect_used)]

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use android_protocol::{LaunchOutcome, RuntimeState, StopReason, android};
use crownos_ipc::ServiceBuilder;
use runtime::Paths;
use runtime::lifecycle::BootFailure;

use crate::activation::Launch;
use crate::daemon::{Daemon, Environment, Services};
use crate::host_worker::Host;
use crate::notice::{Notifier, notice_channel};

const WAIT: Option<Duration> = Some(Duration::from_secs(10));
/// Socket paths must fit `sun_path`; the build's own temp dir is too deep.
const SHORT_TEMP: &str = "/tmp";
const PACKAGE: &str = "org.example.notes";
const ANDROID_RON: &str =
    r#"(enabled: true, google_play: false, tier: Some(Native), bundle: Some("2026.11.0"))"#;

/// Records the host steps instead of taking them.
#[derive(Clone, Default)]
struct FakeHost(Arc<Mutex<Vec<String>>>);

impl FakeHost {
    fn record(&self, step: impl Into<String>) {
        self.0.lock().expect("host log").push(step.into());
    }
}

impl Host for FakeHost {
    fn start_session(&mut self, venus: bool) -> Result<(), BootFailure> {
        self.record(format!("start venus={venus}"));
        Ok(())
    }
    fn stop_session(&mut self, venus: bool) {
        self.record(format!("stop venus={venus}"));
    }
    fn set_safe_mode(&mut self, enabled: bool) {
        self.record(format!("safe mode {enabled}"));
    }
    fn freeze(&mut self) {
        self.record("freeze");
    }
    fn thaw(&mut self, venus: bool) {
        self.record(format!("thaw venus={venus}"));
    }
    fn hold_sleep_inhibitor(&mut self, _: &Notifier) {
        self.record("hold inhibitor");
    }
    fn release_sleep_inhibitor(&mut self) {
        self.record("release inhibitor");
    }
    fn launch_intent(&mut self, action: &str, uri: &str) {
        self.record(format!("intent {action} {uri}"));
    }
    fn retry_setup(&mut self) {
        self.record("retry setup");
    }
}

/// How far the fake system's setup got.
#[derive(Clone, Copy)]
enum Setup {
    Done,
    Pending,
}

const SETUP_READY: &str =
    r#"{"format":1,"stage":"ready","percent":100,"message":"Ready","failure":null}"#;
const SETUP_DOWNLOADING: &str = r#"{"format":1,"stage":"downloading","percent":40,"message":"Downloading Android","failure":null}"#;

/// A daemon serving a temporary tree from its own thread.
struct Harness {
    root: PathBuf,
    socket_dir: PathBuf,
    paths: Paths,
    host: FakeHost,
}

impl Harness {
    fn start(name: &str) -> Self {
        Self::start_with(name, Setup::Done)
    }

    fn start_with(name: &str, setup: Setup) -> Self {
        let root = std::env::temp_dir().join(format!("androidd-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let runtime_dir = Path::new(SHORT_TEMP).join(format!("adt-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&runtime_dir);
        let paths = Paths::under_home(&root.join("home"), &runtime_dir);
        let socket_dir = runtime_dir.join("crownos");
        fs::create_dir_all(&socket_dir).expect("create the runtime directory");
        let system = root.join("system");
        write(&system.join("etc/crownos/android.ron"), ANDROID_RON);
        let setup_dir = system.join("var/lib/crownos/android");
        match setup {
            Setup::Done => write(&setup_dir.join("setup.json"), SETUP_READY),
            Setup::Pending => {
                write(&setup_dir.join("pending"), "");
                write(&setup_dir.join("setup.json"), SETUP_DOWNLOADING);
            }
        }
        write(&paths.state_file(), "(warmed_up: true)");
        write(
            &paths
                .waydroid_applications()
                .join(format!("waydroid.{PACKAGE}.desktop")),
            "[Desktop Entry]\nName=Notes\n",
        );
        let host = FakeHost::default();
        let (ready, serving) = mpsc::channel();
        let thread_host = host.clone();
        let (thread_paths, thread_dir) = (paths.clone(), socket_dir.clone());
        std::thread::spawn(move || {
            let server = ServiceBuilder::new(android::SERVICE)
                .directory(&thread_dir)
                .build()
                .expect("bind the service socket");
            let (notifier, inbox) = notice_channel().expect("notice pipe");
            let services = Services {
                host: Box::new(thread_host),
                buses: None,
                wayland: false,
                tone: splash::Tone::Light,
            };
            let env = Environment {
                paths: thread_paths,
                system_root: system,
            };
            let daemon = Daemon::new(env, services, notifier, 5).expect("daemon");
            ready.send(()).expect("test is waiting");
            let _ = crate::event_loop::run(server, daemon, inbox, Launch::Manual);
        });
        serving
            .recv_timeout(Duration::from_secs(10))
            .expect("daemon started");
        Self {
            root,
            socket_dir,
            paths,
            host,
        }
    }

    fn android(&self) -> android::Client {
        android::Client::connect_in(&self.socket_dir).expect("connect to the daemon")
    }

    fn system(&self) -> PathBuf {
        self.root.join("system")
    }

    fn host_steps(&self) -> Vec<String> {
        self.host.0.lock().expect("host log").clone()
    }
}

impl Drop for Harness {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
        let _ = fs::remove_dir_all(&self.paths.runtime_dir);
    }
}

fn write(path: &Path, text: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create the fake tree");
    }
    fs::write(path, text).expect("write the fake tree");
}

#[test]
fn status_reports_the_installed_runtime() {
    let harness = Harness::start("status");
    let mut client = harness.android();
    let pending = client.status().expect("send");
    let status = client.wait(pending, WAIT).expect("status");
    assert_eq!(status.state, RuntimeState::Stopped);
    assert_eq!(status.tier, Some(android_protocol::GpuTier::Native));
    assert_eq!(status.bundle_version.as_deref(), Some("2026.11.0"));
    assert_eq!(status.windows, 0);
}

#[test]
fn launch_starts_android_and_refuses_unknown_packages() {
    let harness = Harness::start("launch");
    let mut client = harness.android();
    client
        .subscribe_blocking::<android::RuntimeChanged>(WAIT)
        .expect("subscribe");
    let pending = client
        .launch(PACKAGE.into(), Some("token".into()))
        .expect("send");
    assert_eq!(
        client.wait(pending, WAIT).expect("launch"),
        LaunchOutcome::Starting
    );
    let pending = client
        .launch("org.example.missing".into(), None)
        .expect("send");
    assert_eq!(
        client.wait(pending, WAIT).expect("launch"),
        LaunchOutcome::UnknownPackage
    );
    let pending = client.launch("rm -rf".into(), None).expect("send");
    assert_eq!(
        client.wait(pending, WAIT).expect("launch"),
        LaunchOutcome::UnknownPackage
    );
    let pending = client.status().expect("send");
    client.wait(pending, WAIT).expect("status");
    let changed = client.next_event().expect("a RuntimeChanged event");
    let android::Event::RuntimeChanged(changed) = changed else {
        panic!("expected RuntimeChanged, got {changed:?}");
    };
    assert_ne!(changed.status.state, RuntimeState::Stopped);
}

#[test]
fn stop_returns_after_the_stop_sequence() {
    let harness = Harness::start("stop");
    let mut client = harness.android();
    let pending = client.launch(PACKAGE.into(), None).expect("send");
    client.wait(pending, WAIT).expect("launch");
    let pending = client.stop(StopReason::User).expect("send");
    client.wait(pending, WAIT).expect("stop");
    assert!(
        harness
            .host_steps()
            .iter()
            .any(|step| step.starts_with("stop"))
    );
    let pending = client.status().expect("send");
    let state = client.wait(pending, WAIT).expect("status").state;
    assert_eq!(state, RuntimeState::Stopped);
}

#[test]
fn diagnose_writes_an_archive() {
    let harness = Harness::start("diagnose");
    let mut client = harness.android();
    let pending = client.diagnose().expect("send");
    let path = client
        .wait(pending, Some(Duration::from_secs(60)))
        .expect("diagnose");
    assert!(Path::new(&path).starts_with(harness.paths.diagnose_dir()));
    assert!(Path::new(&path).is_file());
}

#[test]
fn the_registration_id_is_none_before_android_reports_one() {
    let harness = Harness::start("gsf");
    let mut client = harness.android();
    let pending = client.google_registration_id().expect("send");
    assert_eq!(client.wait(pending, WAIT).expect("id"), None);
}

#[test]
fn install_takes_only_files_already_in_the_inbox() {
    let harness = Harness::start("install");
    let mut client = harness.android();
    let pending = client.install_package("missing.apk".into()).expect("send");
    assert!(client.wait(pending, WAIT).is_err());
    let pending = client
        .install_package("../escape.apk".into())
        .expect("send");
    assert!(client.wait(pending, WAIT).is_err());
    write(&harness.paths.inbox().join("notes.apk"), "apk");
    let pending = client.install_package("notes.apk".into()).expect("send");
    assert_eq!(
        client.wait(pending, WAIT).expect("install"),
        LaunchOutcome::Starting
    );
}

#[test]
fn only_market_and_https_links_open() {
    let harness = Harness::start("open");
    let mut client = harness.android();
    let pending = client.open_uri("file:///etc/passwd".into()).expect("send");
    assert!(client.wait(pending, WAIT).is_err());
    let pending = client
        .open_uri("market://details?id=org.example.notes".into())
        .expect("send");
    assert_eq!(
        client.wait(pending, WAIT).expect("open"),
        LaunchOutcome::Starting
    );
}

#[test]
fn app_settings_need_a_package_name() {
    let harness = Harness::start("settings");
    let mut client = harness.android();
    let pending = client
        .open_app_settings("not a package".into())
        .expect("send");
    assert_eq!(
        client.wait(pending, WAIT).expect("settings"),
        LaunchOutcome::UnknownPackage
    );
    let pending = client.open_app_settings(PACKAGE.into()).expect("send");
    assert_eq!(
        client.wait(pending, WAIT).expect("settings"),
        LaunchOutcome::Starting
    );
}

#[test]
fn the_menu_gets_the_mirrors_handlers_and_overrides() {
    let harness = Harness::start("menu");
    let mut client = harness.android();
    let pending = client.status().expect("send");
    client.wait(pending, WAIT).expect("status");
    let menu = harness.paths.applications();
    let mirror =
        fs::read_to_string(menu.join(format!("waydroid.{PACKAGE}.desktop"))).expect("mirror");
    assert!(mirror.contains("Exec=crown-android launch org.example.notes"));
    assert!(menu.join("crown-android-handler.desktop").is_file());
    let launcher = fs::read_to_string(menu.join("Waydroid.desktop")).expect("override");
    assert!(launcher.contains("Hidden=true"));
}

#[test]
fn setup_progress_follows_setup_json() {
    let harness = Harness::start_with("setup", Setup::Pending);
    let mut client = harness.android();
    client
        .subscribe_blocking::<android::SetupProgress>(WAIT)
        .expect("subscribe");
    let pending = client.status().expect("send");
    assert_eq!(
        client.wait(pending, WAIT).expect("status").state,
        RuntimeState::SettingUp
    );
    let advanced = SETUP_DOWNLOADING.replace("40", "75");
    let setup_json = harness.system().join("var/lib/crownos/android/setup.json");
    let staged = setup_json.with_extension("tmp");
    write(&staged, &advanced);
    fs::rename(&staged, &setup_json).expect("replace setup.json");
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    let progress = loop {
        let pending = client.status().expect("send");
        client.wait(pending, WAIT).expect("status");
        if let Some(android::Event::SetupProgress(progress)) = client.next_event() {
            break progress;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "no SetupProgress event"
        );
        std::thread::sleep(Duration::from_millis(50));
    };
    assert_eq!(progress.status.percent, Some(75));
}
