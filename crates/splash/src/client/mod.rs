mod clipboard;
mod dispatch;
mod globals;
mod security_context;
mod window;

use std::io;
use std::os::fd::{AsFd, BorrowedFd};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::{Duration, Instant};

use wayland_client::backend::WaylandError;
use wayland_client::globals::{BindError, GlobalError, registry_queue_init};
use wayland_client::protocol::wl_output::WlOutput;
use wayland_client::{ConnectError, Connection, DispatchError, EventQueue};

pub use security_context::SecurityContext;

use crate::color::Tone;
use crate::{LogicalSize, PhysicalSize, Scale, icon, launch_size};
use clipboard::Selection;
use globals::Globals;
use window::{Content, SplashWindow, WindowId};

/// A splash closes by itself this long after it opened.
const SPLASH_LIFETIME: Duration = Duration::from_secs(120);
/// A failed launch shows its reason this long.
const FAILURE_LIFETIME: Duration = Duration::from_secs(3);

#[derive(Debug, thiserror::Error)]
pub enum SplashError {
    #[error("no Wayland compositor: {0}")]
    Connect(#[from] ConnectError),
    #[error("Wayland globals: {0}")]
    Globals(#[from] GlobalError),
    #[error("Wayland bind: {0}")]
    Bind(#[from] BindError),
    #[error("Wayland connection: {0}")]
    Connection(#[from] WaylandError),
    #[error("Wayland dispatch: {0}")]
    Dispatch(#[from] DispatchError),
    #[error("the compositor lacks {0}")]
    Missing(&'static str),
    #[error("{0}")]
    Io(#[from] io::Error),
}

/// One splash to show.
pub struct SplashRequest<'a> {
    pub package: &'a str,
    /// The app's name, the window title.
    pub title: &'a str,
    /// The app's PNG icon. It is decoded now, not before.
    pub icon: Option<&'a Path>,
}

/// The daemon's Wayland connection: splash windows, the security context
/// for Waydroid and the clipboard.
pub struct Splashes {
    connection: Connection,
    queue: EventQueue<ClientState>,
    state: ClientState,
}

pub struct ClientState {
    globals: Globals,
    tone: Tone,
    windows: Vec<SplashWindow>,
    next_window: u32,
    primary_output: Option<WlOutput>,
    output_size: Option<LogicalSize>,
    /// The last preferred scale any surface or output reported.
    scale: Scale,
    closed: Vec<String>,
    selection: Option<Selection>,
}

impl Splashes {
    /// Takes over a connected compositor socket and binds the globals.
    pub fn connect(socket: UnixStream, tone: Tone) -> Result<Self, SplashError> {
        let connection = Connection::from_socket(socket)?;
        let (list, mut queue) = registry_queue_init::<ClientState>(&connection)?;
        let handle = queue.handle();
        let globals = Globals::bind(&list, &handle)?;
        let primary_output = globals::bind_primary_output(&list, &handle);
        let mut state = ClientState {
            globals,
            tone,
            windows: Vec::new(),
            next_window: 0,
            primary_output,
            output_size: None,
            scale: Scale::ONE,
            closed: Vec::new(),
            selection: None,
        };
        queue.roundtrip(&mut state)?;
        Ok(Self {
            connection,
            queue,
            state,
        })
    }

    /// Opens a splash for `request.package`, unless one is open already.
    pub fn open(&mut self, request: &SplashRequest<'_>, now: Instant) -> Result<(), SplashError> {
        if self.is_open(request.package) {
            return Ok(());
        }
        self.state.next_window = self.state.next_window.wrapping_add(1);
        let content = Content {
            package: request.package.to_owned(),
            title: request.title.to_owned(),
            icon: request.icon.and_then(icon::load),
            size: launch_size(self.state.output_size),
            scale: self.state.scale,
        };
        let window = SplashWindow::open(
            &self.state.globals,
            &self.queue.handle(),
            WindowId(self.state.next_window),
            content,
            self.state.tone,
            now + SPLASH_LIFETIME,
        );
        self.state.windows.push(window);
        self.connection.flush()?;
        Ok(())
    }

    /// Shows why `package` did not open and closes its splash shortly after.
    pub fn fail(&mut self, package: &str, reason: &str, now: Instant) -> Result<(), SplashError> {
        let handle = self.queue.handle();
        let state = &mut self.state;
        if let Some(window) = state
            .windows
            .iter_mut()
            .find(|window| window.package == package)
        {
            window.fail(reason, now + FAILURE_LIFETIME, &state.globals, &handle)?;
        }
        Ok(())
    }

    pub fn close(&mut self, package: &str) {
        self.state.close_where(|window| window.package == package);
    }

    pub fn is_open(&self, package: &str) -> bool {
        self.state
            .windows
            .iter()
            .any(|window| window.package == package)
    }

    /// Splashes that closed since the last call, by package.
    pub fn take_closed(&mut self) -> Vec<String> {
        std::mem::take(&mut self.state.closed)
    }

    /// Whether anything needs the connection to stay: a splash or a
    /// clipboard selection only this process can serve.
    pub fn is_busy(&self) -> bool {
        !self.state.windows.is_empty() || self.state.selection.is_some()
    }

    /// The size of a launched app's window in Android pixels.
    pub fn launch_pixels(&self) -> PhysicalSize {
        launch_size(self.state.output_size).to_physical(self.state.scale)
    }

    pub fn next_deadline(&self) -> Option<Instant> {
        self.state
            .windows
            .iter()
            .map(|window| window.closes_at)
            .min()
    }

    pub fn tick(&mut self, now: Instant) {
        self.state.close_where(|window| window.closes_at <= now);
    }

    /// Dispatches queued events and flushes requests; call before polling.
    pub fn flush(&mut self) -> Result<(), SplashError> {
        self.queue.dispatch_pending(&mut self.state)?;
        match self.connection.flush() {
            Err(WaylandError::Io(error)) if error.kind() == io::ErrorKind::WouldBlock => Ok(()),
            other => Ok(other?),
        }
    }

    /// Reads and dispatches events after the descriptor polled readable.
    pub fn read(&mut self) -> Result<(), SplashError> {
        if let Some(guard) = self.queue.prepare_read() {
            match guard.read() {
                Err(WaylandError::Io(error)) if error.kind() == io::ErrorKind::WouldBlock => {}
                other => {
                    other?;
                }
            }
        }
        self.flush()
    }
}

impl AsFd for Splashes {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.connection.as_fd()
    }
}

impl ClientState {
    fn close_where(&mut self, closes: impl Fn(&SplashWindow) -> bool) {
        let (closing, open): (Vec<_>, Vec<_>) = std::mem::take(&mut self.windows)
            .into_iter()
            .partition(|window| closes(window));
        self.windows = open;
        for window in closing {
            self.closed.push(window.package.clone());
            window.destroy();
        }
    }

    fn window(&mut self, id: WindowId) -> Option<&mut SplashWindow> {
        self.windows.iter_mut().find(|window| window.id == id)
    }

    /// A splash that cannot draw is closed rather than left blank.
    fn note(&mut self, id: WindowId, result: Result<(), SplashError>) {
        if let Err(error) = result {
            log::warn!("closing a splash that cannot draw: {error}");
            self.close_where(|window| window.id == id);
        }
    }
}
