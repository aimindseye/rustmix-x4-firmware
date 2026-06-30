// ReaderRemote guarded navigation for the COLMI R10 BLE remote.
//
// This module is the first runtime navigation gate after the ProbeOnly path.
// It is deliberately strict:
// - Disabled is default.
// - ProbeOnly never injects Reader input.
// - ReaderRemote must be explicitly unlocked.
// - Only the Reader screen may receive Reader navigation.
// - Notify handle and R10 packet classification must pass.
// - Existing R10 debounce and input bridge are reused.

use super::r10_ble_device_task::R10BleDeviceTaskX4DeployMode;
use super::r10_ble_transport::R10_BLE_LIVE_NOTIFY_VALUE_HANDLE;
use super::r10_input_bridge::R10InputInjection;

#[cfg(all(target_arch = "riscv32", feature = "r10-ble-host"))]
use super::r10_input_bridge::try_enqueue_remote_action;
use super::r10_protocol::{R10RemotePacket, R10RemotePacketKind};
use super::r10_remote_policy::{R10RemoteAction, R10RemotePolicy, R10RemoteScreen};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R10BleReaderRemoteGuardMode {
    Disabled,
    ProbeOnly,
    ReaderRemote,
}

impl R10BleReaderRemoteGuardMode {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::ProbeOnly => "probe_only",
            Self::ReaderRemote => "reader_remote",
        }
    }

    pub const fn allows_reader_remote(&self) -> bool {
        matches!(self, Self::ReaderRemote)
    }
}

impl From<R10BleDeviceTaskX4DeployMode> for R10BleReaderRemoteGuardMode {
    fn from(value: R10BleDeviceTaskX4DeployMode) -> Self {
        match value {
            R10BleDeviceTaskX4DeployMode::Disabled => Self::Disabled,
            R10BleDeviceTaskX4DeployMode::ProbeOnly => Self::ProbeOnly,
            R10BleDeviceTaskX4DeployMode::ReaderRemote => Self::ReaderRemote,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R10BleReaderRemoteGuardReason {
    Disabled,
    ProbeOnlyLogOnly,
    ReaderRemoteLocked,
    NotReaderScreen,
    WrongHandle,
    BadPacket,
    NoMotion,
    Debounced,
    Accepted,
}

impl R10BleReaderRemoteGuardReason {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::ProbeOnlyLogOnly => "probe_log_only",
            Self::ReaderRemoteLocked => "reader_locked",
            Self::NotReaderScreen => "not_reader",
            Self::WrongHandle => "wrong_handle",
            Self::BadPacket => "bad_packet",
            Self::NoMotion => "no_motion",
            Self::Debounced => "debounced",
            Self::Accepted => "accepted",
        }
    }

    pub const fn is_accept(&self) -> bool {
        matches!(self, Self::Accepted)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R10BleReaderRemoteGuardRecord {
    pub event: &'static str,
    pub mode: &'static str,
    pub screen: &'static str,
    pub reason: &'static str,
    pub reader: &'static str,
    pub status: &'static str,
}

impl R10BleReaderRemoteGuardRecord {
    pub const fn new(
        mode: R10BleReaderRemoteGuardMode,
        screen: R10RemoteScreen,
        reason: R10BleReaderRemoteGuardReason,
    ) -> Self {
        Self {
            event: "ble_reader_guard",
            mode: mode.as_str(),
            screen: reader_remote_screen_label(screen),
            reason: reason.as_str(),
            reader: reader_remote_reader_label(reason),
            status: reader_remote_status_label(reason),
        }
    }

    pub fn is_monitor_safe(&self) -> bool {
        super::r10_ble_device_task::r10_ble_device_task_monitor_label_is_safe(self.event)
            && super::r10_ble_device_task::r10_ble_device_task_monitor_label_is_safe(self.mode)
            && super::r10_ble_device_task::r10_ble_device_task_monitor_label_is_safe(self.screen)
            && super::r10_ble_device_task::r10_ble_device_task_monitor_label_is_safe(self.reason)
            && super::r10_ble_device_task::r10_ble_device_task_monitor_label_is_safe(self.reader)
            && super::r10_ble_device_task::r10_ble_device_task_monitor_label_is_safe(self.status)
    }
}

pub const fn reader_remote_screen_label(screen: R10RemoteScreen) -> &'static str {
    match screen {
        R10RemoteScreen::Reader => "reader",
        R10RemoteScreen::Home => "home",
        R10RemoteScreen::Library => "library",
        R10RemoteScreen::Settings => "settings",
        R10RemoteScreen::WifiTransfer => "wifi",
        R10RemoteScreen::Sleep => "sleep",
        R10RemoteScreen::Confirm => "confirm",
        R10RemoteScreen::Unknown => "unknown",
    }
}

pub const fn reader_remote_reader_label(reason: R10BleReaderRemoteGuardReason) -> &'static str {
    match reason {
        R10BleReaderRemoteGuardReason::Accepted => "reader_on",
        _ => "reader_off",
    }
}

pub const fn reader_remote_status_label(reason: R10BleReaderRemoteGuardReason) -> &'static str {
    match reason {
        R10BleReaderRemoteGuardReason::Accepted => "ok",
        R10BleReaderRemoteGuardReason::Debounced => "ignored",
        R10BleReaderRemoteGuardReason::NoMotion => "ignored",
        _ => "blocked",
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R10BleReaderRemoteGuardDecision {
    pub action: R10RemoteAction,
    pub injection: R10InputInjection,
    pub reason: R10BleReaderRemoteGuardReason,
    pub record: R10BleReaderRemoteGuardRecord,
}

impl R10BleReaderRemoteGuardDecision {
    pub const fn new(
        mode: R10BleReaderRemoteGuardMode,
        screen: R10RemoteScreen,
        action: R10RemoteAction,
        reason: R10BleReaderRemoteGuardReason,
    ) -> Self {
        Self {
            action,
            injection: R10InputInjection::from_remote_action(action),
            reason,
            record: R10BleReaderRemoteGuardRecord::new(mode, screen, reason),
        }
    }

    pub const fn accepted(&self) -> bool {
        self.reason.is_accept()
    }

    pub const fn should_enqueue(&self) -> bool {
        self.accepted() && matches!(self.injection, R10InputInjection::Press(_))
    }

    pub fn is_monitor_safe(&self) -> bool {
        self.record.is_monitor_safe()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R10BleReaderRemoteGuard {
    mode: R10BleReaderRemoteGuardMode,
    explicit_reader_remote: bool,
    policy: R10RemotePolicy,
}

impl R10BleReaderRemoteGuard {
    pub const fn disabled() -> Self {
        Self {
            mode: R10BleReaderRemoteGuardMode::Disabled,
            explicit_reader_remote: false,
            policy: R10RemotePolicy::disabled(),
        }
    }

    pub const fn probe_only() -> Self {
        Self {
            mode: R10BleReaderRemoteGuardMode::ProbeOnly,
            explicit_reader_remote: false,
            policy: R10RemotePolicy::disabled(),
        }
    }

    pub const fn reader_remote_locked() -> Self {
        Self {
            mode: R10BleReaderRemoteGuardMode::ReaderRemote,
            explicit_reader_remote: false,
            policy: R10RemotePolicy {
                enabled: false,
                screen: R10RemoteScreen::Unknown,
                debounce: super::r10_remote_policy::R10RemoteDebounce::new(3500),
            },
        }
    }

    pub const fn reader_remote_unlocked_for_reader(debounce_ms: u64) -> Self {
        Self {
            mode: R10BleReaderRemoteGuardMode::ReaderRemote,
            explicit_reader_remote: true,
            policy: R10RemotePolicy::enabled_for_reader(debounce_ms),
        }
    }

    pub const fn from_deploy_mode(mode: R10BleDeviceTaskX4DeployMode) -> Self {
        match mode {
            R10BleDeviceTaskX4DeployMode::Disabled => Self::disabled(),
            R10BleDeviceTaskX4DeployMode::ProbeOnly => Self::probe_only(),
            R10BleDeviceTaskX4DeployMode::ReaderRemote => Self::reader_remote_locked(),
        }
    }

    pub const fn mode(&self) -> R10BleReaderRemoteGuardMode {
        self.mode
    }

    pub const fn explicit_reader_remote(&self) -> bool {
        self.explicit_reader_remote
    }

    pub const fn screen(&self) -> R10RemoteScreen {
        self.policy.screen
    }

    pub fn set_screen(&mut self, screen: R10RemoteScreen) {
        self.policy.screen = screen;
    }

    pub fn unlock_reader_remote_for_reader(&mut self, debounce_ms: u64) {
        self.mode = R10BleReaderRemoteGuardMode::ReaderRemote;
        self.explicit_reader_remote = true;
        self.policy = R10RemotePolicy::enabled_for_reader(debounce_ms);
    }

    pub fn lock_reader_remote(&mut self) {
        self.explicit_reader_remote = false;
        self.policy.enabled = false;
        self.policy.debounce.reset();
    }

    pub fn handle_notify(
        &mut self,
        screen: R10RemoteScreen,
        handle: u16,
        payload: &[u8],
        now_ms: u64,
    ) -> R10BleReaderRemoteGuardDecision {
        self.policy.screen = screen;

        if self.mode == R10BleReaderRemoteGuardMode::Disabled {
            return R10BleReaderRemoteGuardDecision::new(
                self.mode,
                screen,
                R10RemoteAction::None,
                R10BleReaderRemoteGuardReason::Disabled,
            );
        }

        if self.mode == R10BleReaderRemoteGuardMode::ProbeOnly {
            return R10BleReaderRemoteGuardDecision::new(
                self.mode,
                screen,
                R10RemoteAction::None,
                R10BleReaderRemoteGuardReason::ProbeOnlyLogOnly,
            );
        }

        if !self.explicit_reader_remote {
            return R10BleReaderRemoteGuardDecision::new(
                self.mode,
                screen,
                R10RemoteAction::None,
                R10BleReaderRemoteGuardReason::ReaderRemoteLocked,
            );
        }

        if !R10RemotePolicy::screen_allows_reader_action(screen) {
            return R10BleReaderRemoteGuardDecision::new(
                self.mode,
                screen,
                R10RemoteAction::None,
                R10BleReaderRemoteGuardReason::NotReaderScreen,
            );
        }

        if handle != R10_BLE_LIVE_NOTIFY_VALUE_HANDLE {
            return R10BleReaderRemoteGuardDecision::new(
                self.mode,
                screen,
                R10RemoteAction::None,
                R10BleReaderRemoteGuardReason::WrongHandle,
            );
        }

        match R10RemotePacket::classify(payload) {
            R10RemotePacketKind::RemoteMotion => {}
            R10RemotePacketKind::RemoteNoEvent => {
                return R10BleReaderRemoteGuardDecision::new(
                    self.mode,
                    screen,
                    R10RemoteAction::None,
                    R10BleReaderRemoteGuardReason::NoMotion,
                );
            }
            _ => {
                return R10BleReaderRemoteGuardDecision::new(
                    self.mode,
                    screen,
                    R10RemoteAction::None,
                    R10BleReaderRemoteGuardReason::BadPacket,
                );
            }
        }

        self.policy.enabled = true;
        let action = self.policy.handle_packet(now_ms, payload);

        if action == R10RemoteAction::None {
            return R10BleReaderRemoteGuardDecision::new(
                self.mode,
                screen,
                action,
                R10BleReaderRemoteGuardReason::Debounced,
            );
        }

        R10BleReaderRemoteGuardDecision::new(
            self.mode,
            screen,
            action,
            R10BleReaderRemoteGuardReason::Accepted,
        )
    }
}

impl Default for R10BleReaderRemoteGuard {
    fn default() -> Self {
        Self::disabled()
    }
}

#[cfg(all(target_arch = "riscv32", feature = "r10-ble-host"))]
pub fn r10_ble_x4_reader_remote_try_enqueue_guarded_notify(
    guard: &mut R10BleReaderRemoteGuard,
    screen: R10RemoteScreen,
    handle: u16,
    payload: &[u8],
    now_ms: u64,
) -> bool {
    let decision = guard.handle_notify(screen, handle, payload, now_ms);

    if decision.should_enqueue() {
        try_enqueue_remote_action(decision.action)
    } else {
        false
    }
}

#[cfg(all(target_arch = "riscv32", feature = "r10-ble-host"))]
pub fn r10_ble_x4_emit_reader_remote_guard_startup_log() {
    let record = R10BleReaderRemoteGuardRecord::new(
        R10BleReaderRemoteGuardMode::ReaderRemote,
        R10RemoteScreen::Reader,
        R10BleReaderRemoteGuardReason::ReaderRemoteLocked,
    );

    esp_println::println!(
        "rustmix event={} mode={} screen={} reason={} reader={} status={}",
        record.event,
        record.mode,
        record.screen,
        record.reason,
        record.reader,
        record.status
    );

    esp_println::println!(
        "rustmix event=ble_reader_guard mode=reader_remote screen=reader reason=explicit_opt_in reader=reader_off status=ready"
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rustmix_x4::contracts::input_semantics::RustmixReaderAction;
    use crate::rustmix_x4::x4_kernel::board::button::Button;
    use crate::rustmix_x4::x4_kernel::drivers::input::Event;

    const MOTION: [u8; 16] = [0x02, 0x02, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x04];
    const NO_EVENT: [u8; 16] = [0x02, 0x00, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x02];
    const BAD: [u8; 16] = [0x02, 0x02, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x05];

    #[test]
    fn r10_ble_reader_remote_guard_defaults_disabled() {
        let mut guard = R10BleReaderRemoteGuard::default();
        let decision = guard.handle_notify(
            R10RemoteScreen::Reader,
            R10_BLE_LIVE_NOTIFY_VALUE_HANDLE,
            &MOTION,
            10_000,
        );

        assert_eq!(guard.mode(), R10BleReaderRemoteGuardMode::Disabled);
        assert_eq!(decision.reason, R10BleReaderRemoteGuardReason::Disabled);
        assert_eq!(decision.action, R10RemoteAction::None);
        assert!(!decision.should_enqueue());
        assert!(decision.is_monitor_safe());
    }

    #[test]
    fn r10_ble_reader_remote_guard_probe_only_is_log_only() {
        let mut guard = R10BleReaderRemoteGuard::probe_only();
        let decision = guard.handle_notify(
            R10RemoteScreen::Reader,
            R10_BLE_LIVE_NOTIFY_VALUE_HANDLE,
            &MOTION,
            10_000,
        );

        assert_eq!(
            decision.reason,
            R10BleReaderRemoteGuardReason::ProbeOnlyLogOnly
        );
        assert_eq!(decision.record.reader, "reader_off");
        assert!(!decision.should_enqueue());
    }

    #[test]
    fn r10_ble_reader_remote_guard_reader_remote_starts_locked() {
        let mut guard =
            R10BleReaderRemoteGuard::from_deploy_mode(R10BleDeviceTaskX4DeployMode::ReaderRemote);
        let decision = guard.handle_notify(
            R10RemoteScreen::Reader,
            R10_BLE_LIVE_NOTIFY_VALUE_HANDLE,
            &MOTION,
            10_000,
        );

        assert_eq!(guard.mode(), R10BleReaderRemoteGuardMode::ReaderRemote);
        assert!(!guard.explicit_reader_remote());
        assert_eq!(
            decision.reason,
            R10BleReaderRemoteGuardReason::ReaderRemoteLocked
        );
        assert!(!decision.should_enqueue());
    }

    #[test]
    fn r10_ble_reader_remote_guard_unlocked_reader_motion_maps_to_next_page() {
        let mut guard = R10BleReaderRemoteGuard::reader_remote_unlocked_for_reader(3500);
        let decision = guard.handle_notify(
            R10RemoteScreen::Reader,
            R10_BLE_LIVE_NOTIFY_VALUE_HANDLE,
            &MOTION,
            10_000,
        );

        assert_eq!(decision.reason, R10BleReaderRemoteGuardReason::Accepted);
        assert_eq!(
            decision.action,
            R10RemoteAction::Reader(RustmixReaderAction::NextPage)
        );
        assert_eq!(
            decision.injection,
            R10InputInjection::Press(Event::Press(Button::VolDown))
        );
        assert_eq!(decision.record.reader, "reader_on");
        assert!(decision.should_enqueue());
        assert!(decision.is_monitor_safe());
    }

    #[test]
    fn r10_ble_reader_remote_guard_blocks_non_reader_screen() {
        let mut guard = R10BleReaderRemoteGuard::reader_remote_unlocked_for_reader(3500);
        let decision = guard.handle_notify(
            R10RemoteScreen::Settings,
            R10_BLE_LIVE_NOTIFY_VALUE_HANDLE,
            &MOTION,
            10_000,
        );

        assert_eq!(
            decision.reason,
            R10BleReaderRemoteGuardReason::NotReaderScreen
        );
        assert_eq!(decision.action, R10RemoteAction::None);
        assert!(!decision.should_enqueue());
    }

    #[test]
    fn r10_ble_reader_remote_guard_blocks_wrong_notify_handle() {
        let mut guard = R10BleReaderRemoteGuard::reader_remote_unlocked_for_reader(3500);
        let decision = guard.handle_notify(R10RemoteScreen::Reader, 0xffff, &MOTION, 10_000);

        assert_eq!(decision.reason, R10BleReaderRemoteGuardReason::WrongHandle);
        assert_eq!(decision.action, R10RemoteAction::None);
    }

    #[test]
    fn r10_ble_reader_remote_guard_blocks_bad_payload() {
        let mut guard = R10BleReaderRemoteGuard::reader_remote_unlocked_for_reader(3500);
        let decision = guard.handle_notify(
            R10RemoteScreen::Reader,
            R10_BLE_LIVE_NOTIFY_VALUE_HANDLE,
            &BAD,
            10_000,
        );

        assert_eq!(decision.reason, R10BleReaderRemoteGuardReason::BadPacket);
        assert_eq!(decision.action, R10RemoteAction::None);
    }

    #[test]
    fn r10_ble_reader_remote_guard_ignores_no_motion_packet() {
        let mut guard = R10BleReaderRemoteGuard::reader_remote_unlocked_for_reader(3500);
        let decision = guard.handle_notify(
            R10RemoteScreen::Reader,
            R10_BLE_LIVE_NOTIFY_VALUE_HANDLE,
            &NO_EVENT,
            10_000,
        );

        assert_eq!(decision.reason, R10BleReaderRemoteGuardReason::NoMotion);
        assert_eq!(decision.action, R10RemoteAction::None);
        assert_eq!(decision.record.status, "ignored");
    }

    #[test]
    fn r10_ble_reader_remote_guard_reuses_existing_debounce() {
        let mut guard = R10BleReaderRemoteGuard::reader_remote_unlocked_for_reader(3500);

        let first = guard.handle_notify(
            R10RemoteScreen::Reader,
            R10_BLE_LIVE_NOTIFY_VALUE_HANDLE,
            &MOTION,
            10_000,
        );
        let second = guard.handle_notify(
            R10RemoteScreen::Reader,
            R10_BLE_LIVE_NOTIFY_VALUE_HANDLE,
            &MOTION,
            11_000,
        );
        let third = guard.handle_notify(
            R10RemoteScreen::Reader,
            R10_BLE_LIVE_NOTIFY_VALUE_HANDLE,
            &MOTION,
            14_000,
        );

        assert_eq!(first.reason, R10BleReaderRemoteGuardReason::Accepted);
        assert_eq!(second.reason, R10BleReaderRemoteGuardReason::Debounced);
        assert_eq!(third.reason, R10BleReaderRemoteGuardReason::Accepted);
    }

    #[test]
    fn r10_ble_reader_remote_guard_lock_disables_navigation_again() {
        let mut guard = R10BleReaderRemoteGuard::reader_remote_unlocked_for_reader(3500);

        let accepted = guard.handle_notify(
            R10RemoteScreen::Reader,
            R10_BLE_LIVE_NOTIFY_VALUE_HANDLE,
            &MOTION,
            10_000,
        );
        guard.lock_reader_remote();
        let locked = guard.handle_notify(
            R10RemoteScreen::Reader,
            R10_BLE_LIVE_NOTIFY_VALUE_HANDLE,
            &MOTION,
            20_000,
        );

        assert!(accepted.should_enqueue());
        assert_eq!(
            locked.reason,
            R10BleReaderRemoteGuardReason::ReaderRemoteLocked
        );
        assert!(!locked.should_enqueue());
    }

    #[test]
    fn r10_ble_reader_remote_guard_unlock_sets_reader_screen() {
        let mut guard = R10BleReaderRemoteGuard::reader_remote_locked();

        guard.unlock_reader_remote_for_reader(3500);

        assert!(guard.explicit_reader_remote());
        assert_eq!(guard.screen(), R10RemoteScreen::Reader);
    }

    #[test]
    fn r10_ble_reader_remote_guard_records_are_monitor_safe() {
        let modes = [
            R10BleReaderRemoteGuardMode::Disabled,
            R10BleReaderRemoteGuardMode::ProbeOnly,
            R10BleReaderRemoteGuardMode::ReaderRemote,
        ];
        let reasons = [
            R10BleReaderRemoteGuardReason::Disabled,
            R10BleReaderRemoteGuardReason::ProbeOnlyLogOnly,
            R10BleReaderRemoteGuardReason::ReaderRemoteLocked,
            R10BleReaderRemoteGuardReason::NotReaderScreen,
            R10BleReaderRemoteGuardReason::WrongHandle,
            R10BleReaderRemoteGuardReason::BadPacket,
            R10BleReaderRemoteGuardReason::NoMotion,
            R10BleReaderRemoteGuardReason::Debounced,
            R10BleReaderRemoteGuardReason::Accepted,
        ];

        for mode in modes {
            for reason in reasons {
                let record =
                    R10BleReaderRemoteGuardRecord::new(mode, R10RemoteScreen::Reader, reason);
                assert!(record.is_monitor_safe());
            }
        }
    }

    #[test]
    fn r10_ble_reader_remote_guard_screen_labels_are_monitor_safe() {
        let screens = [
            R10RemoteScreen::Reader,
            R10RemoteScreen::Home,
            R10RemoteScreen::Library,
            R10RemoteScreen::Settings,
            R10RemoteScreen::WifiTransfer,
            R10RemoteScreen::Sleep,
            R10RemoteScreen::Confirm,
            R10RemoteScreen::Unknown,
        ];

        for screen in screens {
            assert!(
                super::super::r10_ble_device_task::r10_ble_device_task_monitor_label_is_safe(
                    reader_remote_screen_label(screen)
                )
            );
        }
    }

    #[test]
    fn r10_ble_reader_remote_guard_deploy_mode_mapping_keeps_probe_safe() {
        assert_eq!(
            R10BleReaderRemoteGuard::from_deploy_mode(R10BleDeviceTaskX4DeployMode::Disabled)
                .mode(),
            R10BleReaderRemoteGuardMode::Disabled
        );
        assert_eq!(
            R10BleReaderRemoteGuard::from_deploy_mode(R10BleDeviceTaskX4DeployMode::ProbeOnly)
                .mode(),
            R10BleReaderRemoteGuardMode::ProbeOnly
        );
        assert_eq!(
            R10BleReaderRemoteGuard::from_deploy_mode(R10BleDeviceTaskX4DeployMode::ReaderRemote)
                .mode(),
            R10BleReaderRemoteGuardMode::ReaderRemote
        );

        let mut probe =
            R10BleReaderRemoteGuard::from_deploy_mode(R10BleDeviceTaskX4DeployMode::ProbeOnly);
        let decision = probe.handle_notify(
            R10RemoteScreen::Reader,
            R10_BLE_LIVE_NOTIFY_VALUE_HANDLE,
            &MOTION,
            10_000,
        );

        assert_eq!(
            decision.reason,
            R10BleReaderRemoteGuardReason::ProbeOnlyLogOnly
        );
        assert!(!decision.should_enqueue());
    }
}
