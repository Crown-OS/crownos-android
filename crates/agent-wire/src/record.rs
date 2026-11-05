/// A rectangle in Android display pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Rect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

/// Why Android must keep running with no window open.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct KeepAliveReasons(pub u32);

impl KeepAliveReasons {
    pub const AUDIO: u32 = 1 << 0;
    pub const CALL: u32 = 1 << 1;
    pub const INSTALL: u32 = 1 << 2;
    pub const DOWNLOAD: u32 = 1 << 3;
    pub const FOREGROUND_SERVICE: u32 = 1 << 4;

    pub const fn any(self) -> bool {
        self.0 != 0
    }
}

/// Every record, in wire type order. The doc comment says who sends it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Record {
    /// Both directions, first record on every connection.
    Hello {
        wire: u32,
        bundle: String,
    },

    /// HWC → agent: the host window of `task` is now `width`×`height` pixels.
    SetSize {
        task: u32,
        width: u32,
        height: u32,
        serial: u32,
    },
    /// HWC → agent: the host focused the window of `task`.
    Focus {
        task: u32,
    },
    /// HWC → agent: the user closed the window of `task`.
    Close {
        task: u32,
    },
    /// HWC → agent: the user pressed Back in the window of `task`.
    Back {
        task: u32,
    },
    /// HWC → agent: whether the host shows the window of `task` at all.
    Visibility {
        task: u32,
        visible: bool,
    },

    /// Agent → HWC: facts about a task, sent when it opens and when they change.
    TaskInfo {
        task: u32,
        package: String,
        label: String,
        /// `0xAARRGGBB`; 0 means no color.
        color: u32,
        resizable: bool,
        min_width: u32,
        min_height: u32,
    },
    /// Agent → HWC: `task` now occupies `slot`, after the [`Record::SetSize`] with `serial`.
    SlotApplied {
        task: u32,
        serial: u32,
        slot: Rect,
    },
    /// Agent → HWC: `task` is gone.
    TaskRemoved {
        task: u32,
    },
    /// Agent → HWC: raise the window of `task` with this xdg-activation token.
    Activate {
        task: u32,
        token: String,
    },

    /// Daemon → agent: open `package` in a new task of this size.
    Launch {
        package: String,
        width: u32,
        height: u32,
        token: String,
    },
    /// Daemon → agent: reply with [`Record::Tasks`].
    ListTasks,
    /// Daemon → agent: reply with [`Record::KeepAlive`].
    QueryKeepAlive,
    /// Daemon → agent: reply with [`Record::GsfId`].
    QueryGsfId,
    /// Daemon → agent: Android is about to stop.
    Shutdown,
    /// Daemon → agent: open a `market:` or `https:` link in Android.
    OpenUri {
        uri: String,
    },
    /// Daemon → agent: install the APK at this path under `/data/crownos/inbox/`.
    InstallPackage {
        path: String,
    },

    /// Agent → daemon: Android finished booting.
    Ready {
        boot_ms: u32,
    },
    TaskOpened {
        task: u32,
        package: String,
    },
    TaskClosed {
        task: u32,
    },
    Tasks {
        tasks: Vec<u32>,
    },
    KeepAlive {
        reasons: KeepAliveReasons,
    },
    /// Empty when the device has no Google services id yet.
    GsfId {
        id: String,
    },
    LaunchFailed {
        package: String,
        reason: String,
    },
    InstallResult {
        path: String,
        ok: bool,
        message: String,
    },
}

impl Record {
    pub const fn kind(&self) -> u16 {
        match self {
            Self::Hello { .. } => 0x0001,
            Self::SetSize { .. } => 0x0101,
            Self::Focus { .. } => 0x0102,
            Self::Close { .. } => 0x0103,
            Self::Back { .. } => 0x0104,
            Self::Visibility { .. } => 0x0105,
            Self::TaskInfo { .. } => 0x0201,
            Self::SlotApplied { .. } => 0x0202,
            Self::TaskRemoved { .. } => 0x0203,
            Self::Activate { .. } => 0x0204,
            Self::Launch { .. } => 0x0301,
            Self::ListTasks => 0x0302,
            Self::QueryKeepAlive => 0x0303,
            Self::QueryGsfId => 0x0304,
            Self::Shutdown => 0x0305,
            Self::OpenUri { .. } => 0x0306,
            Self::InstallPackage { .. } => 0x0307,
            Self::Ready { .. } => 0x0401,
            Self::TaskOpened { .. } => 0x0402,
            Self::TaskClosed { .. } => 0x0403,
            Self::Tasks { .. } => 0x0404,
            Self::KeepAlive { .. } => 0x0405,
            Self::GsfId { .. } => 0x0406,
            Self::LaunchFailed { .. } => 0x0407,
            Self::InstallResult { .. } => 0x0408,
        }
    }
}
