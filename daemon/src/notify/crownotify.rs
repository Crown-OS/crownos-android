//! The crownotify calls the daemon makes. crownotify's own `proto.rs` gains
//! `send_progress_notification` in the notify lane (contract 7); until it
//! lands everywhere, this client-only copy carries exactly its signature and
//! the existing `close_notification`, `send_general_notification` and
//! events, so the selectors match the server's.

crownos_ipc::protocol! {
    /// Client half of the `crownotify` service.
    #[allow(dead_code)]
    pub mod crownotify: "crownotify" {
        /// Dismiss one notification by the id it was posted under.
        notify close_notification(id: u32);

        /// Post a standard notification card.
        fn send_general_notification(
            app_name: String,
            summary: String,
            body: String,
            expire_timeout: u32,
            actions: Vec<String>,
        ) -> u32;

        /// Post or update a progress notification. `replaces_id` 0 posts a
        /// new one; `percent: None` shows a spinner.
        fn send_progress_notification(
            replaces_id: u32,
            app_name: String,
            icon: String,
            summary: String,
            body: String,
            percent: Option<u8>,
            actions: Vec<String>,
        ) -> u32;

        /// A notification left the screen.
        event NotificationClosed { id: u32, reason: u8 };
        /// The user invoked an action on a notification.
        event ActionInvoked { id: u32, action: String };
    }
}
