//! The daemon's end of `/data/crownos/agent.sock`: agent-wire records over a
//! non-blocking Unix stream, pumped by the daemon's poll loop.

use std::io::{self, ErrorKind, Read, Write};
use std::os::fd::{AsFd, BorrowedFd};
use std::os::unix::net::UnixStream;
use std::path::Path;

use agent_wire::{Decoded, MAX_RECORD_LEN, Record, WireError};

/// Bytes read per `read` call.
const READ_CHUNK: usize = 16 * 1024;
/// An agent that leaves this much unread is not reading at all.
const MAX_OUTBOX: usize = 1024 * 1024;

#[derive(Debug, thiserror::Error)]
pub enum AgentError {
    #[error("agent socket: {0}")]
    Io(#[from] io::Error),
    #[error("agent sent a bad record: {0}")]
    Wire(#[from] WireError),
    #[error("the agent closed the connection")]
    Closed,
    #[error("the agent stopped reading")]
    Backlog,
}

pub struct AgentLink {
    stream: UnixStream,
    inbox: Vec<u8>,
    outbox: Vec<u8>,
}

impl AgentLink {
    pub fn connect(path: &Path) -> io::Result<Self> {
        Self::from_stream(UnixStream::connect(path)?)
    }

    pub fn from_stream(stream: UnixStream) -> io::Result<Self> {
        stream.set_nonblocking(true)?;
        Ok(Self {
            stream,
            inbox: Vec::new(),
            outbox: Vec::new(),
        })
    }

    /// Queues `record` and writes as much as the socket takes.
    pub fn send(&mut self, record: &Record) -> Result<(), AgentError> {
        self.outbox.extend_from_slice(&agent_wire::encode(record)?);
        if self.outbox.len() > MAX_OUTBOX {
            return Err(AgentError::Backlog);
        }
        self.flush()
    }

    pub fn flush(&mut self) -> Result<(), AgentError> {
        while !self.outbox.is_empty() {
            match self.stream.write(&self.outbox) {
                Ok(0) => return Err(AgentError::Closed),
                Ok(written) => {
                    self.outbox.drain(..written);
                }
                Err(error) if error.kind() == ErrorKind::WouldBlock => break,
                Err(error) if error.kind() == ErrorKind::Interrupted => {}
                Err(error) => return Err(error.into()),
            }
        }
        Ok(())
    }

    pub fn wants_write(&self) -> bool {
        !self.outbox.is_empty()
    }

    /// Reads what the socket holds and returns every whole record in it.
    pub fn receive(&mut self) -> Result<Vec<Record>, AgentError> {
        let mut chunk = [0; READ_CHUNK];
        loop {
            match self.stream.read(&mut chunk) {
                Ok(0) => return Err(AgentError::Closed),
                Ok(read) => self.inbox.extend_from_slice(&chunk[..read]),
                Err(error) if error.kind() == ErrorKind::WouldBlock => break,
                Err(error) if error.kind() == ErrorKind::Interrupted => {}
                Err(error) => return Err(error.into()),
            }
            if self.inbox.len() > MAX_RECORD_LEN + READ_CHUNK {
                break;
            }
        }
        self.decode()
    }

    fn decode(&mut self) -> Result<Vec<Record>, AgentError> {
        let mut records = Vec::new();
        let mut used = 0;
        while let Decoded::Record(record, length) = agent_wire::decode(&self.inbox[used..])? {
            records.push(record);
            used += length;
        }
        self.inbox.drain(..used);
        Ok(records)
    }
}

impl AsFd for AgentLink {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.stream.as_fd()
    }
}
