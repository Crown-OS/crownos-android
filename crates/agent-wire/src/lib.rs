//! The record format `crown-agent` speaks on both of its sockets: with the
//! CrownOS hwcomposer on `/dev/socket/crown_agent` and with `crown-androidd` on
//! `/data/crownos/agent.sock`. The Java agent and the C++ hwcomposer implement
//! the same format and test against `contracts/agent-wire-vectors.txt`.
//!
//! A record is `len: u32`, `type: u16`, `flags: u16`, then the payload, all
//! little-endian. `len` counts every byte after itself.

mod codec;
mod record;
mod util;

pub use codec::{Decoded, decode, encode};
pub use record::{KeepAliveReasons, Record, Rect};

/// Bumped for any change an older peer cannot read. Both sides send it in
/// [`Record::Hello`] and close the connection on a mismatch.
pub const WIRE_VERSION: u32 = 1;

/// Largest `len` a peer accepts.
pub const MAX_RECORD_LEN: usize = 64 * 1024;

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum WireError {
    #[error("record of {0} bytes exceeds the {MAX_RECORD_LEN} byte limit")]
    TooLong(usize),
    #[error("unknown record type {0:#06x}")]
    UnknownType(u16),
    #[error("record type {0:#06x} is malformed")]
    Malformed(u16),
    #[error("a string field is longer than 65535 bytes")]
    StringTooLong,
    #[error("a list field has more than 65535 items")]
    ListTooLong,
}
