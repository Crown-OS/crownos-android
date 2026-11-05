//! `org.freedesktop.Notifications`, for when crownotify is not running.

use std::collections::HashMap;

use zbus::blocking::{Connection, MessageIterator};
use zbus::zvariant::Value;

use super::content::Content;
use crate::notice::{Notice, Notifier};

const TARGET: (&str, &str, &str) = (
    "org.freedesktop.Notifications",
    "/org/freedesktop/Notifications",
    "org.freedesktop.Notifications",
);
const SIGNALS: &str = "type='signal',interface='org.freedesktop.Notifications'";
/// `expire_timeout` that leaves the decision to the server.
const SERVER_DEFAULT_TIMEOUT: i32 = -1;
const NEVER_EXPIRES: i32 = 0;

pub struct Freedesktop {
    connection: Connection,
    following: bool,
}

/// Whether a card stays until it is replaced or closed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Residence {
    Resident,
    Transient,
}

impl Freedesktop {
    pub fn connect(connection: Connection) -> Self {
        Self {
            connection,
            following: false,
        }
    }

    pub fn notify(
        &mut self,
        replaces_id: u32,
        app_name: &str,
        icon: &str,
        content: &Content,
        residence: Residence,
        notifier: &Notifier,
    ) -> zbus::Result<u32> {
        self.follow(notifier);
        let mut hints: HashMap<&str, Value<'_>> = HashMap::new();
        if let Some(percent) = content.percent {
            hints.insert("value", Value::from(i32::from(percent)));
        }
        let expire_timeout = match residence {
            Residence::Resident => {
                hints.insert("resident", Value::from(true));
                NEVER_EXPIRES
            }
            Residence::Transient => SERVER_DEFAULT_TIMEOUT,
        };
        let (destination, path, interface) = TARGET;
        let reply = self.connection.call_method(
            Some(destination),
            path,
            Some(interface),
            "Notify",
            &(
                app_name,
                replaces_id,
                icon,
                content.summary.as_str(),
                content.body.as_str(),
                content.action_list(),
                hints,
                expire_timeout,
            ),
        )?;
        reply.body().deserialize()
    }

    pub fn close(&self, id: u32) {
        let (destination, path, interface) = TARGET;
        let closed = self.connection.call_method(
            Some(destination),
            path,
            Some(interface),
            "CloseNotification",
            &(id,),
        );
        if let Err(error) = closed {
            log::debug!("could not close notification {id}: {error}");
        }
    }

    /// Relays `ActionInvoked` and `NotificationClosed` to the event loop.
    fn follow(&mut self, notifier: &Notifier) {
        if self.following {
            return;
        }
        let signals = match MessageIterator::for_match_rule(SIGNALS, &self.connection, None) {
            Ok(signals) => signals,
            Err(error) => {
                log::warn!("cannot follow notification actions: {error}");
                return;
            }
        };
        let notifier = notifier.clone();
        let spawned = std::thread::Builder::new()
            .name("notifications".into())
            .spawn(move || {
                for message in signals.flatten() {
                    let header = message.header();
                    let notice = match header.member().map(|member| member.as_str()) {
                        Some("ActionInvoked") => message
                            .body()
                            .deserialize::<(u32, String)>()
                            .ok()
                            .map(|(id, action)| Notice::NotificationAction { id, action }),
                        Some("NotificationClosed") => message
                            .body()
                            .deserialize::<(u32, u32)>()
                            .ok()
                            .map(|(id, _)| Notice::NotificationClosed { id }),
                        _ => None,
                    };
                    if let Some(notice) = notice {
                        notifier.notify(notice);
                    }
                }
            });
        self.following = spawned.is_ok();
    }
}
