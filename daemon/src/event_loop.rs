//! The single-threaded loop: IPC, the agent, Wayland, crownotify, file
//! watches, worker notices, timers and the idle exit.

use std::os::fd::AsFd;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use anyhow::Context;
use crownos_ipc::{PeerId, RemoteError, Server};
use rustix::event::{PollFd, PollFlags, Timespec};

use crate::activation::Launch;
use crate::daemon::{AGENT_RETRY, Daemon, Topic};
use crate::notice::{Notice, NoticeInbox};
use crate::notify::UserAction;
use crate::util::shutdown_signal::shutdown_signal;

/// The daemon exits after this long with Android stopped, no client and
/// nothing pending.
const IDLE_EXIT: Duration = Duration::from_secs(30);

struct Readiness {
    listener: bool,
    notices: bool,
    watch: bool,
    agent: PollFlags,
    wayland: bool,
    crownotify: bool,
    peers: Vec<(PeerId, PollFlags)>,
}

pub fn run(
    mut server: Server,
    mut daemon: Daemon,
    mut inbox: NoticeInbox,
    launch: Launch,
) -> anyhow::Result<()> {
    let shutdown = shutdown_signal().context("could not handle termination signals")?;
    daemon.start(&mut server, Instant::now());
    let mut idle_since = Instant::now();
    let mut idle_exit = match launch {
        Launch::Systemd if !daemon.is_busy() => Duration::ZERO,
        Launch::Systemd | Launch::Manual => IDLE_EXIT,
    };
    log::info!("serving {}", server.service());
    loop {
        daemon.before_poll(Instant::now());
        let idle = server.peer_ids().is_empty() && !daemon.is_busy();
        let exit_at = idle.then_some(idle_since + idle_exit);
        let wakeup = [daemon.next_wakeup(), exit_at].into_iter().flatten().min();
        let ready = wait(&server, &inbox, &daemon, wakeup)?;
        if shutdown.load(Ordering::Relaxed) {
            log::info!("asked to stop, exiting");
            return Ok(());
        }
        let now = Instant::now();
        if ready.listener {
            while server.accept()?.is_some() {}
        }
        for (peer, events) in ready.peers {
            serve_peer(&mut server, &mut daemon, peer, events)?;
        }
        if ready.notices {
            for notice in inbox.drain() {
                daemon.handle_notice(&mut server, notice, now);
            }
        }
        if ready.watch {
            daemon.handle_watch(&mut server, now);
        }
        daemon.handle_io(&ready.agent, ready.wayland, ready.crownotify, now);
        daemon.tick(&mut server, now);
        daemon.settle(&mut server, now);
        if !(server.peer_ids().is_empty() && !daemon.is_busy()) {
            idle_since = now;
            idle_exit = IDLE_EXIT;
        } else if now.duration_since(idle_since) >= idle_exit {
            log::info!("idle for {idle_exit:?}, exiting");
            return Ok(());
        }
    }
}

fn serve_peer(
    server: &mut Server,
    daemon: &mut Daemon,
    peer: PeerId,
    events: PollFlags,
) -> anyhow::Result<()> {
    if events.contains(PollFlags::OUT) && server.handle_writable(peer).is_err() {
        return Ok(());
    }
    if events.intersects(PollFlags::IN | PollFlags::HUP | PollFlags::ERR)
        && server.handle_readable(peer).is_err()
    {
        return Ok(());
    }
    while let Some(message) = server.next_request(peer) {
        daemon.dispatch(server, peer, message)?;
        daemon.settle(server, Instant::now());
    }
    server.reap(peer);
    Ok(())
}

fn wait(
    server: &Server,
    inbox: &NoticeInbox,
    daemon: &Daemon,
    wakeup: Option<Instant>,
) -> anyhow::Result<Readiness> {
    let peers = server.peer_ids();
    let mut fds = Vec::with_capacity(peers.len() + 6);
    fds.push(PollFd::new(server, PollFlags::IN));
    fds.push(PollFd::new(inbox, PollFlags::IN));
    fds.push(PollFd::new(&daemon.watch, PollFlags::IN));
    let agent_slot = daemon.agent.as_ref().map(|agent| {
        let mut flags = PollFlags::IN;
        flags.set(PollFlags::OUT, agent.wants_write());
        fds.push(PollFd::new(agent, flags));
        fds.len() - 1
    });
    let wayland_slot = daemon.wayland.connection().map(|splashes| {
        fds.push(PollFd::from_borrowed_fd(splashes.as_fd(), PollFlags::IN));
        fds.len() - 1
    });
    let crownotify_slot = daemon.notifications.fd().map(|fd| {
        let mut flags = PollFlags::IN;
        flags.set(PollFlags::OUT, daemon.notifications.wants_write());
        fds.push(PollFd::from_borrowed_fd(fd, flags));
        fds.len() - 1
    });
    let first_peer = fds.len();
    let mut polled = Vec::with_capacity(peers.len());
    for peer in peers {
        if let (Some(fd), Some(interest)) = (server.peer_fd(peer), server.peer_interest(peer)) {
            let mut flags = PollFlags::empty();
            flags.set(PollFlags::IN, interest.read);
            flags.set(PollFlags::OUT, interest.write);
            fds.push(PollFd::from_borrowed_fd(fd, flags));
            polled.push(peer);
        }
    }
    let timeout = wakeup
        .map(|at| Timespec::try_from(at.saturating_duration_since(Instant::now())))
        .transpose()
        .context("timeout out of range")?;
    match rustix::event::poll(&mut fds, timeout.as_ref()) {
        Ok(_) | Err(rustix::io::Errno::INTR) => {}
        Err(error) => return Err(std::io::Error::from(error)).context("poll failed"),
    }
    let events = |slot: Option<usize>| slot.map_or(PollFlags::empty(), |slot| fds[slot].revents());
    Ok(Readiness {
        listener: !fds[0].revents().is_empty(),
        notices: !fds[1].revents().is_empty(),
        watch: !fds[2].revents().is_empty(),
        agent: events(agent_slot),
        wayland: !events(wayland_slot).is_empty(),
        crownotify: !events(crownotify_slot).is_empty(),
        peers: polled
            .into_iter()
            .zip(&fds[first_peer..])
            .filter_map(|(peer, fd)| {
                Some((peer, fd.revents())).filter(|(_, events)| !events.is_empty())
            })
            .collect(),
    })
}

impl Daemon {
    /// Flushes what the connections queued.
    fn before_poll(&mut self, now: Instant) {
        if let Some(splashes) = self.wayland.connected()
            && let Err(error) = splashes.flush()
        {
            self.wayland.lost(now, &error);
        }
        self.notifications.flush();
        if let Some(agent) = &mut self.agent
            && let Err(error) = agent.flush()
        {
            log::warn!("{error}");
            self.agent = None;
            self.lifecycle.agent_lost(now);
        }
    }

    pub fn handle_notice(&mut self, server: &mut Server, notice: Notice, now: Instant) {
        match notice {
            Notice::SessionStarted(Ok(())) => self.lifecycle.session_started(),
            Notice::SessionStarted(Err(failure)) => self.lifecycle.session_failed(failure),
            Notice::SessionStopped => {
                self.session = None;
                self.lifecycle.stopped(now);
                for reply in self.pending_stops.drain(..) {
                    let _ = reply.send(server, Ok(&()));
                }
            }
            Notice::Sleep(sleeping) => self.lifecycle.prepare_for_sleep(sleeping),
            Notice::IdleMinutes(minutes) => self.lifecycle.set_idle_minutes(minutes, now),
            Notice::Diagnosed { reply, archive } => {
                self.diagnoses = self.diagnoses.saturating_sub(1);
                let _ = reply.send(server, archive.as_ref().map_err(RemoteError::handler));
            }
            Notice::NotificationAction { id, action } => {
                self.handle_user_action(UserAction::Invoked { id, action }, now);
            }
            Notice::NotificationClosed { id } => {
                self.handle_user_action(UserAction::Closed { id }, now)
            }
        }
    }

    fn handle_watch(&mut self, server: &mut Server, now: Instant) {
        let topics = match self.watch.drain() {
            Ok(topics) => topics,
            Err(error) => {
                log::warn!("lost the file watches: {error}");
                return;
            }
        };
        if topics
            .iter()
            .any(|topic| matches!(topic, Topic::Setup | Topic::SystemConfig))
        {
            self.reload_installation(server, now);
        }
        if topics.contains(&Topic::Apps) {
            self.sync_apps();
        }
    }

    fn handle_io(&mut self, agent: &PollFlags, wayland: bool, crownotify: bool, now: Instant) {
        if agent.intersects(PollFlags::IN | PollFlags::HUP | PollFlags::ERR)
            && let Some(link) = &mut self.agent
        {
            match link.receive() {
                Ok(records) => {
                    for record in records {
                        self.lifecycle.agent_record(record, now);
                    }
                }
                Err(error) => {
                    log::info!("{error}");
                    self.agent = None;
                    self.lifecycle.agent_lost(now);
                }
            }
        }
        if wayland && let Some(splashes) = self.wayland.connected() {
            match splashes.read() {
                Ok(()) => {
                    for package in splashes.take_closed() {
                        log::debug!("the splash of {package} closed");
                    }
                }
                Err(error) => self.wayland.lost(now, &error),
            }
        }
        if crownotify {
            for action in self.notifications.pump() {
                self.handle_user_action(action, now);
            }
        }
    }

    fn tick(&mut self, server: &mut Server, now: Instant) {
        self.lifecycle.tick(now);
        if let Some(splashes) = self.wayland.connected() {
            splashes.tick(now);
        }
        let known = self.lifecycle.persisted().gsf_id.clone();
        self.gsf_queries.answer_expired(server, known.as_ref(), now);
        if self.lifecycle.wants_agent() && self.agent.is_none() && now >= self.next_agent_attempt {
            match runtime::agent::AgentLink::connect(&self.env.paths.agent_socket()) {
                Ok(link) => {
                    log::info!("connected to crown-agent");
                    self.agent = Some(link);
                    self.lifecycle.agent_connected();
                }
                Err(_) => self.next_agent_attempt = now + AGENT_RETRY,
            }
        }
    }
}
