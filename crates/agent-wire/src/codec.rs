use crate::util::bytes::{Reader, Writer};
use crate::{KeepAliveReasons, MAX_RECORD_LEN, Record, Rect, WireError};

/// Header bytes after `len`: `type` and `flags`.
const HEADER_AFTER_LEN: usize = 4;

/// Serializes `record` with its header.
pub fn encode(record: &Record) -> Result<Vec<u8>, WireError> {
    let payload = payload(record)?;
    let len = payload.len() + HEADER_AFTER_LEN;
    if len > MAX_RECORD_LEN {
        return Err(WireError::TooLong(len));
    }
    let mut bytes = Vec::with_capacity(4 + len);
    bytes.extend_from_slice(&(len as u32).to_le_bytes());
    bytes.extend_from_slice(&record.kind().to_le_bytes());
    bytes.extend_from_slice(&0u16.to_le_bytes());
    bytes.extend_from_slice(&payload);
    Ok(bytes)
}

/// What [`decode`] found at the start of a buffer.
#[derive(Debug, PartialEq, Eq)]
pub enum Decoded {
    /// A whole record and how many bytes it used.
    Record(Record, usize),
    /// The buffer holds only part of a record. Read more and try again.
    Incomplete,
}

/// Reads one record from the start of `bytes`. A peer that gets an error drops
/// the connection.
pub fn decode(bytes: &[u8]) -> Result<Decoded, WireError> {
    let Some(len) = bytes
        .first_chunk::<4>()
        .map(|len| u32::from_le_bytes(*len) as usize)
    else {
        return Ok(Decoded::Incomplete);
    };
    if len > MAX_RECORD_LEN {
        return Err(WireError::TooLong(len));
    }
    if len < HEADER_AFTER_LEN {
        return Err(WireError::Malformed(0));
    }
    let Some(body) = bytes.get(4..4 + len) else {
        return Ok(Decoded::Incomplete);
    };
    let kind = u16::from_le_bytes([body[0], body[1]]);
    let record = parse(kind, &body[HEADER_AFTER_LEN..])?;
    Ok(Decoded::Record(record, 4 + len))
}

fn payload(record: &Record) -> Result<Vec<u8>, WireError> {
    let mut w = Writer::default();
    match record {
        Record::Hello { wire, bundle } => {
            w.u32(*wire).str(bundle)?;
        }
        Record::SetSize {
            task,
            width,
            height,
            serial,
        } => {
            w.u32(*task).u32(*width).u32(*height).u32(*serial);
        }
        Record::Focus { task }
        | Record::Close { task }
        | Record::Back { task }
        | Record::TaskRemoved { task }
        | Record::TaskClosed { task } => {
            w.u32(*task);
        }
        Record::Visibility { task, visible } => {
            w.u32(*task).bool(*visible);
        }
        Record::TaskInfo {
            task,
            package,
            label,
            color,
            resizable,
            min_width,
            min_height,
        } => {
            w.u32(*task).str(package)?.str(label)?;
            w.u32(*color)
                .bool(*resizable)
                .u32(*min_width)
                .u32(*min_height);
        }
        Record::SlotApplied { task, serial, slot } => {
            w.u32(*task).u32(*serial);
            w.i32(slot.left)
                .i32(slot.top)
                .i32(slot.right)
                .i32(slot.bottom);
        }
        Record::Activate { task, token } => {
            w.u32(*task).str(token)?;
        }
        Record::Launch {
            package,
            width,
            height,
            token,
        } => {
            w.str(package)?.u32(*width).u32(*height).str(token)?;
        }
        Record::ListTasks | Record::QueryKeepAlive | Record::QueryGsfId | Record::Shutdown => {}
        Record::OpenUri { uri } => {
            w.str(uri)?;
        }
        Record::InstallPackage { path } => {
            w.str(path)?;
        }
        Record::Ready { boot_ms } => {
            w.u32(*boot_ms);
        }
        Record::TaskOpened { task, package } => {
            w.u32(*task).str(package)?;
        }
        Record::Tasks { tasks } => {
            w.u32_list(tasks)?;
        }
        Record::KeepAlive { reasons } => {
            w.u32(reasons.0);
        }
        Record::GsfId { id } => {
            w.str(id)?;
        }
        Record::LaunchFailed { package, reason } => {
            w.str(package)?.str(reason)?;
        }
        Record::InstallResult { path, ok, message } => {
            w.str(path)?.bool(*ok).str(message)?;
        }
    }
    Ok(w.0)
}

fn parse(kind: u16, payload: &[u8]) -> Result<Record, WireError> {
    let mut r = Reader(payload);
    let record = parse_fields(kind, &mut r).ok_or(WireError::Malformed(kind))??;
    if r.is_empty() {
        Ok(record)
    } else {
        Err(WireError::Malformed(kind))
    }
}

/// `None` is a short or invalid payload, `Some(Err)` an unknown type.
fn parse_fields(kind: u16, r: &mut Reader<'_>) -> Option<Result<Record, WireError>> {
    let record = match kind {
        0x0001 => Record::Hello {
            wire: r.u32()?,
            bundle: r.str()?,
        },
        0x0101 => Record::SetSize {
            task: r.u32()?,
            width: r.u32()?,
            height: r.u32()?,
            serial: r.u32()?,
        },
        0x0102 => Record::Focus { task: r.u32()? },
        0x0103 => Record::Close { task: r.u32()? },
        0x0104 => Record::Back { task: r.u32()? },
        0x0105 => Record::Visibility {
            task: r.u32()?,
            visible: r.bool()?,
        },
        0x0201 => Record::TaskInfo {
            task: r.u32()?,
            package: r.str()?,
            label: r.str()?,
            color: r.u32()?,
            resizable: r.bool()?,
            min_width: r.u32()?,
            min_height: r.u32()?,
        },
        0x0202 => Record::SlotApplied {
            task: r.u32()?,
            serial: r.u32()?,
            slot: Rect {
                left: r.i32()?,
                top: r.i32()?,
                right: r.i32()?,
                bottom: r.i32()?,
            },
        },
        0x0203 => Record::TaskRemoved { task: r.u32()? },
        0x0204 => Record::Activate {
            task: r.u32()?,
            token: r.str()?,
        },
        0x0301 => Record::Launch {
            package: r.str()?,
            width: r.u32()?,
            height: r.u32()?,
            token: r.str()?,
        },
        0x0302 => Record::ListTasks,
        0x0303 => Record::QueryKeepAlive,
        0x0304 => Record::QueryGsfId,
        0x0305 => Record::Shutdown,
        0x0306 => Record::OpenUri { uri: r.str()? },
        0x0307 => Record::InstallPackage { path: r.str()? },
        0x0401 => Record::Ready { boot_ms: r.u32()? },
        0x0402 => Record::TaskOpened {
            task: r.u32()?,
            package: r.str()?,
        },
        0x0403 => Record::TaskClosed { task: r.u32()? },
        0x0404 => Record::Tasks {
            tasks: r.u32_list()?,
        },
        0x0405 => Record::KeepAlive {
            reasons: KeepAliveReasons(r.u32()?),
        },
        0x0406 => Record::GsfId { id: r.str()? },
        0x0407 => Record::LaunchFailed {
            package: r.str()?,
            reason: r.str()?,
        },
        0x0408 => Record::InstallResult {
            path: r.str()?,
            ok: r.bool()?,
            message: r.str()?,
        },
        unknown => return Some(Err(WireError::UnknownType(unknown))),
    };
    Some(Ok(record))
}

#[cfg(test)]
mod tests;
