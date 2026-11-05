use std::os::fd::AsFd;
use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};

use super::{SplashError, Splashes};

/// Sandbox engine, app id: contract 10's window identity.
const SANDBOX_ENGINE: &str = "id.waydro.waydroid";
const APP_ID: &str = "waydroid";

/// A `wp_security_context_v1` listener. The compositor accepts Waydroid's
/// clients on its socket until this is dropped, which closes the close fd's
/// peer and removes the socket file.
pub struct SecurityContext {
    path: PathBuf,
    _close_peer: std::io::PipeWriter,
}

impl SecurityContext {
    pub fn socket(&self) -> &Path {
        &self.path
    }
}

impl Drop for SecurityContext {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

impl Splashes {
    /// Binds `path` and hands it to the compositor as Waydroid's sandboxed
    /// Wayland socket, tagged with `instance_id`.
    pub fn listen_security_context(
        &mut self,
        path: &Path,
        instance_id: &str,
    ) -> Result<SecurityContext, SplashError> {
        let manager = self
            .state
            .globals
            .security_context
            .as_ref()
            .ok_or(SplashError::Missing("wp_security_context_manager_v1"))?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        match std::fs::remove_file(path) {
            Err(error) if error.kind() != std::io::ErrorKind::NotFound => return Err(error.into()),
            _ => {}
        }
        let listener = UnixListener::bind(path)?;
        let (close_fd, close_peer) = std::io::pipe()?;
        let context =
            manager.create_listener(listener.as_fd(), close_fd.as_fd(), &self.queue.handle(), ());
        context.set_sandbox_engine(SANDBOX_ENGINE.into());
        context.set_app_id(APP_ID.into());
        context.set_instance_id(instance_id.into());
        context.commit();
        context.destroy();
        self.connection.flush()?;
        Ok(SecurityContext {
            path: path.to_owned(),
            _close_peer: close_peer,
        })
    }
}
