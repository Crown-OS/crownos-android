//! Replies sent after the request returned: diagnostics archives and the
//! Google services id, which wait for a worker or the agent.

use std::time::Instant;

use crownos_ipc::{Error, PeerId, RemoteError, Server, postcard};
use serde::Serialize;

/// Where a deferred reply goes.
#[derive(Clone, Copy, Debug)]
pub struct ReplyTo {
    pub peer: PeerId,
    pub req_id: u32,
}

impl ReplyTo {
    /// Fails when the peer is gone or not reading.
    pub fn send<T: Serialize>(
        self,
        server: &mut Server,
        result: Result<&T, RemoteError>,
    ) -> Result<(), Error> {
        match result {
            Ok(value) => server.reply(
                self.peer,
                self.req_id,
                &postcard::to_stdvec(value)?,
                Vec::new(),
            ),
            Err(error) => server.reply_err(self.peer, self.req_id, &error),
        }
    }
}

/// `google_registration_id` requests waiting for the agent's answer.
#[derive(Default)]
pub struct GsfQueries {
    waiting: Vec<(ReplyTo, Instant)>,
}

impl GsfQueries {
    pub fn push(&mut self, reply: ReplyTo, deadline: Instant) {
        self.waiting.push((reply, deadline));
    }

    pub fn is_empty(&self) -> bool {
        self.waiting.is_empty()
    }

    pub fn next_deadline(&self) -> Option<Instant> {
        self.waiting.iter().map(|(_, deadline)| *deadline).min()
    }

    /// Answers every waiting request with `id`.
    pub fn answer_all(&mut self, server: &mut Server, id: Option<&String>) {
        for (reply, _) in self.waiting.drain(..) {
            let _ = reply.send(server, Ok(&id));
        }
    }

    /// Answers the requests past their deadline with `id`.
    pub fn answer_expired(&mut self, server: &mut Server, id: Option<&String>, now: Instant) {
        let (expired, waiting) = std::mem::take(&mut self.waiting)
            .into_iter()
            .partition(|(_, deadline)| *deadline <= now);
        self.waiting = waiting;
        for (reply, _) in expired {
            let _ = reply.send(server, Ok(&id));
        }
    }
}
