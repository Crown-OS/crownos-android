//! Worker threads telling the event loop about things it must act on.

use std::io::{self, PipeReader, PipeWriter, Read, Write};
use std::os::fd::{AsFd, BorrowedFd};
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, channel};

use runtime::lifecycle::BootFailure;

use crate::deferred::ReplyTo;

const WAKE_BATCH: usize = 64;

pub enum Notice {
    /// The host worker finished starting a session.
    SessionStarted(Result<(), BootFailure>),
    /// The host worker finished the stop sequence.
    SessionStopped,
    /// logind: the machine is about to sleep (`true`) or woke up.
    Sleep(bool),
    /// `android.ron` changed the idle timeout.
    IdleMinutes(u32),
    /// A diagnostics archive was written, or could not be.
    Diagnosed {
        reply: ReplyTo,
        archive: Result<String, String>,
    },
    /// The freedesktop notification server reported an action.
    NotificationAction {
        id: u32,
        action: String,
    },
    NotificationClosed {
        id: u32,
    },
}

/// Cloneable sender that also wakes the event loop's `poll`.
#[derive(Clone)]
pub struct Notifier {
    notices: Sender<Notice>,
    wake: Arc<PipeWriter>,
}

impl Notifier {
    pub fn notify(&self, notice: Notice) {
        if self.notices.send(notice).is_ok() {
            let _ = (&*self.wake).write(&[1]);
        }
    }
}

/// The event loop's end: poll it for readability, then [`NoticeInbox::drain`].
pub struct NoticeInbox {
    notices: Receiver<Notice>,
    wake: PipeReader,
}

impl NoticeInbox {
    pub fn drain(&mut self) -> Vec<Notice> {
        let _ = self.wake.read(&mut [0; WAKE_BATCH]);
        self.notices.try_iter().collect()
    }
}

impl AsFd for NoticeInbox {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.wake.as_fd()
    }
}

pub fn notice_channel() -> io::Result<(Notifier, NoticeInbox)> {
    let (reader, writer) = io::pipe()?;
    rustix::io::ioctl_fionbio(&reader, true)?;
    let (notices, receiver) = channel();
    Ok((
        Notifier {
            notices,
            wake: Arc::new(writer),
        },
        NoticeInbox {
            notices: receiver,
            wake: reader,
        },
    ))
}
