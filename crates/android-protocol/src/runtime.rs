use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RuntimeState {
    /// Android support was not chosen, or was removed.
    NotInstalled,
    /// The root setup service is still working. See the `SetupProgress` event.
    SettingUp,
    Stopped,
    Starting,
    Running,
    Stopping,
    /// Android runs with stock windowing because the CrownOS bundle did not load.
    SafeMode,
    /// This machine cannot run Android, for example a 16 KB page kernel.
    Unsupported,
}

/// How Android renders on this machine.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum GpuTier {
    /// Mesa on the compositor's GPU.
    Native,
    /// Venus through the host render server on an NVIDIA GPU.
    Venus,
    /// SwiftShader on the CPU.
    Software,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum StopReason {
    User,
    Idle,
    Update,
    Shutdown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum LaunchOutcome {
    /// The task is opening. Its window follows.
    Opening,
    /// Android is starting. The splash window shows until the task opens.
    Starting,
    NotInstalled,
    SettingUp,
    Unsupported,
    UnknownPackage,
}

/// New fields only ever go last: postcard ignores trailing bytes, so older
/// clients still decode it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeStatus {
    pub state: RuntimeState,
    /// Open Android windows.
    pub windows: u32,
    /// When Android stops for being idle, in Unix milliseconds. `None` while
    /// windows are open or something keeps Android awake.
    pub idle_stop_at_ms: Option<u64>,
    pub tier: Option<GpuTier>,
    pub bundle_version: Option<String>,
}

impl RuntimeStatus {
    pub const fn stopped() -> Self {
        Self {
            state: RuntimeState::Stopped,
            windows: 0,
            idle_stop_at_ms: None,
            tier: None,
            bundle_version: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crownos_ipc::postcard;

    #[derive(Debug, PartialEq, Deserialize)]
    struct StatusWithoutBundle {
        state: RuntimeState,
        windows: u32,
        idle_stop_at_ms: Option<u64>,
        tier: Option<GpuTier>,
    }

    #[test]
    fn older_clients_still_read_the_status() -> Result<(), postcard::Error> {
        let status = RuntimeStatus {
            state: RuntimeState::Running,
            windows: 2,
            idle_stop_at_ms: None,
            tier: Some(GpuTier::Native),
            bundle_version: Some("1".into()),
        };
        let bytes = postcard::to_stdvec(&status)?;
        assert_eq!(
            postcard::from_bytes::<StatusWithoutBundle>(&bytes)?,
            StatusWithoutBundle {
                state: RuntimeState::Running,
                windows: 2,
                idle_stop_at_ms: None,
                tier: Some(GpuTier::Native),
            }
        );
        Ok(())
    }
}
