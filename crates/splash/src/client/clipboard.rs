use std::io::Write;

use wayland_protocols::ext::data_control::v1::client::ext_data_control_device_v1::ExtDataControlDeviceV1;
use wayland_protocols::ext::data_control::v1::client::ext_data_control_source_v1::ExtDataControlSourceV1;

use super::{SplashError, Splashes};

const TEXT_TYPES: [&str; 3] = ["text/plain;charset=utf-8", "text/plain", "UTF8_STRING"];

/// Text the daemon offers as the clipboard selection until another client
/// takes it.
pub struct Selection {
    pub device: ExtDataControlDeviceV1,
    pub source: ExtDataControlSourceV1,
    pub text: String,
}

impl Selection {
    pub fn send(&self, fd: std::os::fd::OwnedFd) {
        let mut pipe = std::fs::File::from(fd);
        if let Err(error) = pipe.write_all(self.text.as_bytes()) {
            log::warn!("could not hand over the clipboard: {error}");
        }
    }

    pub fn destroy(self) {
        self.source.destroy();
        self.device.destroy();
    }
}

impl Splashes {
    /// Makes `text` the clipboard selection.
    pub fn copy_text(&mut self, text: String) -> Result<(), SplashError> {
        let globals = &self.state.globals;
        let manager = globals
            .data_control
            .as_ref()
            .ok_or(SplashError::Missing("ext_data_control_manager_v1"))?;
        let seat = globals
            .seat
            .as_ref()
            .ok_or(SplashError::Missing("wl_seat"))?;
        let queue = self.queue.handle();
        let device = manager.get_data_device(seat, &queue, ());
        let source = manager.create_data_source(&queue, ());
        for mime in TEXT_TYPES {
            source.offer(mime.into());
        }
        device.set_selection(Some(&source));
        if let Some(previous) = self.state.selection.replace(Selection {
            device,
            source,
            text,
        }) {
            previous.destroy();
        }
        self.connection.flush()?;
        Ok(())
    }
}
