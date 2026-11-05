use agent_wire::Record;

use super::Errand;
use crate::Persisted;

/// What the daemon must do after an event. Effects come out in the order
/// they must run; host work (sessions, freezing, props) runs in that order on
/// one worker.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Effect {
    /// The public status changed: emit `RuntimeChanged`.
    StatusChanged,
    /// Open the splash window of `package` now.
    ShowSplash {
        package: String,
    },
    /// Show why `package` did not open, then close its splash.
    FailSplash {
        package: String,
        reason: String,
    },
    /// Create the Wayland security context and data home, then start the
    /// container, the render service on the Venus tier and the session unit.
    StartSession {
        venus: bool,
    },
    /// Stop the session, the container and its service, and the render service.
    StopSession {
        venus: bool,
    },
    /// Drop the host side of a session that never got the container, which
    /// belongs to another user.
    AbandonSession,
    SendAgent(Record),
    /// Close the agent connection.
    DropAgent,
    /// Turn an errand into agent records or Waydroid calls.
    Perform(Errand),
    /// `waydroid prop set persist.crownos.safe_mode`, effective next boot.
    SetSafeMode(bool),
    /// Freeze the container, then let the machine sleep.
    Freeze,
    /// Unfreeze the container after resume, restarting the render service on
    /// the Venus tier.
    Thaw {
        venus: bool,
    },
    HoldSleepInhibitor,
    ReleaseSleepInhibitor,
    /// Store the state file.
    Persist(Persisted),
    /// Something the user hears about.
    Report(Milestone),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Milestone {
    /// The first boot after setup started.
    WarmUpStarted,
    /// The first boot after setup reached `Ready`.
    WarmUpFinished,
    WarmUpFailed {
        reason: String,
    },
    /// Android restarts with stock windowing.
    SafeModeEntered {
        reason: String,
    },
    BootFailed {
        reason: String,
    },
    InstallFinished {
        path: String,
        ok: bool,
        message: String,
    },
    /// The agent reported a Google services framework id.
    GsfId {
        id: String,
    },
}
