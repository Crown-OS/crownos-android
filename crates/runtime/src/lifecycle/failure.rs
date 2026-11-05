use super::{AfterStop, Effect, FAILED_BOOTS_FOR_SAFE_MODE, Lifecycle, Milestone};

impl Lifecycle {
    /// A boot that will not reach `Ready`.
    pub(super) fn boot_failed(&mut self, reason: String) {
        self.persisted.failed_boots = self.persisted.failed_boots.saturating_add(1);
        self.persist();
        log::warn!(
            "boot {} in a row failed: {reason}",
            self.persisted.failed_boots
        );
        self.report(Milestone::BootFailed {
            reason: reason.clone(),
        });
        if self.is_warming_up() {
            self.report(Milestone::WarmUpFailed {
                reason: reason.clone(),
            });
        }
        if self.persisted.failed_boots >= FAILED_BOOTS_FOR_SAFE_MODE && !self.in_safe_mode() {
            self.enter_safe_mode(reason);
            return;
        }
        self.fail_errands(&reason);
        self.begin_stop(AfterStop::Rest);
    }

    /// A bundle mismatch, a failed handshake or a boot past the deadline.
    /// Already in safe mode, it is a failed boot instead.
    pub(super) fn trigger_safe_mode(&mut self, reason: &str) {
        if self.in_safe_mode() {
            self.boot_failed(reason.to_owned());
        } else {
            self.enter_safe_mode(reason.to_owned());
        }
    }

    fn enter_safe_mode(&mut self, reason: String) {
        log::warn!("entering safe mode: {reason}");
        self.effects.push(Effect::SetSafeMode(true));
        self.persisted.safe_mode_bundle = Some(
            self.installed()
                .and_then(|installed| installed.bundle.clone())
                .unwrap_or_default(),
        );
        self.persist();
        self.report(Milestone::SafeModeEntered { reason });
        self.begin_stop(AfterStop::Restart { safe_mode: true });
    }
}
