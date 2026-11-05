use crownos_protocols::android_shell::v1::client::crownos_android_shell_manager_v1::CrownosAndroidShellManagerV1;
use wayland_client::QueueHandle;
use wayland_client::globals::GlobalList;
use wayland_client::protocol::wl_compositor::WlCompositor;
use wayland_client::protocol::wl_output::WlOutput;
use wayland_client::protocol::wl_seat::WlSeat;
use wayland_client::protocol::wl_shm::WlShm;
use wayland_client::protocol::wl_subcompositor::WlSubcompositor;
use wayland_protocols::ext::data_control::v1::client::ext_data_control_manager_v1::ExtDataControlManagerV1;
use wayland_protocols::wp::fractional_scale::v1::client::wp_fractional_scale_manager_v1::WpFractionalScaleManagerV1;
use wayland_protocols::wp::security_context::v1::client::wp_security_context_manager_v1::WpSecurityContextManagerV1;
use wayland_protocols::wp::viewporter::client::wp_viewporter::WpViewporter;
use wayland_protocols::xdg::decoration::zv1::client::zxdg_decoration_manager_v1::ZxdgDecorationManagerV1;
use wayland_protocols::xdg::shell::client::xdg_wm_base::XdgWmBase;
use wayland_protocols::xdg::xdg_output::zv1::client::zxdg_output_manager_v1::ZxdgOutputManagerV1;

use super::{ClientState, SplashError};

/// The globals the daemon uses. The required ones fail the connection when
/// missing; the others turn their feature off.
pub struct Globals {
    pub compositor: WlCompositor,
    pub subcompositor: WlSubcompositor,
    pub shm: WlShm,
    pub wm_base: XdgWmBase,
    pub viewporter: WpViewporter,
    pub decorations: Option<ZxdgDecorationManagerV1>,
    pub fractional_scale: Option<WpFractionalScaleManagerV1>,
    pub android_shell: Option<CrownosAndroidShellManagerV1>,
    pub security_context: Option<WpSecurityContextManagerV1>,
    pub data_control: Option<ExtDataControlManagerV1>,
    pub seat: Option<WlSeat>,
}

impl Globals {
    pub fn bind(list: &GlobalList, queue: &QueueHandle<ClientState>) -> Result<Self, SplashError> {
        let required = |interface: &'static str| move |_| SplashError::Missing(interface);
        let globals = Self {
            compositor: list
                .bind(queue, 4..=6, ())
                .map_err(required("wl_compositor"))?,
            subcompositor: list
                .bind(queue, 1..=1, ())
                .map_err(required("wl_subcompositor"))?,
            shm: list.bind(queue, 1..=1, ()).map_err(required("wl_shm"))?,
            wm_base: list
                .bind(queue, 1..=6, ())
                .map_err(required("xdg_wm_base"))?,
            viewporter: list
                .bind(queue, 1..=1, ())
                .map_err(required("wp_viewporter"))?,
            decorations: list.bind(queue, 1..=1, ()).ok(),
            fractional_scale: list.bind(queue, 1..=1, ()).ok(),
            android_shell: list.bind(queue, 1..=1, ()).ok(),
            security_context: list.bind(queue, 1..=1, ()).ok(),
            data_control: list.bind(queue, 1..=1, ()).ok(),
            seat: list.bind(queue, 1..=1, ()).ok(),
        };
        if globals.android_shell.is_none() {
            log::warn!("no crownos_android_shell_manager_v1: splashes are not placeholders");
        }
        Ok(globals)
    }
}

/// The first output and its logical size, followed through xdg-output.
pub fn bind_primary_output(
    list: &GlobalList,
    queue: &QueueHandle<ClientState>,
) -> Option<WlOutput> {
    let output: WlOutput = list.bind(queue, 1..=4, ()).ok()?;
    let manager: Option<ZxdgOutputManagerV1> = list.bind(queue, 1..=3, ()).ok();
    if let Some(manager) = manager {
        manager.get_xdg_output(&output, queue, ());
    }
    Some(output)
}
