use std::time::Instant;

use tiny_skia::Pixmap;
use wayland_client::QueueHandle;
use wayland_client::protocol::wl_buffer::WlBuffer;
use wayland_client::protocol::wl_subsurface::WlSubsurface;
use wayland_client::protocol::wl_surface::WlSurface;
use wayland_protocols::wp::fractional_scale::v1::client::wp_fractional_scale_v1::WpFractionalScaleV1;
use wayland_protocols::wp::viewporter::client::wp_viewport::WpViewport;
use wayland_protocols::xdg::decoration::zv1::client::zxdg_toplevel_decoration_v1::{
    Mode, ZxdgToplevelDecorationV1,
};
use wayland_protocols::xdg::shell::client::xdg_surface::XdgSurface;
use wayland_protocols::xdg::shell::client::xdg_toplevel::XdgToplevel;

use super::globals::Globals;
use super::{ClientState, SplashError};
use crate::color::{self, Rgb, Tone};
use crate::icon::{self, Look};
use crate::layout::icon_place;
use crate::{LogicalSize, Scale, shm};

/// Ties every object of one splash to its window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WindowId(pub u32);

/// What one splash shows.
pub struct Content {
    pub package: String,
    pub title: String,
    pub icon: Option<Pixmap>,
    pub size: LogicalSize,
    pub scale: Scale,
}

pub struct SplashWindow {
    pub id: WindowId,
    pub package: String,
    pub closes_at: Instant,
    title: String,
    icon: Option<Pixmap>,
    background: Rgb,
    look: Look,
    size: LogicalSize,
    pending_size: Option<LogicalSize>,
    scale: Scale,
    configured: bool,
    drawn: Option<(LogicalSize, Scale, Look)>,
    surface: WlSurface,
    viewport: WpViewport,
    xdg_surface: XdgSurface,
    toplevel: XdgToplevel,
    decoration: Option<ZxdgToplevelDecorationV1>,
    fractional_scale: Option<WpFractionalScaleV1>,
    icon_surface: WlSurface,
    icon_viewport: WpViewport,
    icon_subsurface: WlSubsurface,
    background_buffer: Option<WlBuffer>,
    icon_buffer: Option<WlBuffer>,
}

impl SplashWindow {
    /// Creates the toplevel, marks it as the placeholder for its app and
    /// commits it without a buffer, so the compositor configures it.
    pub fn open(
        globals: &Globals,
        queue: &QueueHandle<ClientState>,
        id: WindowId,
        content: Content,
        tone: Tone,
        closes_at: Instant,
    ) -> Self {
        let app_id = android_protocol::app_id(&content.package);
        let surface = globals.compositor.create_surface(queue, id);
        let viewport = globals.viewporter.get_viewport(&surface, queue, ());
        let fractional_scale = globals
            .fractional_scale
            .as_ref()
            .map(|manager| manager.get_fractional_scale(&surface, queue, id));
        let icon_surface = globals.compositor.create_surface(queue, id);
        let icon_viewport = globals.viewporter.get_viewport(&icon_surface, queue, ());
        let icon_subsurface =
            globals
                .subcompositor
                .get_subsurface(&icon_surface, &surface, queue, ());
        let xdg_surface = globals.wm_base.get_xdg_surface(&surface, queue, id);
        let toplevel = xdg_surface.get_toplevel(queue, id);
        toplevel.set_app_id(app_id.clone());
        toplevel.set_title(content.title.clone());
        let decoration = globals.decorations.as_ref().map(|manager| {
            let decoration = manager.get_toplevel_decoration(&toplevel, queue, ());
            decoration.set_mode(Mode::ServerSide);
            decoration
        });
        if let Some(shell) = &globals.android_shell {
            shell.mark_placeholder(&toplevel, app_id);
        }
        surface.commit();
        let background = content.icon.as_ref().map_or_else(
            || color::background(&[], tone),
            |icon| color::background(icon.data(), tone),
        );
        Self {
            id,
            package: content.package,
            closes_at,
            title: content.title,
            icon: content.icon,
            background,
            look: Look::Normal,
            size: content.size,
            pending_size: None,
            scale: content.scale,
            configured: false,
            drawn: None,
            surface,
            viewport,
            xdg_surface,
            toplevel,
            decoration,
            fractional_scale,
            icon_surface,
            icon_viewport,
            icon_subsurface,
            background_buffer: None,
            icon_buffer: None,
        }
    }

    /// The size from an `xdg_toplevel.configure`; zero keeps ours.
    pub fn configure_toplevel(&mut self, width: i32, height: i32) {
        let side = |value: i32| u32::try_from(value).ok().filter(|value| *value > 0);
        self.pending_size = side(width)
            .zip(side(height))
            .map(|(width, height)| LogicalSize { width, height });
    }

    pub fn configure_surface(
        &mut self,
        serial: u32,
        globals: &Globals,
        queue: &QueueHandle<ClientState>,
    ) -> Result<(), SplashError> {
        self.xdg_surface.ack_configure(serial);
        if let Some(size) = self.pending_size.take() {
            self.size = size;
        }
        self.configured = true;
        self.draw(globals, queue)
    }

    pub fn set_scale(
        &mut self,
        scale: Scale,
        globals: &Globals,
        queue: &QueueHandle<ClientState>,
    ) -> Result<(), SplashError> {
        self.scale = scale;
        self.draw(globals, queue)
    }

    /// Shows why the app did not open, in the title and a faded icon.
    pub fn fail(
        &mut self,
        reason: &str,
        closes_at: Instant,
        globals: &Globals,
        queue: &QueueHandle<ClientState>,
    ) -> Result<(), SplashError> {
        self.toplevel
            .set_title(format!("{} could not open: {reason}", self.title));
        self.look = Look::Failed;
        self.closes_at = closes_at;
        self.draw(globals, queue)
    }

    fn draw(
        &mut self,
        globals: &Globals,
        queue: &QueueHandle<ClientState>,
    ) -> Result<(), SplashError> {
        let wanted = (self.size, self.scale, self.look);
        if !self.configured || self.drawn == Some(wanted) {
            return Ok(());
        }
        if self.background_buffer.is_none() {
            let Rgb { r, g, b } = self.background;
            let buffer = shm::buffer(&globals.shm, queue, 1, 1, &[r, g, b, u8::MAX])?;
            self.surface.attach(Some(&buffer), 0, 0);
            self.surface.damage_buffer(0, 0, 1, 1);
            self.background_buffer = Some(buffer);
        }
        let logical = |value: u32| i32::try_from(value).unwrap_or(i32::MAX);
        self.viewport
            .set_destination(logical(self.size.width), logical(self.size.height));
        let place = icon_place(self.size);
        let side = self.scale.physical(place.size).max(1);
        let pixels = icon::draw(self.icon.as_ref(), side, self.look)
            .ok_or(SplashError::Missing("memory for the icon"))?;
        let buffer = shm::buffer(&globals.shm, queue, side, side, pixels.data())?;
        self.icon_surface.attach(Some(&buffer), 0, 0);
        self.icon_surface
            .damage_buffer(0, 0, logical(side), logical(side));
        self.icon_viewport
            .set_destination(logical(place.size), logical(place.size));
        self.icon_surface.commit();
        self.icon_subsurface.set_position(place.x, place.y);
        self.surface.commit();
        if let Some(previous) = self.icon_buffer.replace(buffer) {
            previous.destroy();
        }
        self.drawn = Some(wanted);
        Ok(())
    }

    pub fn destroy(self) {
        if let Some(decoration) = self.decoration {
            decoration.destroy();
        }
        self.toplevel.destroy();
        self.xdg_surface.destroy();
        self.icon_subsurface.destroy();
        self.icon_viewport.destroy();
        self.viewport.destroy();
        if let Some(fractional_scale) = self.fractional_scale {
            fractional_scale.destroy();
        }
        self.icon_surface.destroy();
        self.surface.destroy();
        for buffer in [self.background_buffer, self.icon_buffer]
            .into_iter()
            .flatten()
        {
            buffer.destroy();
        }
    }
}
