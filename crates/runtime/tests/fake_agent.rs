//! The daemon's agent link and lifecycle against a fake crown-agent on the
//! other end of a Unix socket pair, speaking agent-wire.

#![allow(clippy::expect_used)]

use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use agent_wire::{Decoded, KeepAliveReasons, Record, WIRE_VERSION};
use android_protocol::{GpuTier, RuntimeState};
use runtime::agent::{AgentError, AgentLink};
use runtime::lifecycle::Effect;
use runtime::{Errand, Installation, Installed, Lifecycle, Persisted};
use rustix::event::{PollFd, PollFlags, Timespec};

const BUNDLE: &str = "2026.11.0";
const PACKAGE: &str = "org.example.notes";
const WAIT: Duration = Duration::from_secs(5);

/// A crown-agent that answers like the real one. `trickle` writes its
/// records one byte at a time.
fn spawn_agent(stream: UnixStream, trickle: bool) -> JoinHandle<Vec<Record>> {
    std::thread::spawn(move || {
        let mut stream = stream;
        let mut seen = Vec::new();
        let mut buffer = Vec::new();
        let mut tasks = Vec::new();
        loop {
            let Some(record) = read_record(&mut stream, &mut buffer) else {
                return seen;
            };
            let replies = match &record {
                Record::Hello { .. } => vec![
                    Record::Hello {
                        wire: WIRE_VERSION,
                        bundle: BUNDLE.into(),
                    },
                    Record::Ready { boot_ms: 8500 },
                ],
                Record::Launch { package, .. } => {
                    tasks.push(41);
                    vec![Record::TaskOpened {
                        task: 41,
                        package: package.clone(),
                    }]
                }
                Record::ListTasks => vec![Record::Tasks {
                    tasks: tasks.clone(),
                }],
                Record::QueryKeepAlive => vec![Record::KeepAlive {
                    reasons: KeepAliveReasons(0),
                }],
                Record::Shutdown => {
                    seen.push(record);
                    return seen;
                }
                _ => Vec::new(),
            };
            seen.push(record);
            for reply in replies {
                let Ok(bytes) = agent_wire::encode(&reply) else {
                    return seen;
                };
                let written = if trickle {
                    bytes.chunks(1).try_for_each(|byte| stream.write_all(byte))
                } else {
                    stream.write_all(&bytes)
                };
                if written.is_err() {
                    return seen;
                }
            }
        }
    })
}

fn read_record(stream: &mut UnixStream, buffer: &mut Vec<u8>) -> Option<Record> {
    loop {
        if let Ok(Decoded::Record(record, used)) = agent_wire::decode(buffer) {
            buffer.drain(..used);
            return Some(record);
        }
        let mut chunk = [0; 512];
        match stream.read(&mut chunk) {
            Ok(0) | Err(_) => return None,
            Ok(read) => buffer.extend_from_slice(&chunk[..read]),
        }
    }
}

/// The daemon side: polls the link, feeds the lifecycle and carries out its
/// agent effects, until `done` holds.
struct Daemon {
    link: AgentLink,
    lifecycle: Lifecycle,
    effects: Vec<Effect>,
}

impl Daemon {
    fn new(stream: UnixStream) -> Self {
        let mut lifecycle = Lifecycle::new(
            Persisted {
                warmed_up: true,
                ..Persisted::default()
            },
            5,
        );
        lifecycle.set_installation(
            Installation::Installed(Installed {
                tier: Some(GpuTier::Native),
                bundle: Some(BUNDLE.into()),
                google_play: false,
            }),
            Instant::now(),
        );
        let link = AgentLink::from_stream(stream).expect("non-blocking stream");
        Self {
            link,
            lifecycle,
            effects: Vec::new(),
        }
    }

    fn apply_effects(&mut self) {
        for effect in self.lifecycle.take_effects() {
            match &effect {
                Effect::SendAgent(record) => self.link.send(record).expect("send"),
                Effect::Perform(Errand::Launch { package, token }) => self
                    .link
                    .send(&Record::Launch {
                        package: package.clone(),
                        width: 1280,
                        height: 800,
                        token: token.clone().unwrap_or_default(),
                    })
                    .expect("send launch"),
                _ => {}
            }
            self.effects.push(effect);
        }
    }

    fn run_until(&mut self, done: impl Fn(&Lifecycle) -> bool) {
        let deadline = Instant::now() + WAIT;
        self.apply_effects();
        while !done(&self.lifecycle) {
            assert!(Instant::now() < deadline, "the exchange stalled");
            let mut fds = [PollFd::new(&self.link, PollFlags::IN)];
            let timeout = Timespec::try_from(Duration::from_millis(100)).expect("timeout");
            rustix::event::poll(&mut fds, Some(&timeout)).expect("poll");
            for record in self.link.receive().expect("receive") {
                self.lifecycle.agent_record(record, Instant::now());
            }
            self.apply_effects();
        }
    }
}

fn boot_through_the_agent(trickle: bool) -> (Daemon, JoinHandle<Vec<Record>>) {
    let (ours, theirs) = UnixStream::pair().expect("socket pair");
    let agent = spawn_agent(theirs, trickle);
    let mut daemon = Daemon::new(ours);
    daemon.lifecycle.submit(
        Errand::Launch {
            package: PACKAGE.into(),
            token: Some("xdg-token".into()),
        },
        Instant::now(),
    );
    daemon.lifecycle.session_started();
    daemon.lifecycle.agent_connected();
    daemon.run_until(|lifecycle| lifecycle.windows() == 1);
    (daemon, agent)
}

#[test]
fn a_cold_launch_reaches_the_agent_after_the_handshake() {
    let (mut daemon, agent) = boot_through_the_agent(false);
    assert_eq!(daemon.lifecycle.state(), RuntimeState::Running);
    assert!(daemon.lifecycle.has_task(PACKAGE));
    daemon.lifecycle.stop(android_protocol::StopReason::User);
    daemon.apply_effects();
    let seen = agent.join().expect("agent thread");
    assert_eq!(
        seen.first(),
        Some(&Record::Hello {
            wire: WIRE_VERSION,
            bundle: BUNDLE.into()
        })
    );
    assert!(seen.contains(&Record::Launch {
        package: PACKAGE.into(),
        width: 1280,
        height: 800,
        token: "xdg-token".into(),
    }));
    assert_eq!(seen.last(), Some(&Record::Shutdown));
}

#[test]
fn records_split_across_reads_still_decode() {
    let (daemon, _agent) = boot_through_the_agent(true);
    assert_eq!(daemon.lifecycle.windows(), 1);
}

#[test]
fn the_idle_countdown_asks_the_agent_and_stops_android() {
    let (mut daemon, agent) = boot_through_the_agent(false);
    daemon
        .lifecycle
        .agent_record(Record::TaskClosed { task: 41 }, Instant::now());
    let deadline = daemon.lifecycle.idle_deadline().expect("counting down");
    daemon.lifecycle.tick(deadline);
    daemon.run_until(|lifecycle| lifecycle.state() == RuntimeState::Stopping);
    let seen = agent.join().expect("agent thread");
    assert!(seen.contains(&Record::QueryKeepAlive));
    assert_eq!(seen.last(), Some(&Record::Shutdown));
}

#[test]
fn a_malformed_record_ends_the_link() {
    let (ours, mut theirs) = UnixStream::pair().expect("socket pair");
    let mut link = AgentLink::from_stream(ours).expect("non-blocking stream");
    theirs
        .write_all(&[8, 0, 0, 0, 0xff, 0x7f, 0, 0, 1, 2, 3, 4])
        .expect("write");
    std::thread::sleep(Duration::from_millis(20));
    assert!(matches!(link.receive(), Err(AgentError::Wire(_))));
    drop(theirs);
    std::thread::sleep(Duration::from_millis(20));
    assert!(matches!(link.receive(), Err(AgentError::Closed)));
}
