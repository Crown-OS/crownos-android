# crown-agent wire format (v1, frozen)

One record format serves both agent sockets, so the Java agent, the C++ HWC and the Rust daemon each implement one codec.

## Framing

All integers are little-endian.

| Field | Type | Notes |
|---|---|---|
| `len` | u32 | bytes after this field, at most 65536 |
| `type` | u16 | message type below |
| `flags` | u16 | 0 for now |
| payload | | fields in the order listed |

Field types:
- `u32` and `i32` are 4 bytes each.
- `str` is a u16 byte length followed by UTF-8 bytes.
- `rect` is four `i32` values: left, top, right, bottom.
- `list<T>` is a u16 count followed by the items.

A peer drops the connection on an unknown type, an oversized record or a malformed field. Golden vectors live in `agent-wire-vectors.txt` (hex, one record per line), and all three codecs test against them.

## Handshake

Both sides send `Hello` first and close the connection when `wire` differs.

| Type | Name | Payload |
|---|---|---|
| 0x0001 | `Hello` | `wire: u32`, `bundle: str` |

## HWC → agent (`/dev/socket/crown_agent`)

| Type | Name | Payload |
|---|---|---|
| 0x0101 | `SetSize` | `task: u32`, `width: u32`, `height: u32`, `serial: u32` |
| 0x0102 | `Focus` | `task: u32` |
| 0x0103 | `Close` | `task: u32` |
| 0x0104 | `Back` | `task: u32` |
| 0x0105 | `Visibility` | `task: u32`, `visible: u32` |

## Agent → HWC

| Type | Name | Payload |
|---|---|---|
| 0x0201 | `TaskInfo` | `task: u32`, `package: str`, `label: str`, `color: u32`, `resizable: u32`, `min_width: u32`, `min_height: u32` |
| 0x0202 | `SlotApplied` | `task: u32`, `serial: u32`, `slot: rect` |
| 0x0203 | `TaskRemoved` | `task: u32` |
| 0x0204 | `Activate` | `task: u32`, `token: str` |

## Daemon → agent (`/data/crownos/agent.sock`)

| Type | Name | Payload |
|---|---|---|
| 0x0301 | `Launch` | `package: str`, `width: u32`, `height: u32`, `token: str` |
| 0x0302 | `ListTasks` | none |
| 0x0303 | `QueryKeepAlive` | none |
| 0x0304 | `QueryGsfId` | none |
| 0x0305 | `Shutdown` | none |
| 0x0306 | `OpenUri` | `uri: str` (`market:` or `https:` only) |
| 0x0307 | `InstallPackage` | `path: str` (a plain file name under `/data/crownos/inbox/`) |

## Agent → daemon

| Type | Name | Payload |
|---|---|---|
| 0x0401 | `Ready` | `boot_ms: u32` |
| 0x0402 | `TaskOpened` | `task: u32`, `package: str` |
| 0x0403 | `TaskClosed` | `task: u32` |
| 0x0404 | `Tasks` | `list<u32>` |
| 0x0405 | `KeepAlive` | `reasons: u32` (bit 0 audio, 1 call, 2 install, 3 download, 4 foreground service) |
| 0x0406 | `GsfId` | `id: str`, empty when unavailable |
| 0x0407 | `LaunchFailed` | `package: str`, `reason: str` |
| 0x0408 | `InstallResult` | `path: str`, `ok: u32`, `message: str` |

`crates/agent-wire` is the source of truth; this table must match it and `agent-wire-vectors.txt`.
