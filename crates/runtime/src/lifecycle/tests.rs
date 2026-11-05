use std::time::{Duration, Instant};

use agent_wire::{KeepAliveReasons, Record, WIRE_VERSION};
use android_protocol::{GpuTier, LaunchOutcome, RuntimeState, StopReason};

use super::*;

const BUNDLE: &str = "2026.11.0";
const PACKAGE: &str = "com.whatsapp";
const MINUTE: Duration = Duration::from_secs(60);

/// A clock the test moves by hand.
struct FakeClock(Instant);

impl FakeClock {
    fn new() -> Self {
        Self(Instant::now())
    }

    fn now(&self) -> Instant {
        self.0
    }

    fn advance(&mut self, by: Duration) -> Instant {
        self.0 += by;
        self.0
    }
}

fn installed(google_play: bool, tier: GpuTier) -> Installation {
    Installation::Installed(Installed {
        tier: Some(tier),
        bundle: Some(BUNDLE.into()),
        google_play,
    })
}

fn warmed() -> Persisted {
    Persisted {
        warmed_up: true,
        ..Persisted::default()
    }
}

fn lifecycle(clock: &FakeClock) -> Lifecycle {
    let mut lifecycle = Lifecycle::new(warmed(), 5);
    lifecycle.set_installation(installed(false, GpuTier::Native), clock.now());
    lifecycle.take_effects();
    lifecycle
}

fn launch() -> Errand {
    Errand::Launch {
        package: PACKAGE.into(),
        token: Some("token".into()),
    }
}

fn hello(bundle: &str) -> Record {
    Record::Hello {
        wire: WIRE_VERSION,
        bundle: bundle.into(),
    }
}

/// Runs the boot the lifecycle just asked for up to `Ready`.
fn boot(lifecycle: &mut Lifecycle, clock: &FakeClock, bundle: &str) -> Vec<Effect> {
    lifecycle.session_started();
    assert!(lifecycle.wants_agent());
    lifecycle.agent_connected();
    lifecycle.agent_record(hello(bundle), clock.now());
    lifecycle.agent_record(Record::Ready { boot_ms: 9000 }, clock.now());
    lifecycle.take_effects()
}

fn running(clock: &FakeClock) -> Lifecycle {
    let mut lifecycle = lifecycle(clock);
    lifecycle.submit(launch(), clock.now());
    boot(&mut lifecycle, clock, BUNDLE);
    lifecycle.agent_record(
        Record::TaskOpened {
            task: 7,
            package: PACKAGE.into(),
        },
        clock.now(),
    );
    lifecycle.take_effects();
    lifecycle
}

fn sends(effects: &[Effect], record: &Record) -> bool {
    effects.contains(&Effect::SendAgent(record.clone()))
}

fn stops(effects: &[Effect]) -> bool {
    effects
        .iter()
        .any(|effect| matches!(effect, Effect::StopSession { .. }))
}

fn starts(effects: &[Effect]) -> bool {
    effects
        .iter()
        .any(|effect| matches!(effect, Effect::StartSession { .. }))
}

#[test]
fn a_launch_while_stopped_shows_the_splash_and_starts_android() {
    let clock = FakeClock::new();
    let mut lifecycle = lifecycle(&clock);
    assert_eq!(
        lifecycle.submit(launch(), clock.now()),
        LaunchOutcome::Starting
    );
    let effects = lifecycle.take_effects();
    assert_eq!(
        effects[..2],
        [
            Effect::ShowSplash {
                package: PACKAGE.into()
            },
            Effect::StartSession { venus: false },
        ]
    );
    assert_eq!(lifecycle.state(), RuntimeState::Starting);
}

#[test]
fn the_queued_launch_goes_out_once_android_is_ready() {
    let clock = FakeClock::new();
    let mut lifecycle = lifecycle(&clock);
    lifecycle.submit(launch(), clock.now());
    lifecycle.session_started();
    lifecycle.agent_connected();
    let effects = lifecycle.take_effects();
    assert!(sends(&effects, &hello(BUNDLE)));
    lifecycle.agent_record(hello(BUNDLE), clock.now());
    assert_eq!(lifecycle.state(), RuntimeState::Starting);
    lifecycle.agent_record(Record::Ready { boot_ms: 9000 }, clock.now());
    let effects = lifecycle.take_effects();
    assert!(effects.contains(&Effect::Perform(launch())));
    assert!(effects.contains(&Effect::HoldSleepInhibitor));
    assert!(sends(&effects, &Record::ListTasks));
    assert_eq!(lifecycle.state(), RuntimeState::Running);
}

#[test]
fn the_venus_tier_starts_its_render_service_with_the_session() {
    let clock = FakeClock::new();
    let mut lifecycle = Lifecycle::new(warmed(), 5);
    lifecycle.set_installation(installed(false, GpuTier::Venus), clock.now());
    lifecycle.submit(launch(), clock.now());
    assert!(
        lifecycle
            .take_effects()
            .contains(&Effect::StartSession { venus: true })
    );
}

#[test]
fn launching_an_open_app_raises_it_without_a_splash() {
    let clock = FakeClock::new();
    let mut lifecycle = running(&clock);
    assert_eq!(
        lifecycle.submit(launch(), clock.now()),
        LaunchOutcome::Opening
    );
    assert_eq!(lifecycle.take_effects(), [Effect::Perform(launch())]);
}

#[test]
fn launches_wait_for_setup() {
    let clock = FakeClock::new();
    let mut lifecycle = Lifecycle::new(warmed(), 5);
    assert_eq!(
        lifecycle.submit(launch(), clock.now()),
        LaunchOutcome::NotInstalled
    );
    lifecycle.set_installation(Installation::SettingUp, clock.now());
    assert_eq!(
        lifecycle.submit(launch(), clock.now()),
        LaunchOutcome::SettingUp
    );
    assert_eq!(lifecycle.state(), RuntimeState::SettingUp);
}

#[test]
fn android_stops_after_the_idle_timeout_with_no_window() {
    let mut clock = FakeClock::new();
    let mut lifecycle = running(&clock);
    assert_eq!(lifecycle.idle_deadline(), None);
    lifecycle.agent_record(Record::TaskClosed { task: 7 }, clock.now());
    assert_eq!(lifecycle.windows(), 0);
    assert_eq!(lifecycle.idle_deadline(), Some(clock.now() + 5 * MINUTE));
    lifecycle.take_effects();

    let now = clock.advance(MINUTE);
    lifecycle.tick(now);
    assert!(sends(&lifecycle.take_effects(), &Record::QueryKeepAlive));
    lifecycle.agent_record(
        Record::KeepAlive {
            reasons: KeepAliveReasons(0),
        },
        now,
    );
    assert!(!stops(&lifecycle.take_effects()));

    let now = clock.advance(4 * MINUTE);
    lifecycle.tick(now);
    assert!(sends(&lifecycle.take_effects(), &Record::QueryKeepAlive));
    lifecycle.agent_record(
        Record::KeepAlive {
            reasons: KeepAliveReasons(0),
        },
        now,
    );
    let effects = lifecycle.take_effects();
    assert!(sends(&effects, &Record::Shutdown));
    assert!(stops(&effects));
    assert!(effects.contains(&Effect::ReleaseSleepInhibitor));
    assert_eq!(lifecycle.state(), RuntimeState::Stopping);

    lifecycle.stopped(now);
    assert_eq!(lifecycle.state(), RuntimeState::Stopped);
}

#[test]
fn an_unanswered_last_keep_alive_question_still_stops_android() {
    let mut clock = FakeClock::new();
    let mut lifecycle = running(&clock);
    lifecycle.agent_record(Record::TaskClosed { task: 7 }, clock.now());
    let now = clock.advance(5 * MINUTE);
    lifecycle.tick(now);
    lifecycle.take_effects();
    let now = clock.advance(KEEP_ALIVE_REPLY);
    lifecycle.tick(now);
    assert!(stops(&lifecycle.take_effects()));
}

#[test]
fn any_keep_alive_reason_restarts_the_timer() {
    let mut clock = FakeClock::new();
    let mut lifecycle = running(&clock);
    lifecycle.agent_record(Record::TaskClosed { task: 7 }, clock.now());
    let now = clock.advance(4 * MINUTE);
    lifecycle.agent_record(
        Record::KeepAlive {
            reasons: KeepAliveReasons(KeepAliveReasons::AUDIO),
        },
        now,
    );
    assert_eq!(lifecycle.idle_deadline(), Some(now + 5 * MINUTE));
    let now = clock.advance(2 * MINUTE);
    lifecycle.tick(now);
    assert!(!stops(&lifecycle.take_effects()));
}

#[test]
fn an_idle_timeout_of_zero_never_stops_android() {
    let mut clock = FakeClock::new();
    let mut lifecycle = running(&clock);
    lifecycle.set_idle_minutes(0, clock.now());
    lifecycle.agent_record(Record::TaskClosed { task: 7 }, clock.now());
    assert_eq!(lifecycle.idle_deadline(), None);
    let now = clock.advance(600 * MINUTE);
    lifecycle.tick(now);
    let effects = lifecycle.take_effects();
    assert!(!stops(&effects));
    assert!(!sends(&effects, &Record::QueryKeepAlive));
}

#[test]
fn a_changed_idle_timeout_applies_to_the_running_countdown() {
    let mut clock = FakeClock::new();
    let mut lifecycle = running(&clock);
    lifecycle.agent_record(Record::TaskClosed { task: 7 }, clock.now());
    let now = clock.advance(MINUTE);
    lifecycle.set_idle_minutes(1, now);
    assert_eq!(lifecycle.idle_deadline(), Some(now + MINUTE));
}

#[test]
fn a_bundle_mismatch_restarts_android_once_in_safe_mode() {
    let clock = FakeClock::new();
    let mut lifecycle = lifecycle(&clock);
    lifecycle.submit(launch(), clock.now());
    lifecycle.session_started();
    lifecycle.agent_connected();
    lifecycle.agent_record(hello("2026.10.0"), clock.now());
    let effects = lifecycle.take_effects();
    let set = effects.iter().position(|e| *e == Effect::SetSafeMode(true));
    let stop = effects
        .iter()
        .position(|e| matches!(e, Effect::StopSession { .. }));
    assert!(
        set < stop && set.is_some(),
        "the prop is set before the session stops"
    );
    assert!(
        effects
            .iter()
            .any(|e| matches!(e, Effect::Report(Milestone::SafeModeEntered { .. })))
    );
    assert_eq!(
        lifecycle.persisted().safe_mode_bundle.as_deref(),
        Some(BUNDLE)
    );

    lifecycle.stopped(clock.now());
    assert!(starts(&lifecycle.take_effects()));
    let effects = boot(&mut lifecycle, &clock, "2026.10.0");
    assert!(effects.contains(&Effect::Perform(launch())));
    assert_eq!(lifecycle.state(), RuntimeState::SafeMode);
}

#[test]
fn no_ready_within_two_minutes_triggers_safe_mode() {
    let mut clock = FakeClock::new();
    let mut lifecycle = lifecycle(&clock);
    lifecycle.submit(launch(), clock.now());
    lifecycle.session_started();
    lifecycle.take_effects();
    let now = clock.advance(READY_TIMEOUT);
    lifecycle.tick(now);
    let effects = lifecycle.take_effects();
    assert!(effects.contains(&Effect::SetSafeMode(true)));
    assert!(stops(&effects));
}

#[test]
fn a_failed_safe_mode_boot_is_not_restarted_again() {
    let mut clock = FakeClock::new();
    let mut lifecycle = lifecycle(&clock);
    lifecycle.submit(launch(), clock.now());
    let now = clock.advance(READY_TIMEOUT);
    lifecycle.tick(now);
    lifecycle.stopped(now);
    lifecycle.take_effects();
    let now = clock.advance(READY_TIMEOUT);
    lifecycle.tick(now);
    let effects = lifecycle.take_effects();
    assert!(!effects.contains(&Effect::SetSafeMode(true)));
    assert!(effects.contains(&Effect::FailSplash {
        package: PACKAGE.into(),
        reason: "Android did not finish starting within two minutes.".into(),
    }));
    lifecycle.stopped(now);
    assert_eq!(lifecycle.state(), RuntimeState::Stopped);
}

#[test]
fn three_failed_boots_in_a_row_trigger_safe_mode() {
    let clock = FakeClock::new();
    let mut lifecycle = lifecycle(&clock);
    for attempt in 1..=FAILED_BOOTS_FOR_SAFE_MODE {
        lifecycle.submit(launch(), clock.now());
        lifecycle.session_failed(BootFailure::Host("the session unit failed".into()));
        let effects = lifecycle.take_effects();
        assert_eq!(lifecycle.persisted().failed_boots, attempt);
        let safe = effects.contains(&Effect::SetSafeMode(true));
        assert_eq!(safe, attempt == FAILED_BOOTS_FOR_SAFE_MODE);
        lifecycle.stopped(clock.now());
        if !safe {
            assert_eq!(lifecycle.state(), RuntimeState::Stopped);
        }
    }
    assert_eq!(lifecycle.state(), RuntimeState::Starting);
}

#[test]
fn a_ready_boot_resets_the_failure_count() {
    let clock = FakeClock::new();
    let mut lifecycle = lifecycle(&clock);
    lifecycle.submit(launch(), clock.now());
    lifecycle.session_failed(BootFailure::Host("failed".into()));
    lifecycle.stopped(clock.now());
    lifecycle.take_effects();
    lifecycle.submit(launch(), clock.now());
    boot(&mut lifecycle, &clock, BUNDLE);
    assert_eq!(lifecycle.persisted().failed_boots, 0);
}

#[test]
fn a_new_bundle_leaves_safe_mode_at_the_next_boot() {
    let clock = FakeClock::new();
    let persisted = Persisted {
        warmed_up: true,
        safe_mode_bundle: Some("2026.10.0".into()),
        ..Persisted::default()
    };
    let mut lifecycle = Lifecycle::new(persisted, 5);
    lifecycle.set_installation(installed(false, GpuTier::Native), clock.now());
    lifecycle.submit(launch(), clock.now());
    let effects = boot(&mut lifecycle, &clock, BUNDLE);
    assert!(effects.contains(&Effect::SetSafeMode(false)));
    assert_eq!(lifecycle.persisted().safe_mode_bundle, None);
    assert_eq!(lifecycle.state(), RuntimeState::SafeMode);
}

#[test]
fn sleep_freezes_android_before_letting_go_of_the_inhibitor() {
    let clock = FakeClock::new();
    let mut lifecycle = running(&clock);
    lifecycle.prepare_for_sleep(true);
    assert_eq!(
        lifecycle.take_effects(),
        [Effect::Freeze, Effect::ReleaseSleepInhibitor]
    );
    lifecycle.prepare_for_sleep(false);
    assert_eq!(
        lifecycle.take_effects(),
        [Effect::Thaw { venus: false }, Effect::HoldSleepInhibitor]
    );
}

#[test]
fn sleep_with_android_stopped_freezes_nothing() {
    let clock = FakeClock::new();
    let mut lifecycle = lifecycle(&clock);
    lifecycle.prepare_for_sleep(true);
    assert_eq!(lifecycle.take_effects(), [Effect::ReleaseSleepInhibitor]);
    lifecycle.prepare_for_sleep(false);
    assert_eq!(lifecycle.take_effects(), []);
}

#[test]
fn another_users_session_makes_the_runtime_unsupported_until_tried_again() {
    let clock = FakeClock::new();
    let mut lifecycle = lifecycle(&clock);
    lifecycle.submit(launch(), clock.now());
    lifecycle.session_failed(BootFailure::OtherUser { uid: 1001 });
    let effects = lifecycle.take_effects();
    assert!(effects.contains(&Effect::AbandonSession));
    assert!(!stops(&effects), "another user's container is left alone");
    assert_eq!(lifecycle.state(), RuntimeState::Unsupported);
    assert!(
        lifecycle
            .unsupported_reason()
            .is_some_and(|m| m.contains("1001"))
    );
    lifecycle.submit(launch(), clock.now());
    assert!(starts(&lifecycle.take_effects()));
}

#[test]
fn a_restarted_agent_is_welcomed_back() {
    let mut clock = FakeClock::new();
    let mut lifecycle = running(&clock);
    let now = clock.advance(Duration::from_secs(1));
    lifecycle.agent_lost(now);
    assert!(lifecycle.wants_agent());
    assert_eq!(lifecycle.submit(launch(), now), LaunchOutcome::Opening);
    lifecycle.agent_connected();
    lifecycle.agent_record(hello(BUNDLE), now);
    let effects = lifecycle.take_effects();
    assert!(sends(&effects, &Record::ListTasks));
    assert!(
        !effects.contains(&Effect::Perform(launch())),
        "the agent refuses launches until Ready"
    );
    lifecycle.agent_record(Record::Ready { boot_ms: 1 }, now);
    assert!(
        lifecycle
            .take_effects()
            .contains(&Effect::Perform(launch()))
    );
    lifecycle.agent_record(Record::Ready { boot_ms: 1 }, now);
    assert_eq!(
        lifecycle.take_effects(),
        [],
        "a repeated Ready changes nothing"
    );
    assert_eq!(lifecycle.state(), RuntimeState::Running);
}

#[test]
fn tasks_are_a_set_keyed_by_task_id() {
    let clock = FakeClock::new();
    let mut lifecycle = running(&clock);
    let opened = Record::TaskOpened {
        task: 7,
        package: PACKAGE.into(),
    };
    lifecycle.agent_record(opened.clone(), clock.now());
    assert_eq!(lifecycle.windows(), 1);
    lifecycle.agent_record(Record::TaskClosed { task: 7 }, clock.now());
    lifecycle.agent_record(opened, clock.now());
    assert_eq!(lifecycle.windows(), 1);
    assert_eq!(lifecycle.idle_deadline(), None);
}

#[test]
fn an_agent_that_stays_away_stops_android() {
    let mut clock = FakeClock::new();
    let mut lifecycle = running(&clock);
    lifecycle.agent_lost(clock.now());
    let now = clock.advance(AGENT_RECONNECT_GRACE);
    lifecycle.tick(now);
    assert!(stops(&lifecycle.take_effects()));
}

#[test]
fn a_failed_launch_closes_its_splash_with_the_reason() {
    let clock = FakeClock::new();
    let mut lifecycle = running(&clock);
    lifecycle.agent_record(
        Record::LaunchFailed {
            package: "org.example".into(),
            reason: "not installed".into(),
        },
        clock.now(),
    );
    assert_eq!(
        lifecycle.take_effects(),
        [Effect::FailSplash {
            package: "org.example".into(),
            reason: "not installed".into()
        }]
    );
}

#[test]
fn a_user_stop_fails_queued_launches() {
    let clock = FakeClock::new();
    let mut lifecycle = lifecycle(&clock);
    lifecycle.submit(launch(), clock.now());
    lifecycle.take_effects();
    lifecycle.stop(StopReason::User);
    let effects = lifecycle.take_effects();
    assert!(
        effects
            .iter()
            .any(|e| matches!(e, Effect::FailSplash { .. }))
    );
    assert!(stops(&effects));
}

#[test]
fn the_first_boot_after_setup_warms_android_up_and_stops_it() {
    let clock = FakeClock::new();
    let mut lifecycle = Lifecycle::new(Persisted::default(), 5);
    lifecycle.set_installation(installed(false, GpuTier::Native), clock.now());
    let effects = lifecycle.take_effects();
    assert!(effects.contains(&Effect::Report(Milestone::WarmUpStarted)));
    assert!(starts(&effects));
    let effects = boot(&mut lifecycle, &clock, BUNDLE);
    assert!(effects.contains(&Effect::Report(Milestone::WarmUpFinished)));
    assert!(stops(&effects));
    assert!(lifecycle.persisted().warmed_up);
}

#[test]
fn the_warm_up_with_google_play_waits_for_the_services_id() {
    let mut clock = FakeClock::new();
    let mut lifecycle = Lifecycle::new(Persisted::default(), 5);
    lifecycle.set_installation(installed(true, GpuTier::Native), clock.now());
    lifecycle.take_effects();
    let effects = boot(&mut lifecycle, &clock, BUNDLE);
    assert!(sends(&effects, &Record::QueryGsfId));
    assert!(!stops(&effects));
    let now = clock.advance(Duration::from_secs(10));
    lifecycle.tick(now);
    assert!(sends(&lifecycle.take_effects(), &Record::QueryGsfId));
    lifecycle.agent_record(Record::GsfId { id: "3f2a".into() }, now);
    let effects = lifecycle.take_effects();
    assert!(effects.contains(&Effect::Report(Milestone::GsfId { id: "3f2a".into() })));
    assert!(effects.contains(&Effect::Report(Milestone::WarmUpFinished)));
    assert!(stops(&effects));
    assert_eq!(lifecycle.persisted().gsf_id.as_deref(), Some("3f2a"));
}

#[test]
fn the_warm_up_keeps_android_for_an_app_launched_meanwhile() {
    let clock = FakeClock::new();
    let mut lifecycle = Lifecycle::new(Persisted::default(), 5);
    lifecycle.set_installation(installed(false, GpuTier::Native), clock.now());
    lifecycle.submit(launch(), clock.now());
    let effects = boot(&mut lifecycle, &clock, BUNDLE);
    assert!(effects.contains(&Effect::Report(Milestone::WarmUpFinished)));
    assert!(!stops(&effects));
}

#[test]
fn removing_android_stops_it() {
    let clock = FakeClock::new();
    let mut lifecycle = running(&clock);
    lifecycle.set_installation(Installation::NotInstalled, clock.now());
    assert!(stops(&lifecycle.take_effects()));
    lifecycle.stopped(clock.now());
    assert_eq!(lifecycle.state(), RuntimeState::NotInstalled);
}

#[test]
fn a_running_android_is_adopted_without_starting_a_session() {
    let clock = FakeClock::new();
    let mut lifecycle = lifecycle(&clock);
    lifecycle.adopt(clock.now());
    assert!(lifecycle.wants_agent());
    assert!(!starts(&lifecycle.take_effects()));
    lifecycle.agent_connected();
    lifecycle.agent_record(hello(BUNDLE), clock.now());
    lifecycle.agent_record(Record::Ready { boot_ms: 1 }, clock.now());
    assert_eq!(lifecycle.state(), RuntimeState::Running);
}
