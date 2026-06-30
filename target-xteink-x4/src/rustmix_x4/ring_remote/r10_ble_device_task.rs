use super::r10_ble_transport::{
    R10_BLE_RUNTIME_DEFAULT_READER_DEBOUNCE_MS, R10BleRuntimeConfig, R10BleRuntimeMode,
    R10BleRuntimeStartDecision,
};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum R10BleDeviceTaskAction {
    #[default]
    StayDisabled,
    StartProbeOnly,
    StartReaderRemote,
}

impl R10BleDeviceTaskAction {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::StayDisabled => "stay_disabled",
            Self::StartProbeOnly => "start_probe_only",
            Self::StartReaderRemote => "start_reader_remote",
        }
    }

    pub const fn starts_ble(&self) -> bool {
        !matches!(self, Self::StayDisabled)
    }

    pub const fn starts_probe(&self) -> bool {
        matches!(self, Self::StartProbeOnly)
    }

    pub const fn starts_reader_remote(&self) -> bool {
        matches!(self, Self::StartReaderRemote)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R10BleDeviceTaskPlan {
    pub config: R10BleRuntimeConfig,
    pub action: R10BleDeviceTaskAction,
}

impl Default for R10BleDeviceTaskPlan {
    fn default() -> Self {
        Self::from_config(R10BleRuntimeConfig::default())
    }
}

impl R10BleDeviceTaskPlan {
    pub const fn from_config(config: R10BleRuntimeConfig) -> Self {
        let action = match config.start_decision() {
            R10BleRuntimeStartDecision::StayDisabled => R10BleDeviceTaskAction::StayDisabled,
            R10BleRuntimeStartDecision::StartProbe => R10BleDeviceTaskAction::StartProbeOnly,
            R10BleRuntimeStartDecision::StartReaderRemote => {
                R10BleDeviceTaskAction::StartReaderRemote
            }
        };

        Self { config, action }
    }

    pub const fn disabled() -> Self {
        Self::from_config(R10BleRuntimeConfig::disabled())
    }

    pub const fn probe_only_live_r10() -> Self {
        Self::from_config(R10BleRuntimeConfig::probe_only_live_r10())
    }

    pub const fn reader_remote_live_r10() -> Self {
        Self::from_config(R10BleRuntimeConfig::reader_remote_live_r10())
    }

    pub const fn should_start_ble(&self) -> bool {
        self.action.starts_ble()
    }

    pub const fn should_start_probe(&self) -> bool {
        self.action.starts_probe()
    }

    pub const fn should_start_reader_remote(&self) -> bool {
        self.action.starts_reader_remote()
    }

    pub const fn mode(&self) -> R10BleRuntimeMode {
        self.config.mode
    }

    pub const fn reader_debounce_ms(&self) -> u64 {
        self.config.reader_debounce_ms
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R10BleDeviceTaskStartRequest {
    pub action: R10BleDeviceTaskAction,
    pub config: R10BleRuntimeConfig,
}

impl R10BleDeviceTaskStartRequest {
    pub const fn should_run_probe(&self) -> bool {
        matches!(self.action, R10BleDeviceTaskAction::StartProbeOnly)
    }

    pub const fn should_run_reader_remote(&self) -> bool {
        matches!(self.action, R10BleDeviceTaskAction::StartReaderRemote)
    }

    pub const fn should_start_ble(&self) -> bool {
        self.action.starts_ble()
    }

    pub const fn reader_debounce_ms(&self) -> u64 {
        self.config.reader_debounce_ms
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R10BleDeviceTaskLifecycleEvent {
    StartScan,
    TargetSeen,
    Connect,
    Discover,
    Subscribe,
    RemoteStart,
    Poll,
    Notify,
    Timeout,
}

impl R10BleDeviceTaskLifecycleEvent {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::StartScan => "start_scan",
            Self::TargetSeen => "target_seen",
            Self::Connect => "connect",
            Self::Discover => "discover",
            Self::Subscribe => "subscribe",
            Self::RemoteStart => "remote_start",
            Self::Poll => "poll",
            Self::Notify => "notify",
            Self::Timeout => "timeout",
        }
    }

    pub const fn is_terminal(&self) -> bool {
        matches!(self, Self::Notify | Self::Timeout)
    }
}

pub const R10_BLE_DEVICE_TASK_PROBE_LIFECYCLE: [R10BleDeviceTaskLifecycleEvent; 9] = [
    R10BleDeviceTaskLifecycleEvent::StartScan,
    R10BleDeviceTaskLifecycleEvent::TargetSeen,
    R10BleDeviceTaskLifecycleEvent::Connect,
    R10BleDeviceTaskLifecycleEvent::Discover,
    R10BleDeviceTaskLifecycleEvent::Subscribe,
    R10BleDeviceTaskLifecycleEvent::RemoteStart,
    R10BleDeviceTaskLifecycleEvent::Poll,
    R10BleDeviceTaskLifecycleEvent::Notify,
    R10BleDeviceTaskLifecycleEvent::Timeout,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R10BleDeviceTaskRunnerPlan {
    pub request: R10BleDeviceTaskStartRequest,
    pub lifecycle: &'static [R10BleDeviceTaskLifecycleEvent],
    pub reader_events_enabled: bool,
}

impl R10BleDeviceTaskRunnerPlan {
    pub const fn from_start_request(request: R10BleDeviceTaskStartRequest) -> Option<Self> {
        match request.action {
            R10BleDeviceTaskAction::StartProbeOnly => Some(Self {
                request,
                lifecycle: &R10_BLE_DEVICE_TASK_PROBE_LIFECYCLE,
                reader_events_enabled: false,
            }),
            R10BleDeviceTaskAction::StartReaderRemote => Some(Self {
                request,
                lifecycle: &R10_BLE_DEVICE_TASK_PROBE_LIFECYCLE,
                reader_events_enabled: true,
            }),
            R10BleDeviceTaskAction::StayDisabled => None,
        }
    }

    pub fn from_state_start(state: &mut R10BleDeviceTaskState) -> Option<Self> {
        state.start_request().and_then(Self::from_start_request)
    }

    pub const fn should_run_probe(&self) -> bool {
        self.request.should_run_probe()
    }

    pub const fn should_run_reader_remote(&self) -> bool {
        self.request.should_run_reader_remote()
    }

    pub const fn should_emit_reader_events(&self) -> bool {
        self.reader_events_enabled
    }

    pub const fn lifecycle_len(&self) -> usize {
        self.lifecycle.len()
    }

    pub fn lifecycle_contains(&self, event: R10BleDeviceTaskLifecycleEvent) -> bool {
        self.lifecycle.iter().any(|candidate| *candidate == event)
    }

    pub fn lifecycle_label_at(&self, index: usize) -> Option<&'static str> {
        self.lifecycle
            .get(index)
            .map(R10BleDeviceTaskLifecycleEvent::as_str)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R10BleDeviceTaskState {
    pub plan: R10BleDeviceTaskPlan,
    pub started: bool,
}

impl Default for R10BleDeviceTaskState {
    fn default() -> Self {
        Self::new(R10BleRuntimeConfig::default())
    }
}

impl R10BleDeviceTaskState {
    pub const fn new(config: R10BleRuntimeConfig) -> Self {
        Self {
            plan: R10BleDeviceTaskPlan::from_config(config),
            started: false,
        }
    }

    pub fn configure(&mut self, config: R10BleRuntimeConfig) {
        self.plan = R10BleDeviceTaskPlan::from_config(config);
        self.started = false;
    }

    pub fn start_action(&mut self) -> R10BleDeviceTaskAction {
        if self.started {
            return R10BleDeviceTaskAction::StayDisabled;
        }

        self.started = self.plan.should_start_ble();
        self.plan.action
    }

    pub fn start_request(&mut self) -> Option<R10BleDeviceTaskStartRequest> {
        let action = self.start_action();

        if action.starts_ble() {
            Some(R10BleDeviceTaskStartRequest {
                action,
                config: self.plan.config,
            })
        } else {
            None
        }
    }

    pub fn stop(&mut self) {
        self.started = false;
    }

    pub const fn is_started(&self) -> bool {
        self.started
    }
}

pub const fn r10_ble_device_task_default_config() -> R10BleRuntimeConfig {
    R10BleRuntimeConfig::disabled()
}

pub const fn r10_ble_device_task_probe_config() -> R10BleRuntimeConfig {
    R10BleRuntimeConfig::probe_only_live_r10()
}

pub const fn r10_ble_device_task_reader_remote_config() -> R10BleRuntimeConfig {
    R10BleRuntimeConfig::reader_remote_live_r10()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rustmix_x4::ring_remote::r10_ble_transport::{
        R10_BLE_DEFAULT_NAME_PREFIX, R10_BLE_DEFAULT_TARGET_ADDRESS, R10BleScanTarget,
    };

    #[test]
    fn r10_ble_device_task_default_plan_stays_disabled() {
        let plan = R10BleDeviceTaskPlan::default();

        assert_eq!(plan.mode(), R10BleRuntimeMode::Disabled);
        assert_eq!(plan.action, R10BleDeviceTaskAction::StayDisabled);
        assert!(!plan.should_start_ble());
        assert!(!plan.should_start_probe());
        assert!(!plan.should_start_reader_remote());
    }

    #[test]
    fn r10_ble_device_task_probe_only_plan_starts_probe() {
        let plan = R10BleDeviceTaskPlan::probe_only_live_r10();

        assert_eq!(plan.mode(), R10BleRuntimeMode::ProbeOnly);
        assert_eq!(plan.action, R10BleDeviceTaskAction::StartProbeOnly);
        assert!(plan.should_start_ble());
        assert!(plan.should_start_probe());
        assert!(!plan.should_start_reader_remote());
        assert_eq!(
            plan.config.scan_target.address,
            Some(R10_BLE_DEFAULT_TARGET_ADDRESS)
        );
        assert_eq!(
            plan.config.scan_target.name_prefix,
            Some(R10_BLE_DEFAULT_NAME_PREFIX)
        );
    }

    #[test]
    fn r10_ble_device_task_reader_remote_plan_starts_reader_remote() {
        let plan = R10BleDeviceTaskPlan::reader_remote_live_r10();

        assert_eq!(plan.mode(), R10BleRuntimeMode::ReaderRemote);
        assert_eq!(plan.action, R10BleDeviceTaskAction::StartReaderRemote);
        assert!(plan.should_start_ble());
        assert!(!plan.should_start_probe());
        assert!(plan.should_start_reader_remote());
        assert_eq!(
            plan.reader_debounce_ms(),
            R10_BLE_RUNTIME_DEFAULT_READER_DEBOUNCE_MS
        );
    }

    #[test]
    fn r10_ble_device_task_actions_have_stable_labels() {
        assert_eq!(
            R10BleDeviceTaskAction::StayDisabled.as_str(),
            "stay_disabled"
        );
        assert_eq!(
            R10BleDeviceTaskAction::StartProbeOnly.as_str(),
            "start_probe_only"
        );
        assert_eq!(
            R10BleDeviceTaskAction::StartReaderRemote.as_str(),
            "start_reader_remote"
        );
    }

    #[test]
    fn r10_ble_device_task_state_starts_only_once_until_stopped() {
        let mut state = R10BleDeviceTaskState::new(R10BleRuntimeConfig::probe_only_live_r10());

        assert!(!state.is_started());
        assert_eq!(state.start_action(), R10BleDeviceTaskAction::StartProbeOnly);
        assert!(state.is_started());

        assert_eq!(state.start_action(), R10BleDeviceTaskAction::StayDisabled);
        assert!(state.is_started());

        state.stop();
        assert!(!state.is_started());
        assert_eq!(state.start_action(), R10BleDeviceTaskAction::StartProbeOnly);
    }

    #[test]
    fn r10_ble_device_task_state_disabled_never_marks_started() {
        let mut state = R10BleDeviceTaskState::default();

        assert_eq!(state.start_action(), R10BleDeviceTaskAction::StayDisabled);
        assert!(!state.is_started());
    }

    #[test]
    fn r10_ble_device_task_reconfigure_resets_started_flag() {
        let mut state = R10BleDeviceTaskState::new(R10BleRuntimeConfig::reader_remote_live_r10());

        assert_eq!(
            state.start_action(),
            R10BleDeviceTaskAction::StartReaderRemote
        );
        assert!(state.is_started());

        state.configure(R10BleRuntimeConfig::disabled());

        assert!(!state.is_started());
        assert_eq!(state.plan.action, R10BleDeviceTaskAction::StayDisabled);
        assert_eq!(state.start_action(), R10BleDeviceTaskAction::StayDisabled);
    }

    #[test]
    fn r10_ble_device_task_preserves_custom_target_and_debounce() {
        let target = R10BleScanTarget {
            address: Some("AA:BB:CC:DD:EE:FF"),
            name_prefix: Some("COLMI R10"),
        };
        let config = R10BleRuntimeConfig::reader_remote_live_r10()
            .with_scan_target(target)
            .with_reader_debounce_ms(4_000);
        let plan = R10BleDeviceTaskPlan::from_config(config);

        assert_eq!(plan.config.scan_target, target);
        assert_eq!(plan.reader_debounce_ms(), 4_000);
        assert_eq!(plan.action, R10BleDeviceTaskAction::StartReaderRemote);
    }
    #[test]
    fn r10_ble_device_task_disabled_start_request_is_none() {
        let mut state = R10BleDeviceTaskState::default();

        assert_eq!(state.start_request(), None);
        assert!(!state.is_started());
    }

    #[test]
    fn r10_ble_device_task_probe_only_start_request_is_one_shot() {
        let mut state = R10BleDeviceTaskState::new(R10BleRuntimeConfig::probe_only_live_r10());

        let request = state
            .start_request()
            .expect("ProbeOnly should produce one start request");

        assert_eq!(request.action, R10BleDeviceTaskAction::StartProbeOnly);
        assert!(request.should_start_ble());
        assert!(request.should_run_probe());
        assert!(!request.should_run_reader_remote());
        assert!(state.is_started());

        assert_eq!(state.start_request(), None);
        assert!(state.is_started());
    }

    #[test]
    fn r10_ble_device_task_reader_remote_start_request_preserves_debounce() {
        let mut state = R10BleDeviceTaskState::new(R10BleRuntimeConfig::reader_remote_live_r10());

        let request = state
            .start_request()
            .expect("ReaderRemote should produce one start request");

        assert_eq!(request.action, R10BleDeviceTaskAction::StartReaderRemote);
        assert!(request.should_start_ble());
        assert!(!request.should_run_probe());
        assert!(request.should_run_reader_remote());
        assert_eq!(
            request.reader_debounce_ms(),
            R10_BLE_RUNTIME_DEFAULT_READER_DEBOUNCE_MS
        );
    }
    #[test]
    fn r10_ble_device_task_runner_from_disabled_state_is_none() {
        let mut state = R10BleDeviceTaskState::default();

        assert_eq!(
            R10BleDeviceTaskRunnerPlan::from_state_start(&mut state),
            None
        );
        assert!(!state.is_started());
    }

    #[test]
    fn r10_ble_device_task_probe_runner_uses_probe_lifecycle() {
        let request = R10BleDeviceTaskStartRequest {
            action: R10BleDeviceTaskAction::StartProbeOnly,
            config: R10BleRuntimeConfig::probe_only_live_r10(),
        };

        let plan = R10BleDeviceTaskRunnerPlan::from_start_request(request)
            .expect("ProbeOnly should produce a runner plan");

        assert!(plan.should_run_probe());
        assert!(!plan.should_run_reader_remote());
        assert!(!plan.should_emit_reader_events());
        assert_eq!(plan.lifecycle_len(), 9);
        assert_eq!(plan.lifecycle_label_at(0), Some("start_scan"));
        assert_eq!(plan.lifecycle_label_at(7), Some("notify"));
        assert_eq!(plan.lifecycle_label_at(8), Some("timeout"));
    }

    #[test]
    fn r10_ble_device_task_reader_remote_runner_enables_reader_events() {
        let request = R10BleDeviceTaskStartRequest {
            action: R10BleDeviceTaskAction::StartReaderRemote,
            config: R10BleRuntimeConfig::reader_remote_live_r10(),
        };

        let plan = R10BleDeviceTaskRunnerPlan::from_start_request(request)
            .expect("ReaderRemote should produce a runner plan");

        assert!(!plan.should_run_probe());
        assert!(plan.should_run_reader_remote());
        assert!(plan.should_emit_reader_events());
        assert_eq!(
            plan.request.reader_debounce_ms(),
            R10_BLE_RUNTIME_DEFAULT_READER_DEBOUNCE_MS
        );
    }

    #[test]
    fn r10_ble_device_task_runner_rejects_disabled_request() {
        let request = R10BleDeviceTaskStartRequest {
            action: R10BleDeviceTaskAction::StayDisabled,
            config: R10BleRuntimeConfig::disabled(),
        };

        assert_eq!(
            R10BleDeviceTaskRunnerPlan::from_start_request(request),
            None
        );
    }

    #[test]
    fn r10_ble_device_task_runner_from_state_start_is_one_shot() {
        let mut state = R10BleDeviceTaskState::new(R10BleRuntimeConfig::probe_only_live_r10());

        let plan = R10BleDeviceTaskRunnerPlan::from_state_start(&mut state)
            .expect("first ProbeOnly state start should produce runner plan");

        assert_eq!(plan.request.action, R10BleDeviceTaskAction::StartProbeOnly);
        assert!(state.is_started());

        assert_eq!(
            R10BleDeviceTaskRunnerPlan::from_state_start(&mut state),
            None
        );
        assert!(state.is_started());
    }

    #[test]
    fn r10_ble_device_task_runner_lifecycle_order_matches_probe_script() {
        let request = R10BleDeviceTaskStartRequest {
            action: R10BleDeviceTaskAction::StartProbeOnly,
            config: R10BleRuntimeConfig::probe_only_live_r10(),
        };
        let plan = R10BleDeviceTaskRunnerPlan::from_start_request(request).unwrap();

        let expected = [
            "start_scan",
            "target_seen",
            "connect",
            "discover",
            "subscribe",
            "remote_start",
            "poll",
            "notify",
            "timeout",
        ];

        for (index, label) in expected.iter().enumerate() {
            assert_eq!(plan.lifecycle_label_at(index), Some(*label));
        }
        assert_eq!(plan.lifecycle_label_at(expected.len()), None);
    }

    #[test]
    fn r10_ble_device_task_lifecycle_labels_are_monitor_safe() {
        for event in R10_BLE_DEVICE_TASK_PROBE_LIFECYCLE {
            let label = event.as_str();

            assert!(!label.is_empty());
            assert!(
                label
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte == b'_')
            );
        }
    }

    #[test]
    fn r10_ble_device_task_lifecycle_terminal_flags_are_explicit() {
        assert!(!R10BleDeviceTaskLifecycleEvent::StartScan.is_terminal());
        assert!(!R10BleDeviceTaskLifecycleEvent::Poll.is_terminal());
        assert!(R10BleDeviceTaskLifecycleEvent::Notify.is_terminal());
        assert!(R10BleDeviceTaskLifecycleEvent::Timeout.is_terminal());
    }
}
