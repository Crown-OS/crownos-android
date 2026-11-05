//! Event handling for every object the daemon's connection creates.

use crownos_protocols::android_shell::v1::client::crownos_android_shell_manager_v1::CrownosAndroidShellManagerV1;
use wayland_client::globals::GlobalListContents;
use wayland_client::protocol::wl_buffer::WlBuffer;
use wayland_client::protocol::wl_compositor::WlCompositor;
use wayland_client::protocol::wl_output::{self, WlOutput};
use wayland_client::protocol::wl_registry::WlRegistry;
use wayland_client::protocol::wl_seat::WlSeat;
use wayland_client::protocol::wl_shm::WlShm;
use wayland_client::protocol::wl_shm_pool::WlShmPool;
use wayland_client::protocol::wl_subcompositor::WlSubcompositor;
use wayland_client::protocol::wl_subsurface::WlSubsurface;
use wayland_client::protocol::wl_surface::WlSurface;
use wayland_client::{
    Connection, Dispatch, Proxy, QueueHandle, delegate_noop, event_created_child,
};
use wayland_protocols::ext::data_control::v1::client::ext_data_control_device_v1::{
    self, ExtDataControlDeviceV1,
};
use wayland_protocols::ext::data_control::v1::client::ext_data_control_manager_v1::ExtDataControlManagerV1;
use wayland_protocols::ext::data_control::v1::client::ext_data_control_offer_v1::ExtDataControlOfferV1;
use wayland_protocols::ext::data_control::v1::client::ext_data_control_source_v1::{
    self, ExtDataControlSourceV1,
};
use wayland_protocols::wp::fractional_scale::v1::client::wp_fractional_scale_manager_v1::WpFractionalScaleManagerV1;
use wayland_protocols::wp::fractional_scale::v1::client::wp_fractional_scale_v1::{
    self, WpFractionalScaleV1,
};
use wayland_protocols::wp::security_context::v1::client::wp_security_context_manager_v1::WpSecurityContextManagerV1;
use wayland_protocols::wp::security_context::v1::client::wp_security_context_v1::WpSecurityContextV1;
use wayland_protocols::wp::viewporter::client::wp_viewport::WpViewport;
use wayland_protocols::wp::viewporter::client::wp_viewporter::WpViewporter;
use wayland_protocols::xdg::decoration::zv1::client::zxdg_decoration_manager_v1::ZxdgDecorationManagerV1;
use wayland_protocols::xdg::decoration::zv1::client::zxdg_toplevel_decoration_v1::ZxdgToplevelDecorationV1;
use wayland_protocols::xdg::shell::client::xdg_surface::{self, XdgSurface};
use wayland_protocols::xdg::shell::client::xdg_toplevel::{self, XdgToplevel};
use wayland_protocols::xdg::shell::client::xdg_wm_base::{self, XdgWmBase};
use wayland_protocols::xdg::xdg_output::zv1::client::zxdg_output_manager_v1::ZxdgOutputManagerV1;
use wayland_protocols::xdg::xdg_output::zv1::client::zxdg_output_v1::{self, ZxdgOutputV1};

use super::ClientState;
use super::window::WindowId;
use crate::{LogicalSize, Scale};

impl Dispatch<WlRegistry, GlobalListContents> for ClientState {
    fn event(
        _: &mut Self,
        _: &WlRegistry,
        _: <WlRegistry as Proxy>::Event,
        _: &GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<XdgWmBase, ()> for ClientState {
    fn event(
        _: &mut Self,
        wm_base: &XdgWmBase,
        event: xdg_wm_base::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let xdg_wm_base::Event::Ping { serial } = event {
            wm_base.pong(serial);
        }
    }
}

impl Dispatch<XdgSurface, WindowId> for ClientState {
    fn event(
        state: &mut Self,
        _: &XdgSurface,
        event: xdg_surface::Event,
        id: &WindowId,
        _: &Connection,
        queue: &QueueHandle<Self>,
    ) {
        let xdg_surface::Event::Configure { serial } = event else {
            return;
        };
        let globals = &state.globals;
        let Some(window) = state.windows.iter_mut().find(|window| window.id == *id) else {
            return;
        };
        let drawn = window.configure_surface(serial, globals, queue);
        state.note(*id, drawn);
    }
}

impl Dispatch<XdgToplevel, WindowId> for ClientState {
    fn event(
        state: &mut Self,
        _: &XdgToplevel,
        event: xdg_toplevel::Event,
        id: &WindowId,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            xdg_toplevel::Event::Configure { width, height, .. } => {
                if let Some(window) = state.window(*id) {
                    window.configure_toplevel(width, height);
                }
            }
            xdg_toplevel::Event::Close => state.close_where(|window| window.id == *id),
            _ => {}
        }
    }
}

impl Dispatch<WpFractionalScaleV1, WindowId> for ClientState {
    fn event(
        state: &mut Self,
        _: &WpFractionalScaleV1,
        event: wp_fractional_scale_v1::Event,
        id: &WindowId,
        _: &Connection,
        queue: &QueueHandle<Self>,
    ) {
        let wp_fractional_scale_v1::Event::PreferredScale { scale } = event else {
            return;
        };
        let scale = Scale::from_120ths(scale);
        state.scale = scale;
        let globals = &state.globals;
        let Some(window) = state.windows.iter_mut().find(|window| window.id == *id) else {
            return;
        };
        let drawn = window.set_scale(scale, globals, queue);
        state.note(*id, drawn);
    }
}

impl Dispatch<WlOutput, ()> for ClientState {
    fn event(
        state: &mut Self,
        output: &WlOutput,
        event: wl_output::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_output::Event::Scale { factor } = event
            && state.primary_output.as_ref() == Some(output)
        {
            state.scale = Scale::from_integer(factor);
        }
    }
}

impl Dispatch<ZxdgOutputV1, ()> for ClientState {
    fn event(
        state: &mut Self,
        _: &ZxdgOutputV1,
        event: zxdg_output_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let zxdg_output_v1::Event::LogicalSize { width, height } = event {
            let side = |value: i32| u32::try_from(value).ok().filter(|value| *value > 0);
            if let (Some(width), Some(height)) = (side(width), side(height)) {
                state.output_size = Some(LogicalSize { width, height });
            }
        }
    }
}

impl Dispatch<ExtDataControlSourceV1, ()> for ClientState {
    fn event(
        state: &mut Self,
        source: &ExtDataControlSourceV1,
        event: ext_data_control_source_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let ours = state
            .selection
            .as_ref()
            .is_some_and(|selection| selection.source == *source);
        match event {
            ext_data_control_source_v1::Event::Send { fd, .. } => {
                if let Some(selection) = state.selection.as_ref().filter(|_| ours) {
                    selection.send(fd);
                }
            }
            ext_data_control_source_v1::Event::Cancelled => {
                if ours {
                    if let Some(selection) = state.selection.take() {
                        selection.destroy();
                    }
                } else {
                    source.destroy();
                }
            }
            _ => {}
        }
    }
}

impl Dispatch<ExtDataControlDeviceV1, ()> for ClientState {
    fn event(
        _: &mut Self,
        _: &ExtDataControlDeviceV1,
        event: ext_data_control_device_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            ext_data_control_device_v1::Event::Selection { id: Some(offer) }
            | ext_data_control_device_v1::Event::PrimarySelection { id: Some(offer) } => {
                offer.destroy();
            }
            _ => {}
        }
    }

    event_created_child!(ClientState, ExtDataControlDeviceV1, [
        ext_data_control_device_v1::EVT_DATA_OFFER_OPCODE => (ExtDataControlOfferV1, ()),
    ]);
}

delegate_noop!(ClientState: ignore WlCompositor);
delegate_noop!(ClientState: ignore WlSubcompositor);
delegate_noop!(ClientState: ignore WlShm);
delegate_noop!(ClientState: ignore WlShmPool);
delegate_noop!(ClientState: ignore WlBuffer);
delegate_noop!(ClientState: ignore WlSubsurface);
delegate_noop!(ClientState: ignore WlSeat);
delegate_noop!(ClientState: ignore WpViewporter);
delegate_noop!(ClientState: ignore WpViewport);
delegate_noop!(ClientState: ignore WpFractionalScaleManagerV1);
delegate_noop!(ClientState: ignore ZxdgDecorationManagerV1);
delegate_noop!(ClientState: ignore ZxdgToplevelDecorationV1);
delegate_noop!(ClientState: ignore ZxdgOutputManagerV1);
delegate_noop!(ClientState: ignore CrownosAndroidShellManagerV1);
delegate_noop!(ClientState: ignore WpSecurityContextManagerV1);
delegate_noop!(ClientState: ignore WpSecurityContextV1);
delegate_noop!(ClientState: ignore ExtDataControlManagerV1);
delegate_noop!(ClientState: ignore ExtDataControlOfferV1);

impl Dispatch<WlSurface, WindowId> for ClientState {
    fn event(
        _: &mut Self,
        _: &WlSurface,
        _: <WlSurface as Proxy>::Event,
        _: &WindowId,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}
