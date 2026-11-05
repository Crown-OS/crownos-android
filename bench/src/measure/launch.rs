use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Result, anyhow, bail};
use serde::Serialize;

use super::CLI;
use super::boot::BootTimes;
use crate::findings::{Findings, MetricId, Report};
use crate::host::Host;
use crate::unit::Unit;
use crate::util::clock::millis;
use crate::util::command::{Finished, Running};
use crate::wayland::toplevels::{ToplevelEvent, ToplevelId, Toplevels};

pub const SPLASH: MetricId = MetricId {
    name: "launch.splash",
    unit: Unit::Milliseconds,
};
pub const COLD_FIRST_FRAME: MetricId = MetricId {
    name: "launch.cold_first_frame",
    unit: Unit::Milliseconds,
};
pub const WARM_FIRST_FRAME: MetricId = MetricId {
    name: "launch.warm_first_frame",
    unit: Unit::Milliseconds,
};

/// The longest the loop waits on the compositor before it checks on the launch call.
const DISPATCH_INTERVAL: Duration = Duration::from_millis(10);

/// Whether Android was already running when a launch began.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Start {
    Cold,
    Warm,
}

/// One `crown-android launch`, timed from the call.
#[derive(Debug, Serialize)]
pub struct LaunchTimes {
    pub package: String,
    pub start: Start,
    pub call_ms: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// The first toplevel with the app's app_id: the daemon's splash.
    pub splash_ms: Option<f64>,
    /// The next one: the app's own window, listed once it mapped its first frame.
    pub first_frame_ms: Option<f64>,
}

/// Follows the toplevels of one app_id after a launch. The first to be listed
/// is the splash and the next the app's window; a lone toplevel that never
/// closes was the window, shown without a splash.
#[derive(Debug)]
pub struct LaunchWatch<'a> {
    app_id: &'a str,
    started: Instant,
    splash: Option<(ToplevelId, Duration)>,
    window: Option<Duration>,
    splash_closed: bool,
}

impl<'a> LaunchWatch<'a> {
    pub fn new(app_id: &'a str, started: Instant) -> Self {
        Self {
            app_id,
            started,
            splash: None,
            window: None,
            splash_closed: false,
        }
    }

    pub fn observe(&mut self, event: &ToplevelEvent) {
        match event {
            ToplevelEvent::Identified {
                toplevel,
                app_id,
                at,
            } if app_id == self.app_id && self.window.is_none() => {
                let elapsed = at.saturating_duration_since(self.started);
                match self.splash {
                    None => self.splash = Some((*toplevel, elapsed)),
                    Some((splash, _)) if splash != *toplevel => self.window = Some(elapsed),
                    Some(_) => {}
                }
            }
            ToplevelEvent::Closed { toplevel } => {
                let splash = self.splash.map(|(splash, _)| splash);
                self.splash_closed |= self.window.is_none() && splash == Some(*toplevel);
            }
            ToplevelEvent::Identified { .. } => {}
        }
    }

    /// The window appeared, or the splash closed without one.
    pub fn finished(&self) -> bool {
        self.window.is_some() || self.splash_closed
    }

    /// `(splash, first frame)`, each after the launch call.
    pub fn times(&self) -> (Option<Duration>, Option<Duration>) {
        match (self.splash, self.window) {
            (Some((_, splash)), Some(window)) => (Some(splash), Some(window)),
            (Some((_, splash)), None) if self.splash_closed => (Some(splash), None),
            (Some((_, lone)), None) => (None, Some(lone)),
            (None, _) => (None, None),
        }
    }
}

/// A launch that started Android, and the boot it set off.
#[derive(Debug, Serialize)]
pub struct ColdStart {
    pub boot: BootTimes,
    pub launch: LaunchTimes,
}

/// Launches `package` while Android is stopped and times the boot alongside.
pub fn start_android(
    host: &Host,
    mut toplevels: Option<&mut Toplevels>,
    package: &str,
    timeout: Duration,
) -> Result<ColdStart> {
    ensure_closed(toplevels.as_deref_mut(), package)?;
    let started = Instant::now();
    let abandoned = AtomicBool::new(false);
    thread::scope(|scope| {
        let boot = scope.spawn(|| BootTimes::watch(host, started, timeout, &abandoned));
        let launch = launch(toplevels, package, Start::Cold, started, timeout);
        if launch
            .as_ref()
            .map_or(true, |launch| launch.error.is_some())
        {
            abandoned.store(true, Ordering::Relaxed);
        }
        let boot = boot
            .join()
            .map_err(|_| anyhow!("the boot watch panicked"))?;
        Ok(ColdStart {
            boot,
            launch: launch?,
        })
    })
}

/// Launches `package` while Android runs.
pub fn warm(
    mut toplevels: Option<&mut Toplevels>,
    package: &str,
    timeout: Duration,
) -> Result<LaunchTimes> {
    ensure_closed(toplevels.as_deref_mut(), package)?;
    launch(toplevels, package, Start::Warm, Instant::now(), timeout)
}

/// A launch of an app with a window only raises it, which times nothing.
fn ensure_closed(toplevels: Option<&mut Toplevels>, package: &str) -> Result<()> {
    let Some(toplevels) = toplevels else {
        return Ok(());
    };
    toplevels.sync()?;
    toplevels.take_events();
    let app_id = android_protocol::app_id(package);
    if toplevels.open_app_ids().any(|open| open == app_id) {
        bail!("{app_id} already has a window; close it before the bench launches it");
    }
    Ok(())
}

fn launch(
    mut toplevels: Option<&mut Toplevels>,
    package: &str,
    start: Start,
    started: Instant,
    timeout: Duration,
) -> Result<LaunchTimes> {
    let app_id = android_protocol::app_id(package);
    let mut watch = LaunchWatch::new(&app_id, started);
    let mut call = Running::spawn(CLI, &["launch", package])?;
    let mut returned: Option<Finished> = None;
    loop {
        if returned.is_none() {
            returned = call.try_finish()?;
        }
        let call_failed = returned.as_ref().is_some_and(|call| !call.success());
        let watched = toplevels.is_none() || watch.finished();
        if call_failed || (watched && returned.is_some()) || started.elapsed() >= timeout {
            break;
        }
        match toplevels.as_deref_mut() {
            Some(toplevels) => {
                toplevels.dispatch(DISPATCH_INTERVAL)?;
                toplevels
                    .take_events()
                    .iter()
                    .for_each(|event| watch.observe(event));
            }
            None => thread::sleep(DISPATCH_INTERVAL),
        }
    }
    if returned.is_none() {
        call.kill();
    }
    let error = match &returned {
        None => Some(format!("{CLI} launch did not return")),
        Some(call) if !call.success() => Some(call.error()),
        Some(_) => None,
    };
    let (splash, first_frame) = watch.times();
    Ok(LaunchTimes {
        package: package.to_owned(),
        start,
        call_ms: returned.map(|call| millis(call.elapsed)),
        error,
        splash_ms: splash.map(millis),
        first_frame_ms: first_frame.map(millis),
    })
}

/// Every launch of a run.
#[derive(Debug, Serialize)]
pub struct Launches {
    /// Why no window could be watched, when the compositor's toplevel list is out of reach.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unwatched: Option<String>,
    pub launches: Vec<LaunchTimes>,
}

impl Report for Launches {
    fn report(&self, step: &str, findings: &mut Findings) {
        for launch in &self.launches {
            let problem = match (&launch.error, &self.unwatched) {
                (Some(error), _) => Some(error.as_str()),
                (None, None) if launch.first_frame_ms.is_none() => Some("no window appeared"),
                (None, None) if launch.splash_ms.is_none() => {
                    Some("no splash was listed before the window")
                }
                _ => None,
            };
            if let Some(problem) = problem {
                findings.fail(&format!("{step} {}", launch.package), problem);
            }
        }
        if let Some(reason) = &self.unwatched {
            for metric in [SPLASH, COLD_FIRST_FRAME, WARM_FIRST_FRAME] {
                findings.skip(metric, reason.as_str());
            }
            return;
        }
        let slowest = |start: Option<Start>, time: fn(&LaunchTimes) -> Option<f64>| {
            self.launches
                .iter()
                .filter(|launch| start.is_none_or(|start| launch.start == start))
                .filter_map(time)
                .reduce(f64::max)
        };
        let splash = slowest(None, |launch| launch.splash_ms);
        let cold = slowest(Some(Start::Cold), |launch| launch.first_frame_ms);
        let warm = slowest(Some(Start::Warm), |launch| launch.first_frame_ms);
        for (metric, value, missing) in [
            (SPLASH, splash, "no splash was listed"),
            (
                COLD_FIRST_FRAME,
                cold,
                "no app was launched with Android stopped",
            ),
            (
                WARM_FIRST_FRAME,
                warm,
                "no app was launched with Android running",
            ),
        ] {
            match value {
                Some(value) => findings.metric(metric, value),
                None => findings.skip(metric, missing),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const APP_ID: &str = "waydroid.com.android.settings";

    fn identified(toplevel: u64, app_id: &str, at: Instant) -> ToplevelEvent {
        ToplevelEvent::Identified {
            toplevel: ToplevelId(toplevel),
            app_id: app_id.to_owned(),
            at,
        }
    }

    fn closed(toplevel: u64) -> ToplevelEvent {
        ToplevelEvent::Closed {
            toplevel: ToplevelId(toplevel),
        }
    }

    #[test]
    fn times_the_splash_then_the_window() {
        let started = Instant::now();
        let mut watch = LaunchWatch::new(APP_ID, started);
        watch.observe(&identified(
            1,
            "org.crownos.files",
            started + Duration::from_millis(5),
        ));
        watch.observe(&identified(2, APP_ID, started + Duration::from_millis(60)));
        assert!(!watch.finished());
        watch.observe(&identified(2, APP_ID, started + Duration::from_millis(80)));
        watch.observe(&identified(
            3,
            APP_ID,
            started + Duration::from_millis(9_000),
        ));
        watch.observe(&closed(2));
        assert!(watch.finished());
        assert_eq!(
            watch.times(),
            (
                Some(Duration::from_millis(60)),
                Some(Duration::from_millis(9_000))
            )
        );
    }

    #[test]
    fn a_lone_toplevel_is_the_window() {
        let started = Instant::now();
        let mut watch = LaunchWatch::new(APP_ID, started);
        watch.observe(&identified(
            4,
            APP_ID,
            started + Duration::from_millis(1_200),
        ));
        assert!(!watch.finished());
        assert_eq!(watch.times(), (None, Some(Duration::from_millis(1_200))));
    }

    #[test]
    fn a_splash_closing_alone_is_a_failed_launch() {
        let started = Instant::now();
        let mut watch = LaunchWatch::new(APP_ID, started);
        watch.observe(&identified(5, APP_ID, started + Duration::from_millis(40)));
        watch.observe(&closed(5));
        assert!(watch.finished());
        assert_eq!(watch.times(), (Some(Duration::from_millis(40)), None));
    }

    fn times(
        package: &str,
        start: Start,
        splash: Option<f64>,
        first_frame: Option<f64>,
    ) -> LaunchTimes {
        LaunchTimes {
            package: package.to_owned(),
            start,
            call_ms: Some(4.0),
            error: None,
            splash_ms: splash,
            first_frame_ms: first_frame,
        }
    }

    #[test]
    fn reports_the_slowest_launch_of_each_kind() {
        let launches = Launches {
            unwatched: None,
            launches: vec![
                times(
                    "com.android.settings",
                    Start::Cold,
                    Some(70.0),
                    Some(9_100.0),
                ),
                times(
                    "com.android.documentsui",
                    Start::Warm,
                    Some(90.0),
                    Some(900.0),
                ),
                times(
                    "org.lineageos.jelly",
                    Start::Warm,
                    Some(30.0),
                    Some(1_100.0),
                ),
            ],
        };
        let mut findings = Findings::default();
        launches.report("launch", &mut findings);
        let value = |metric: MetricId| findings.metrics.get(metric.name).map(|m| m.value);
        assert_eq!(value(SPLASH), Some(90.0));
        assert_eq!(value(COLD_FIRST_FRAME), Some(9_100.0));
        assert_eq!(value(WARM_FIRST_FRAME), Some(1_100.0));
        assert!(findings.failures.is_empty());
    }

    #[test]
    fn reports_missing_windows_and_displays() {
        let mut failed = times("com.android.settings", Start::Cold, Some(70.0), None);
        failed.error = Some("Android is not set up".to_owned());
        let launches = Launches {
            unwatched: None,
            launches: vec![
                failed,
                times("com.android.documentsui", Start::Warm, None, Some(800.0)),
            ],
        };
        let mut findings = Findings::default();
        launches.report("launch", &mut findings);
        assert_eq!(
            findings.failures["launch com.android.settings"],
            "Android is not set up"
        );
        assert_eq!(
            findings.failures["launch com.android.documentsui"],
            "no splash was listed before the window"
        );
        assert!(findings.skipped.contains_key(COLD_FIRST_FRAME.name));

        let blind = Launches {
            unwatched: Some("no Wayland display".to_owned()),
            launches: vec![times("com.android.settings", Start::Cold, None, None)],
        };
        let mut findings = Findings::default();
        blind.report("launch", &mut findings);
        assert!(findings.failures.is_empty());
        assert_eq!(findings.skipped[SPLASH.name], "no Wayland display");
    }
}
