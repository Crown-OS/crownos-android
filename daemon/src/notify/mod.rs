//! The resident setup card and one-off notices, through crownotify's
//! crownos-ipc service or, when it is not running, freedesktop
//! notifications.

pub mod content;
mod crownotify;
mod freedesktop;

use std::os::fd::{AsFd, BorrowedFd};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use runtime::host::Buses;

use crate::notice::Notifier;
pub use content::{Content, SetupNotice, setup_card};
use crownotify::crownotify as proto;
use freedesktop::{Freedesktop, Residence};

const APP_NAME: &str = "Android apps";
const ICON: &str = "android";
/// How long a call to crownotify may take.
const CALL_TIMEOUT: Duration = Duration::from_secs(2);
/// How long crownotify shows a one-off notice, in milliseconds.
const NOTICE_TIMEOUT_MS: u32 = 8000;

/// What the user did with a notification.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UserAction {
    Invoked { id: u32, action: String },
    Closed { id: u32 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Channel {
    Crownotify,
    Freedesktop,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Posted {
    id: u32,
    channel: Channel,
}

pub struct Notifications {
    notifier: Notifier,
    buses: Option<Arc<Buses>>,
    /// Where crownotify's socket lives, `$XDG_RUNTIME_DIR/crownos`.
    socket_dir: PathBuf,
    crownotify: Option<proto::Client>,
    freedesktop: Option<Freedesktop>,
    setup: Option<(Posted, SetupNotice)>,
}

impl Notifications {
    /// `buses` `None` turns the freedesktop fallback off.
    pub fn new(notifier: Notifier, buses: Option<Arc<Buses>>, socket_dir: PathBuf) -> Self {
        Self {
            notifier,
            buses,
            socket_dir,
            crownotify: None,
            freedesktop: None,
            setup: None,
        }
    }

    /// Shows `notice` on the one setup card, replacing what it said.
    pub fn show_setup(&mut self, notice: SetupNotice) {
        if self
            .setup
            .as_ref()
            .is_some_and(|(_, shown)| *shown == notice)
        {
            return;
        }
        let card = setup_card(&notice);
        let replaces = self.setup.as_ref().map(|(posted, _)| *posted);
        match self.post_progress(replaces, &card) {
            Some(posted) => self.setup = Some((posted, notice)),
            None => log::warn!("could not show the setup notification: {}", card.summary),
        }
    }

    /// The setup card's id, to match actions against.
    pub fn setup_id(&self) -> Option<u32> {
        self.setup.as_ref().map(|(posted, _)| posted.id)
    }

    /// Whether the setup card offers actions the user has not acted on.
    pub fn awaits_user(&self) -> bool {
        self.setup
            .as_ref()
            .is_some_and(|(_, notice)| !setup_card(notice).actions.is_empty())
    }

    pub fn forget(&mut self, id: u32) {
        if self.setup_id() == Some(id) {
            self.setup = None;
        }
    }

    /// Takes the setup card down.
    pub fn close_setup(&mut self) {
        let Some((posted, _)) = self.setup.take() else {
            return;
        };
        match posted.channel {
            Channel::Crownotify => {
                if let Some(client) = &mut self.crownotify {
                    let _ = client.close_notification(posted.id);
                }
            }
            Channel::Freedesktop => {
                if let Some(freedesktop) = &self.freedesktop {
                    freedesktop.close(posted.id);
                }
            }
        }
    }

    /// A one-off notice that expires by itself.
    pub fn post(&mut self, card: &Content) {
        if let Some(client) = self.crownotify() {
            let sent = client
                .send_general_notification(
                    APP_NAME.into(),
                    card.summary.clone(),
                    card.body.clone(),
                    NOTICE_TIMEOUT_MS,
                    card.action_list(),
                )
                .and_then(|pending| client.wait(pending, Some(CALL_TIMEOUT)));
            match sent {
                Ok(_) => return,
                Err(error) => self.lost_crownotify(&error),
            }
        }
        let notifier = self.notifier.clone();
        if let Some(freedesktop) = self.freedesktop()
            && let Err(error) =
                freedesktop.notify(0, APP_NAME, ICON, card, Residence::Transient, &notifier)
        {
            log::warn!("could not post a notification: {error}");
        }
    }

    /// Reads crownotify's events after its socket polled readable.
    pub fn pump(&mut self) -> Vec<UserAction> {
        let Some(client) = &mut self.crownotify else {
            return Vec::new();
        };
        if let Err(error) = client.handle_readable() {
            self.lost_crownotify(&error);
            return Vec::new();
        }
        let mut actions = Vec::new();
        while let Some(event) = client.next_event() {
            actions.push(match event {
                proto::Event::ActionInvoked(invoked) => UserAction::Invoked {
                    id: invoked.id,
                    action: invoked.action,
                },
                proto::Event::NotificationClosed(closed) => UserAction::Closed { id: closed.id },
            });
        }
        if client.is_closed() {
            self.crownotify = None;
        }
        actions
    }

    /// crownotify's socket, while connected.
    pub fn fd(&self) -> Option<BorrowedFd<'_>> {
        self.crownotify.as_ref().map(AsFd::as_fd)
    }

    pub fn wants_write(&self) -> bool {
        self.crownotify
            .as_ref()
            .is_some_and(|client| client.interest().write)
    }

    pub fn flush(&mut self) {
        if let Some(client) = &mut self.crownotify
            && let Err(error) = client.handle_writable()
        {
            self.lost_crownotify(&error);
        }
    }

    fn post_progress(&mut self, replaces: Option<Posted>, card: &Content) -> Option<Posted> {
        let replaces_on = |channel| {
            replaces
                .filter(|posted| posted.channel == channel)
                .map_or(0, |posted| posted.id)
        };
        if let Some(client) = self.crownotify() {
            let sent = client
                .send_progress_notification(
                    replaces_on(Channel::Crownotify),
                    APP_NAME.into(),
                    ICON.into(),
                    card.summary.clone(),
                    card.body.clone(),
                    card.percent,
                    card.action_list(),
                )
                .and_then(|pending| client.wait(pending, Some(CALL_TIMEOUT)));
            match sent {
                Ok(id) => {
                    return Some(Posted {
                        id,
                        channel: Channel::Crownotify,
                    });
                }
                Err(error) => self.lost_crownotify(&error),
            }
        }
        let notifier = self.notifier.clone();
        let replaces_id = replaces_on(Channel::Freedesktop);
        let freedesktop = self.freedesktop()?;
        match freedesktop.notify(
            replaces_id,
            APP_NAME,
            ICON,
            card,
            Residence::Resident,
            &notifier,
        ) {
            Ok(id) => Some(Posted {
                id,
                channel: Channel::Freedesktop,
            }),
            Err(error) => {
                log::warn!("freedesktop notifications failed: {error}");
                None
            }
        }
    }

    fn crownotify(&mut self) -> Option<&mut proto::Client> {
        if self.crownotify.is_none() {
            self.crownotify = connect_crownotify(&self.socket_dir);
        }
        self.crownotify.as_mut()
    }

    fn freedesktop(&mut self) -> Option<&mut Freedesktop> {
        if self.freedesktop.is_none() {
            let session = self.buses.as_ref()?.session().ok()?.clone();
            self.freedesktop = Some(Freedesktop::connect(session));
        }
        self.freedesktop.as_mut()
    }

    fn lost_crownotify(&mut self, error: &crownos_ipc::Error) {
        log::info!("crownotify is not reachable: {error}");
        self.crownotify = None;
    }
}

fn connect_crownotify(socket_dir: &Path) -> Option<proto::Client> {
    let mut client = proto::Client::connect_in(socket_dir).ok()?;
    client
        .subscribe_blocking::<proto::ActionInvoked>(Some(CALL_TIMEOUT))
        .ok()?;
    client
        .subscribe_blocking::<proto::NotificationClosed>(Some(CALL_TIMEOUT))
        .ok()?;
    Some(client)
}
