use std::sync::OnceLock;

use serde::Serialize;
use serde::de::DeserializeOwned;
use zbus::blocking::Connection;
use zbus::zvariant::{DynamicType, OwnedValue};

use super::HostError;

const PROPERTIES: &str = "org.freedesktop.DBus.Properties";

/// The system and session bus, each connected on first use.
#[derive(Default)]
pub struct Buses {
    system: OnceLock<Connection>,
    session: OnceLock<Connection>,
}

impl Buses {
    pub fn system(&self) -> Result<&Connection, HostError> {
        connected(&self.system, Connection::system)
    }

    pub fn session(&self) -> Result<&Connection, HostError> {
        connected(&self.session, Connection::session)
    }
}

fn connected(
    slot: &OnceLock<Connection>,
    connect: fn() -> zbus::Result<Connection>,
) -> Result<&Connection, HostError> {
    if let Some(connection) = slot.get() {
        return Ok(connection);
    }
    let connection = connect()?;
    Ok(slot.get_or_init(|| connection))
}

/// One method call and its decoded reply.
pub(super) fn call<B, R>(
    connection: &Connection,
    target: (&str, &str, &str),
    method: &str,
    body: &B,
) -> Result<R, HostError>
where
    B: Serialize + DynamicType,
    R: DeserializeOwned + zbus::zvariant::Type,
{
    let (destination, path, interface) = target;
    let reply = connection.call_method(Some(destination), path, Some(interface), method, body)?;
    Ok(reply.body().deserialize()?)
}

/// One property, read without building a caching proxy.
pub(super) fn property<T>(
    connection: &Connection,
    target: (&str, &str, &str),
    name: &str,
) -> Result<T, HostError>
where
    T: TryFrom<OwnedValue>,
    T::Error: Into<zbus::zvariant::Error>,
{
    let (destination, path, interface) = target;
    let value: OwnedValue = call(
        connection,
        (destination, path, PROPERTIES),
        "Get",
        &(interface, name),
    )?;
    T::try_from(value).map_err(|error| HostError::Value(error.into()))
}
