// R10 BLE Settings UI + runtime-mode persistence.
//
// This is the Settings > Controls > R10 BLE Remote controller. It is intentionally
// small, no-alloc, and host-testable. Runtime ReaderRemote remains guarded:
// Off is the default, ProbeOnly is log-only, and ReaderRemote must be explicitly
// confirmed before persistence/enabling.

use super::r10_ble_device_task::R10BleDeviceTaskX4DeployMode;
use super::r10_ble_reader_remote_guard::{
    R10BleReaderRemoteGuard, R10BleReaderRemoteGuardMode, R10BleReaderRemoteGuardReason,
    R10BleReaderRemoteGuardRecord,
};
use super::r10_remote_policy::R10RemoteScreen;

pub const R10_BLE_SETTINGS_PATH: &str = "Settings > Controls > R10 BLE Remote";
pub const R10_BLE_SETTINGS_TAB: &str = "Controls";
pub const R10_BLE_SETTINGS_CONTROL_LABEL: &str = "R10 BLE Remote";
pub const R10_BLE_SETTINGS_FILE_NAME: &str = "R10BLE.TXT";
pub const R10_BLE_SETTINGS_RECORD_PREFIX: &str = "R10BLE_MODE=";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R10BleSettingsMode {
    Off,
    ProbeOnly,
    ReaderRemote,
}

impl R10BleSettingsMode {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::ProbeOnly => "probe_only",
            Self::ReaderRemote => "reader_remote",
        }
    }

    pub const fn display_label(&self) -> &'static str {
        match self {
            Self::Off => "Off",
            Self::ProbeOnly => "Probe Only",
            Self::ReaderRemote => "Reader Remote",
        }
    }

    pub const fn status_label(&self) -> &'static str {
        match self {
            Self::Off => "Off",
            Self::ProbeOnly => "ProbeOnly log-only",
            Self::ReaderRemote => "ReaderRemote guarded",
        }
    }

    pub const fn safety_label(&self) -> &'static str {
        match self {
            Self::Off => "off_default",
            Self::ProbeOnly => "probe_log_only",
            Self::ReaderRemote => "reader_guarded",
        }
    }

    pub const fn requires_confirmation(&self) -> bool {
        matches!(self, Self::ReaderRemote)
    }

    pub const fn to_deploy_mode(&self) -> R10BleDeviceTaskX4DeployMode {
        match self {
            Self::Off => R10BleDeviceTaskX4DeployMode::Disabled,
            Self::ProbeOnly => R10BleDeviceTaskX4DeployMode::ProbeOnly,
            Self::ReaderRemote => R10BleDeviceTaskX4DeployMode::ReaderRemote,
        }
    }

    pub const fn to_guard_mode(&self) -> R10BleReaderRemoteGuardMode {
        match self {
            Self::Off => R10BleReaderRemoteGuardMode::Disabled,
            Self::ProbeOnly => R10BleReaderRemoteGuardMode::ProbeOnly,
            Self::ReaderRemote => R10BleReaderRemoteGuardMode::ReaderRemote,
        }
    }
}

impl From<R10BleDeviceTaskX4DeployMode> for R10BleSettingsMode {
    fn from(value: R10BleDeviceTaskX4DeployMode) -> Self {
        match value {
            R10BleDeviceTaskX4DeployMode::Disabled => Self::Off,
            R10BleDeviceTaskX4DeployMode::ProbeOnly => Self::ProbeOnly,
            R10BleDeviceTaskX4DeployMode::ReaderRemote => Self::ReaderRemote,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R10BleSettingsAction {
    Open,
    SelectOff,
    SelectProbeOnly,
    SelectReaderRemote,
    ConfirmReaderRemote,
    CancelReaderRemote,
    QuickDisable,
}

impl R10BleSettingsAction {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::SelectOff => "select_off",
            Self::SelectProbeOnly => "select_probe",
            Self::SelectReaderRemote => "select_reader",
            Self::ConfirmReaderRemote => "confirm_reader",
            Self::CancelReaderRemote => "cancel_reader",
            Self::QuickDisable => "quick_disable",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R10BleSettingsStatus {
    Viewing,
    Persisted,
    ConfirmationRequired,
    Cancelled,
    Disabled,
}

impl R10BleSettingsStatus {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Viewing => "viewing",
            Self::Persisted => "persisted",
            Self::ConfirmationRequired => "confirm_required",
            Self::Cancelled => "cancelled",
            Self::Disabled => "disabled",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R10BleSettingsRow {
    pub mode: R10BleSettingsMode,
    pub label: &'static str,
    pub status: &'static str,
    pub selected: bool,
    pub requires_confirmation: bool,
}

impl R10BleSettingsRow {
    pub const fn new(mode: R10BleSettingsMode, selected_mode: R10BleSettingsMode) -> Self {
        Self {
            mode,
            label: mode.display_label(),
            status: mode.status_label(),
            selected: mode as u8 == selected_mode as u8,
            requires_confirmation: mode.requires_confirmation(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R10BleSettingsScreen {
    pub title: &'static str,
    pub path: &'static str,
    pub rows: [R10BleSettingsRow; 3],
    pub pending_confirmation: Option<R10BleSettingsMode>,
    pub quick_disable_available: bool,
}

impl R10BleSettingsScreen {
    pub const fn new(mode: R10BleSettingsMode, pending: Option<R10BleSettingsMode>) -> Self {
        Self {
            title: "R10 BLE Remote",
            path: R10_BLE_SETTINGS_PATH,
            rows: [
                R10BleSettingsRow::new(R10BleSettingsMode::Off, mode),
                R10BleSettingsRow::new(R10BleSettingsMode::ProbeOnly, mode),
                R10BleSettingsRow::new(R10BleSettingsMode::ReaderRemote, mode),
            ],
            pending_confirmation: pending,
            quick_disable_available: !matches!(mode, R10BleSettingsMode::Off),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R10BleSettingsPersistenceRecord {
    pub mode: R10BleSettingsMode,
    pub explicit_user_change: bool,
    pub reader_remote_confirmed: bool,
}

impl R10BleSettingsPersistenceRecord {
    pub const fn default_off() -> Self {
        Self {
            mode: R10BleSettingsMode::Off,
            explicit_user_change: false,
            reader_remote_confirmed: false,
        }
    }

    pub const fn new(
        mode: R10BleSettingsMode,
        explicit_user_change: bool,
        reader_remote_confirmed: bool,
    ) -> Self {
        Self {
            mode,
            explicit_user_change,
            reader_remote_confirmed,
        }
    }

    pub fn parse(input: &str) -> Self {
        let trimmed = input.trim();

        if trimmed == "off" || trimmed == "R10BLE_MODE=off" {
            Self::new(R10BleSettingsMode::Off, true, false)
        } else if trimmed == "probe_only" || trimmed == "R10BLE_MODE=probe_only" {
            Self::new(R10BleSettingsMode::ProbeOnly, true, false)
        } else if trimmed == "reader_remote_confirmed"
            || trimmed == "R10BLE_MODE=reader_remote_confirmed"
        {
            Self::new(R10BleSettingsMode::ReaderRemote, true, true)
        } else if trimmed == "reader_remote" || trimmed == "R10BLE_MODE=reader_remote" {
            // Safety: unconfirmed ReaderRemote records are not trusted.
            Self::default_off()
        } else {
            Self::default_off()
        }
    }

    pub const fn stable_value(&self) -> &'static str {
        match (self.mode, self.reader_remote_confirmed) {
            (R10BleSettingsMode::Off, _) => "off",
            (R10BleSettingsMode::ProbeOnly, _) => "probe_only",
            (R10BleSettingsMode::ReaderRemote, true) => "reader_remote_confirmed",
            (R10BleSettingsMode::ReaderRemote, false) => "off",
        }
    }

    pub fn write_line<W: core::fmt::Write>(&self, out: &mut W) -> core::fmt::Result {
        out.write_str(R10_BLE_SETTINGS_RECORD_PREFIX)?;
        out.write_str(self.stable_value())?;
        out.write_char('\n')
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R10BleSettingsEventRecord {
    pub event: &'static str,
    pub path: &'static str,
    pub action: &'static str,
    pub mode: &'static str,
    pub safety: &'static str,
    pub persisted: &'static str,
    pub reader: &'static str,
    pub status: &'static str,
}

impl R10BleSettingsEventRecord {
    pub const fn new(
        action: R10BleSettingsAction,
        mode: R10BleSettingsMode,
        persisted: bool,
        status: R10BleSettingsStatus,
    ) -> Self {
        Self {
            event: "ble_settings",
            path: "settings_controls_r_ble",
            action: action.as_str(),
            mode: mode.as_str(),
            safety: mode.safety_label(),
            persisted: if persisted { "yes" } else { "no" },
            reader: if matches!(mode, R10BleSettingsMode::ReaderRemote)
                && matches!(status, R10BleSettingsStatus::Persisted)
            {
                "reader_guarded"
            } else {
                "reader_off"
            },
            status: status.as_str(),
        }
    }

    pub fn is_monitor_safe(&self) -> bool {
        super::r10_ble_device_task::r10_ble_device_task_monitor_label_is_safe(self.event)
            && super::r10_ble_device_task::r10_ble_device_task_monitor_label_is_safe(self.path)
            && super::r10_ble_device_task::r10_ble_device_task_monitor_label_is_safe(self.action)
            && super::r10_ble_device_task::r10_ble_device_task_monitor_label_is_safe(self.mode)
            && super::r10_ble_device_task::r10_ble_device_task_monitor_label_is_safe(self.safety)
            && super::r10_ble_device_task::r10_ble_device_task_monitor_label_is_safe(self.persisted)
            && super::r10_ble_device_task::r10_ble_device_task_monitor_label_is_safe(self.reader)
            && super::r10_ble_device_task::r10_ble_device_task_monitor_label_is_safe(self.status)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R10BleSettingsTransition {
    pub mode: R10BleSettingsMode,
    pub pending_confirmation: Option<R10BleSettingsMode>,
    pub should_persist: bool,
    pub persistence: R10BleSettingsPersistenceRecord,
    pub record: R10BleSettingsEventRecord,
}

impl R10BleSettingsTransition {
    pub const fn deploy_mode(&self) -> R10BleDeviceTaskX4DeployMode {
        self.mode.to_deploy_mode()
    }

    pub const fn guard_mode(&self) -> R10BleReaderRemoteGuardMode {
        self.mode.to_guard_mode()
    }

    pub fn reader_guard(&self) -> R10BleReaderRemoteGuard {
        match (self.mode, self.persistence.reader_remote_confirmed) {
            (R10BleSettingsMode::Off, _) => R10BleReaderRemoteGuard::disabled(),
            (R10BleSettingsMode::ProbeOnly, _) => R10BleReaderRemoteGuard::probe_only(),
            (R10BleSettingsMode::ReaderRemote, true) => {
                R10BleReaderRemoteGuard::reader_remote_unlocked_for_reader(3500)
            }
            (R10BleSettingsMode::ReaderRemote, false) => {
                R10BleReaderRemoteGuard::reader_remote_locked()
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R10BleSettingsController {
    mode: R10BleSettingsMode,
    pending_confirmation: Option<R10BleSettingsMode>,
    explicit_user_change: bool,
}

impl R10BleSettingsController {
    pub const fn new() -> Self {
        Self {
            mode: R10BleSettingsMode::Off,
            pending_confirmation: None,
            explicit_user_change: false,
        }
    }

    pub const fn from_persistence(record: R10BleSettingsPersistenceRecord) -> Self {
        Self {
            mode: match (record.mode, record.reader_remote_confirmed) {
                (R10BleSettingsMode::ReaderRemote, false) => R10BleSettingsMode::Off,
                _ => record.mode,
            },
            pending_confirmation: None,
            explicit_user_change: record.explicit_user_change,
        }
    }

    pub const fn mode(&self) -> R10BleSettingsMode {
        self.mode
    }

    pub const fn explicit_user_change(&self) -> bool {
        self.explicit_user_change
    }

    pub const fn pending_confirmation(&self) -> Option<R10BleSettingsMode> {
        self.pending_confirmation
    }

    pub const fn screen(&self) -> R10BleSettingsScreen {
        R10BleSettingsScreen::new(self.mode, self.pending_confirmation)
    }

    pub fn apply(&mut self, action: R10BleSettingsAction) -> R10BleSettingsTransition {
        match action {
            R10BleSettingsAction::Open => self.transition(
                action,
                self.mode,
                self.pending_confirmation,
                false,
                R10BleSettingsStatus::Viewing,
                false,
            ),
            R10BleSettingsAction::SelectOff => {
                self.mode = R10BleSettingsMode::Off;
                self.pending_confirmation = None;
                self.explicit_user_change = true;
                self.transition(
                    action,
                    self.mode,
                    None,
                    true,
                    R10BleSettingsStatus::Persisted,
                    false,
                )
            }
            R10BleSettingsAction::SelectProbeOnly => {
                self.mode = R10BleSettingsMode::ProbeOnly;
                self.pending_confirmation = None;
                self.explicit_user_change = true;
                self.transition(
                    action,
                    self.mode,
                    None,
                    true,
                    R10BleSettingsStatus::Persisted,
                    false,
                )
            }
            R10BleSettingsAction::SelectReaderRemote => {
                self.pending_confirmation = Some(R10BleSettingsMode::ReaderRemote);
                self.transition(
                    action,
                    self.mode,
                    self.pending_confirmation,
                    false,
                    R10BleSettingsStatus::ConfirmationRequired,
                    false,
                )
            }
            R10BleSettingsAction::ConfirmReaderRemote => {
                if self.pending_confirmation == Some(R10BleSettingsMode::ReaderRemote) {
                    self.mode = R10BleSettingsMode::ReaderRemote;
                    self.pending_confirmation = None;
                    self.explicit_user_change = true;
                    self.transition(
                        action,
                        self.mode,
                        None,
                        true,
                        R10BleSettingsStatus::Persisted,
                        true,
                    )
                } else {
                    self.transition(
                        action,
                        self.mode,
                        self.pending_confirmation,
                        false,
                        R10BleSettingsStatus::Viewing,
                        false,
                    )
                }
            }
            R10BleSettingsAction::CancelReaderRemote => {
                self.pending_confirmation = None;
                self.transition(
                    action,
                    self.mode,
                    None,
                    false,
                    R10BleSettingsStatus::Cancelled,
                    false,
                )
            }
            R10BleSettingsAction::QuickDisable => {
                self.mode = R10BleSettingsMode::Off;
                self.pending_confirmation = None;
                self.explicit_user_change = true;
                self.transition(
                    action,
                    self.mode,
                    None,
                    true,
                    R10BleSettingsStatus::Disabled,
                    false,
                )
            }
        }
    }

    fn transition(
        &self,
        action: R10BleSettingsAction,
        mode: R10BleSettingsMode,
        pending_confirmation: Option<R10BleSettingsMode>,
        should_persist: bool,
        status: R10BleSettingsStatus,
        reader_remote_confirmed: bool,
    ) -> R10BleSettingsTransition {
        R10BleSettingsTransition {
            mode,
            pending_confirmation,
            should_persist,
            persistence: R10BleSettingsPersistenceRecord::new(
                mode,
                self.explicit_user_change,
                reader_remote_confirmed,
            ),
            record: R10BleSettingsEventRecord::new(action, mode, should_persist, status),
        }
    }
}

impl Default for R10BleSettingsController {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(all(target_arch = "riscv32", feature = "r10-ble-host"))]
pub fn r10_ble_x4_emit_settings_status_startup_log() {
    let controller = R10BleSettingsController::new();
    let screen = controller.screen();
    let record = R10BleSettingsEventRecord::new(
        R10BleSettingsAction::Open,
        controller.mode(),
        false,
        R10BleSettingsStatus::Viewing,
    );

    esp_println::println!(
        "rustmix event={} path={} action={} mode={} safety={} persisted={} reader={} status={}",
        record.event,
        record.path,
        record.action,
        record.mode,
        record.safety,
        record.persisted,
        record.reader,
        record.status
    );

    esp_println::println!(
        "rustmix event=ble_settings path=settings_controls_r_ble title=rten_remote rows=off_probe_reader quick_disable={} status=ready",
        if screen.quick_disable_available {
            "yes"
        } else {
            "no"
        }
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn r10_ble_settings_ui_defaults_to_off() {
        let controller = R10BleSettingsController::new();
        let screen = controller.screen();

        assert_eq!(controller.mode(), R10BleSettingsMode::Off);
        assert!(!controller.explicit_user_change());
        assert_eq!(screen.title, "R10 BLE Remote");
        assert_eq!(screen.path, R10_BLE_SETTINGS_PATH);
        assert_eq!(screen.rows[0].label, "Off");
        assert_eq!(screen.rows[1].label, "Probe Only");
        assert_eq!(screen.rows[2].label, "Reader Remote");
        assert!(screen.rows[0].selected);
        assert!(!screen.quick_disable_available);
    }

    #[test]
    fn r10_ble_settings_ui_probe_only_persists_after_explicit_selection() {
        let mut controller = R10BleSettingsController::new();

        let transition = controller.apply(R10BleSettingsAction::SelectProbeOnly);

        assert_eq!(transition.mode, R10BleSettingsMode::ProbeOnly);
        assert!(transition.should_persist);
        assert_eq!(
            transition.persistence,
            R10BleSettingsPersistenceRecord::new(R10BleSettingsMode::ProbeOnly, true, false)
        );
        assert_eq!(
            transition.deploy_mode(),
            R10BleDeviceTaskX4DeployMode::ProbeOnly
        );
        assert_eq!(transition.record.reader, "reader_off");
        assert!(transition.record.is_monitor_safe());
        assert!(controller.screen().quick_disable_available);
    }

    #[test]
    fn r10_ble_settings_ui_reader_remote_requires_confirmation() {
        let mut controller = R10BleSettingsController::new();

        let selected = controller.apply(R10BleSettingsAction::SelectReaderRemote);

        assert_eq!(controller.mode(), R10BleSettingsMode::Off);
        assert_eq!(
            selected.pending_confirmation,
            Some(R10BleSettingsMode::ReaderRemote)
        );
        assert!(!selected.should_persist);
        assert_eq!(selected.record.status, "confirm_required");
        assert_eq!(selected.record.reader, "reader_off");
    }

    #[test]
    fn r10_ble_settings_ui_reader_remote_confirm_persists_guarded_mode() {
        let mut controller = R10BleSettingsController::new();

        let _ = controller.apply(R10BleSettingsAction::SelectReaderRemote);
        let confirmed = controller.apply(R10BleSettingsAction::ConfirmReaderRemote);

        assert_eq!(controller.mode(), R10BleSettingsMode::ReaderRemote);
        assert!(controller.explicit_user_change());
        assert!(confirmed.should_persist);
        assert_eq!(
            confirmed.persistence.stable_value(),
            "reader_remote_confirmed"
        );
        assert_eq!(
            confirmed.deploy_mode(),
            R10BleDeviceTaskX4DeployMode::ReaderRemote
        );
        assert_eq!(
            confirmed.guard_mode(),
            R10BleReaderRemoteGuardMode::ReaderRemote
        );
        assert_eq!(confirmed.record.reader, "reader_guarded");
        assert!(confirmed.record.is_monitor_safe());
    }

    #[test]
    fn r10_ble_settings_ui_reader_remote_cancel_does_not_persist() {
        let mut controller = R10BleSettingsController::new();

        let _ = controller.apply(R10BleSettingsAction::SelectReaderRemote);
        let cancelled = controller.apply(R10BleSettingsAction::CancelReaderRemote);

        assert_eq!(controller.mode(), R10BleSettingsMode::Off);
        assert_eq!(controller.pending_confirmation(), None);
        assert!(!cancelled.should_persist);
        assert_eq!(cancelled.record.status, "cancelled");
    }

    #[test]
    fn r10_ble_settings_ui_quick_disable_persists_off() {
        let mut controller = R10BleSettingsController::new();

        let _ = controller.apply(R10BleSettingsAction::SelectProbeOnly);
        let disabled = controller.apply(R10BleSettingsAction::QuickDisable);

        assert_eq!(controller.mode(), R10BleSettingsMode::Off);
        assert!(disabled.should_persist);
        assert_eq!(disabled.persistence.stable_value(), "off");
        assert_eq!(
            disabled.deploy_mode(),
            R10BleDeviceTaskX4DeployMode::Disabled
        );
        assert_eq!(disabled.record.status, "disabled");
        assert_eq!(disabled.record.reader, "reader_off");
    }

    #[test]
    fn r10_ble_settings_ui_off_is_persisted_after_explicit_selection_only() {
        let mut controller = R10BleSettingsController::new();
        let open = controller.apply(R10BleSettingsAction::Open);

        assert!(!open.should_persist);
        assert_eq!(
            open.persistence,
            R10BleSettingsPersistenceRecord::new(R10BleSettingsMode::Off, false, false)
        );

        let off = controller.apply(R10BleSettingsAction::SelectOff);

        assert!(off.should_persist);
        assert_eq!(
            off.persistence,
            R10BleSettingsPersistenceRecord::new(R10BleSettingsMode::Off, true, false)
        );
    }

    #[test]
    fn r10_ble_settings_persistence_parses_safe_records() {
        assert_eq!(
            R10BleSettingsPersistenceRecord::parse("R10BLE_MODE=off").mode,
            R10BleSettingsMode::Off
        );
        assert_eq!(
            R10BleSettingsPersistenceRecord::parse("R10BLE_MODE=probe_only").mode,
            R10BleSettingsMode::ProbeOnly
        );

        let reader = R10BleSettingsPersistenceRecord::parse("R10BLE_MODE=reader_remote_confirmed");
        assert_eq!(reader.mode, R10BleSettingsMode::ReaderRemote);
        assert!(reader.reader_remote_confirmed);
    }

    #[test]
    fn r10_ble_settings_persistence_rejects_unconfirmed_reader_remote() {
        let parsed = R10BleSettingsPersistenceRecord::parse("R10BLE_MODE=reader_remote");

        assert_eq!(parsed, R10BleSettingsPersistenceRecord::default_off());
    }

    #[test]
    fn r10_ble_settings_persistence_writes_stable_line() {
        let mut line = std::string::String::new();

        R10BleSettingsPersistenceRecord::new(R10BleSettingsMode::ReaderRemote, true, true)
            .write_line(&mut line)
            .unwrap();

        assert_eq!(line.as_str(), "R10BLE_MODE=reader_remote_confirmed\n");
    }

    #[test]
    fn r10_ble_settings_controller_loads_persisted_probe_only() {
        let controller = R10BleSettingsController::from_persistence(
            R10BleSettingsPersistenceRecord::new(R10BleSettingsMode::ProbeOnly, true, false),
        );

        assert_eq!(controller.mode(), R10BleSettingsMode::ProbeOnly);
        assert!(controller.explicit_user_change());
    }

    #[test]
    fn r10_ble_settings_controller_sanitizes_unconfirmed_reader_remote_load() {
        let controller = R10BleSettingsController::from_persistence(
            R10BleSettingsPersistenceRecord::new(R10BleSettingsMode::ReaderRemote, true, false),
        );

        assert_eq!(controller.mode(), R10BleSettingsMode::Off);
    }

    #[test]
    fn r10_ble_settings_transition_builds_matching_reader_guard() {
        let mut controller = R10BleSettingsController::new();

        let _ = controller.apply(R10BleSettingsAction::SelectReaderRemote);
        let confirmed = controller.apply(R10BleSettingsAction::ConfirmReaderRemote);
        let guard = confirmed.reader_guard();

        assert_eq!(guard.mode(), R10BleReaderRemoteGuardMode::ReaderRemote);
        assert!(guard.explicit_reader_remote());
        assert_eq!(guard.screen(), R10RemoteScreen::Reader);
    }

    #[test]
    fn r10_ble_settings_records_are_monitor_safe() {
        let actions = [
            R10BleSettingsAction::Open,
            R10BleSettingsAction::SelectOff,
            R10BleSettingsAction::SelectProbeOnly,
            R10BleSettingsAction::SelectReaderRemote,
            R10BleSettingsAction::ConfirmReaderRemote,
            R10BleSettingsAction::CancelReaderRemote,
            R10BleSettingsAction::QuickDisable,
        ];
        let modes = [
            R10BleSettingsMode::Off,
            R10BleSettingsMode::ProbeOnly,
            R10BleSettingsMode::ReaderRemote,
        ];

        for action in actions {
            for mode in modes {
                let record = R10BleSettingsEventRecord::new(
                    action,
                    mode,
                    false,
                    R10BleSettingsStatus::Viewing,
                );
                assert!(record.is_monitor_safe());
            }
        }
    }

    #[test]
    fn r10_ble_settings_ui_lives_under_controls_tab() {
        let controller = R10BleSettingsController::new();
        let screen = controller.screen();

        assert_eq!(R10_BLE_SETTINGS_TAB, "Controls");
        assert_eq!(R10_BLE_SETTINGS_CONTROL_LABEL, "R10 BLE Remote");
        assert_eq!(screen.path, "Settings > Controls > R10 BLE Remote");
        assert!(!screen.path.contains("Input"));
    }

    #[test]
    fn r10_ble_settings_status_labels_match_scope() {
        assert_eq!(R10BleSettingsMode::Off.status_label(), "Off");
        assert_eq!(
            R10BleSettingsMode::ProbeOnly.status_label(),
            "ProbeOnly log-only"
        );
        assert_eq!(
            R10BleSettingsMode::ReaderRemote.status_label(),
            "ReaderRemote guarded"
        );
    }

    #[test]
    fn r10_ble_settings_reader_remote_record_uses_guarded_reader_label() {
        let record = R10BleSettingsEventRecord::new(
            R10BleSettingsAction::ConfirmReaderRemote,
            R10BleSettingsMode::ReaderRemote,
            true,
            R10BleSettingsStatus::Persisted,
        );

        assert_eq!(record.reader, "reader_guarded");
        assert!(record.is_monitor_safe());

        let guard_record = R10BleReaderRemoteGuardRecord::new(
            R10BleReaderRemoteGuardMode::ReaderRemote,
            R10RemoteScreen::Reader,
            R10BleReaderRemoteGuardReason::ReaderRemoteLocked,
        );

        assert_eq!(guard_record.reader, "reader_off");
        assert!(guard_record.is_monitor_safe());
    }
}
