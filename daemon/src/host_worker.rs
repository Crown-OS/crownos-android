//! One thread that runs every blocking host step in submission order, so a
//! freeze can never overtake the stop before it.

use std::io;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{Sender, channel};

use runtime::lifecycle::BootFailure;

use crate::notice::{Notice, Notifier};

/// The host steps, behind a trait so tests run the daemon without D-Bus.
pub trait Host: Send {
    fn start_session(&mut self, venus: bool) -> Result<(), BootFailure>;
    fn stop_session(&mut self, venus: bool);
    fn set_safe_mode(&mut self, enabled: bool);
    fn freeze(&mut self);
    fn thaw(&mut self, venus: bool);
    fn hold_sleep_inhibitor(&mut self, notifier: &Notifier);
    fn release_sleep_inhibitor(&mut self);
    fn launch_intent(&mut self, action: &str, uri: &str);
    fn retry_setup(&mut self);
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Job {
    StartSession { venus: bool },
    StopSession { venus: bool },
    SetSafeMode(bool),
    Freeze,
    Thaw { venus: bool },
    HoldSleepInhibitor,
    ReleaseSleepInhibitor,
    LaunchIntent { action: &'static str, uri: String },
    RetrySetup,
}

pub struct HostWorker {
    jobs: Sender<Job>,
    in_flight: Arc<AtomicUsize>,
}

impl HostWorker {
    pub fn spawn(mut host: Box<dyn Host>, notifier: Notifier) -> io::Result<Self> {
        let (jobs, inbox) = channel::<Job>();
        let in_flight = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&in_flight);
        std::thread::Builder::new()
            .name("host".into())
            .spawn(move || {
                for job in inbox {
                    if let Some(notice) = run(host.as_mut(), job, &notifier) {
                        notifier.notify(notice);
                    }
                    counter.fetch_sub(1, Ordering::Release);
                }
            })?;
        Ok(Self { jobs, in_flight })
    }

    pub fn submit(&self, job: Job) {
        self.in_flight.fetch_add(1, Ordering::Acquire);
        if self.jobs.send(job).is_err() {
            self.in_flight.fetch_sub(1, Ordering::Release);
            log::error!("the host worker is gone");
        }
    }

    pub fn is_busy(&self) -> bool {
        self.in_flight.load(Ordering::Acquire) > 0
    }
}

fn run(host: &mut dyn Host, job: Job, notifier: &Notifier) -> Option<Notice> {
    match job {
        Job::StartSession { venus } => {
            return Some(Notice::SessionStarted(host.start_session(venus)));
        }
        Job::StopSession { venus } => {
            host.stop_session(venus);
            return Some(Notice::SessionStopped);
        }
        Job::SetSafeMode(enabled) => host.set_safe_mode(enabled),
        Job::Freeze => host.freeze(),
        Job::Thaw { venus } => host.thaw(venus),
        Job::HoldSleepInhibitor => host.hold_sleep_inhibitor(notifier),
        Job::ReleaseSleepInhibitor => host.release_sleep_inhibitor(),
        Job::LaunchIntent { action, uri } => host.launch_intent(action, &uri),
        Job::RetrySetup => host.retry_setup(),
    }
    None
}
