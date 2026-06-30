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
pub struct R10BleDeviceTaskMonitorEvent {
    pub action: R10BleDeviceTaskAction,
    pub lifecycle: R10BleDeviceTaskLifecycleEvent,
    pub index: u8,
    pub reader_events_enabled: bool,
}

impl R10BleDeviceTaskMonitorEvent {
    pub const fn prefix_label(&self) -> &'static str {
        "r10_ble_task"
    }

    pub const fn action_label(&self) -> &'static str {
        self.action.as_str()
    }

    pub const fn lifecycle_label(&self) -> &'static str {
        self.lifecycle.as_str()
    }

    pub const fn reader_label(&self) -> &'static str {
        if self.reader_events_enabled {
            "reader_on"
        } else {
            "reader_off"
        }
    }

    pub const fn terminal_label(&self) -> &'static str {
        if self.lifecycle.is_terminal() {
            "terminal"
        } else {
            "running"
        }
    }

    pub fn is_monitor_safe(&self) -> bool {
        r10_ble_device_task_monitor_prefix_is_safe(self.prefix_label())
            && r10_ble_device_task_monitor_label_is_safe(self.action_label())
            && r10_ble_device_task_monitor_label_is_safe(self.lifecycle_label())
            && r10_ble_device_task_monitor_label_is_safe(self.reader_label())
            && r10_ble_device_task_monitor_label_is_safe(self.terminal_label())
    }
}

pub fn r10_ble_device_task_monitor_prefix_is_safe(label: &str) -> bool {
    label == "r10_ble_task"
}

pub fn r10_ble_device_task_monitor_label_is_safe(label: &str) -> bool {
    !label.is_empty()
        && label
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte == b'_')
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct R10BleDeviceTaskMonitorSink<const N: usize> {
    events: [Option<R10BleDeviceTaskMonitorEvent>; N],
    len: usize,
    dropped: usize,
}

impl<const N: usize> Default for R10BleDeviceTaskMonitorSink<N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize> R10BleDeviceTaskMonitorSink<N> {
    pub fn new() -> Self {
        Self {
            events: [None; N],
            len: 0,
            dropped: 0,
        }
    }

    pub fn push(&mut self, event: R10BleDeviceTaskMonitorEvent) -> bool {
        if self.len < N {
            self.events[self.len] = Some(event);
            self.len += 1;
            true
        } else {
            self.dropped += 1;
            false
        }
    }

    pub fn record_plan(&mut self, plan: &R10BleDeviceTaskRunnerPlan) -> usize {
        plan.emit_monitor_events(self)
    }

    pub const fn len(&self) -> usize {
        self.len
    }

    pub const fn dropped(&self) -> usize {
        self.dropped
    }

    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn event_at(&self, index: usize) -> Option<R10BleDeviceTaskMonitorEvent> {
        if index < self.len {
            self.events[index]
        } else {
            None
        }
    }

    pub fn last(&self) -> Option<R10BleDeviceTaskMonitorEvent> {
        if self.len == 0 {
            None
        } else {
            self.event_at(self.len - 1)
        }
    }

    pub fn clear(&mut self) {
        let mut index = 0;

        while index < self.len {
            self.events[index] = None;
            index += 1;
        }

        self.len = 0;
        self.dropped = 0;
    }
}

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

    pub fn monitor_event_at(&self, index: usize) -> Option<R10BleDeviceTaskMonitorEvent> {
        self.lifecycle
            .get(index)
            .map(|event| R10BleDeviceTaskMonitorEvent {
                action: self.request.action,
                lifecycle: *event,
                index: index as u8,
                reader_events_enabled: self.reader_events_enabled,
            })
    }

    pub fn emit_monitor_events<const N: usize>(
        &self,
        sink: &mut R10BleDeviceTaskMonitorSink<N>,
    ) -> usize {
        let mut emitted = 0;
        let mut index = 0;

        while index < self.lifecycle.len() {
            if let Some(event) = self.monitor_event_at(index) {
                if sink.push(event) {
                    emitted += 1;
                }
            }

            index += 1;
        }

        emitted
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R10BleDeviceTaskHardwareCommand {
    ScanStart,
    ConnectTarget,
    DiscoverGatt,
    SubscribeCccd,
    WriteRemoteStart,
    WritePoll,
    HandleNotify,
}

impl R10BleDeviceTaskHardwareCommand {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::ScanStart => "scan_start",
            Self::ConnectTarget => "connect_target",
            Self::DiscoverGatt => "discover_gatt",
            Self::SubscribeCccd => "subscribe_cccd",
            Self::WriteRemoteStart => "write_remote_start",
            Self::WritePoll => "write_poll",
            Self::HandleNotify => "handle_notify",
        }
    }

    pub const fn is_write(&self) -> bool {
        matches!(
            self,
            Self::SubscribeCccd | Self::WriteRemoteStart | Self::WritePoll
        )
    }

    pub const fn is_notify_handler(&self) -> bool {
        matches!(self, Self::HandleNotify)
    }

    pub const fn from_lifecycle(lifecycle: R10BleDeviceTaskLifecycleEvent) -> Option<Self> {
        match lifecycle {
            R10BleDeviceTaskLifecycleEvent::StartScan => Some(Self::ScanStart),
            R10BleDeviceTaskLifecycleEvent::TargetSeen => Some(Self::ConnectTarget),
            R10BleDeviceTaskLifecycleEvent::Connect => Some(Self::DiscoverGatt),
            R10BleDeviceTaskLifecycleEvent::Discover => Some(Self::SubscribeCccd),
            R10BleDeviceTaskLifecycleEvent::Subscribe => Some(Self::WriteRemoteStart),
            R10BleDeviceTaskLifecycleEvent::RemoteStart => Some(Self::WritePoll),
            R10BleDeviceTaskLifecycleEvent::Poll => Some(Self::WritePoll),
            R10BleDeviceTaskLifecycleEvent::Notify => Some(Self::HandleNotify),
            R10BleDeviceTaskLifecycleEvent::Timeout => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R10BleDeviceTaskHardwareCommandStep {
    pub lifecycle: R10BleDeviceTaskLifecycleEvent,
    pub command: R10BleDeviceTaskHardwareCommand,
    pub index: u8,
    pub reader_events_enabled: bool,
}

impl R10BleDeviceTaskHardwareCommandStep {
    pub const fn lifecycle_label(&self) -> &'static str {
        self.lifecycle.as_str()
    }

    pub const fn command_label(&self) -> &'static str {
        self.command.as_str()
    }

    pub const fn reader_label(&self) -> &'static str {
        if self.reader_events_enabled {
            "reader_on"
        } else {
            "reader_off"
        }
    }

    pub const fn should_emit_reader_event(&self) -> bool {
        self.reader_events_enabled && self.command.is_notify_handler()
    }

    pub const fn is_probe_log_only(&self) -> bool {
        !self.reader_events_enabled
    }

    pub fn is_monitor_safe(&self) -> bool {
        r10_ble_device_task_monitor_label_is_safe(self.lifecycle_label())
            && r10_ble_device_task_monitor_label_is_safe(self.command_label())
            && r10_ble_device_task_monitor_label_is_safe(self.reader_label())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct R10BleDeviceTaskHardwareCommandSink<const N: usize> {
    steps: [Option<R10BleDeviceTaskHardwareCommandStep>; N],
    len: usize,
    dropped: usize,
}

impl<const N: usize> Default for R10BleDeviceTaskHardwareCommandSink<N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize> R10BleDeviceTaskHardwareCommandSink<N> {
    pub fn new() -> Self {
        Self {
            steps: [None; N],
            len: 0,
            dropped: 0,
        }
    }

    pub fn push(&mut self, step: R10BleDeviceTaskHardwareCommandStep) -> bool {
        if self.len < N {
            self.steps[self.len] = Some(step);
            self.len += 1;
            true
        } else {
            self.dropped += 1;
            false
        }
    }

    pub fn record_plan(&mut self, plan: &R10BleDeviceTaskRunnerPlan) -> usize {
        plan.emit_hardware_commands(self)
    }

    pub const fn len(&self) -> usize {
        self.len
    }

    pub const fn dropped(&self) -> usize {
        self.dropped
    }

    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn step_at(&self, index: usize) -> Option<R10BleDeviceTaskHardwareCommandStep> {
        if index < self.len {
            self.steps[index]
        } else {
            None
        }
    }

    pub fn last(&self) -> Option<R10BleDeviceTaskHardwareCommandStep> {
        if self.len == 0 {
            None
        } else {
            self.step_at(self.len - 1)
        }
    }

    pub fn clear(&mut self) {
        let mut index = 0;

        while index < self.len {
            self.steps[index] = None;
            index += 1;
        }

        self.len = 0;
        self.dropped = 0;
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R10BleDeviceTaskHardwareCommandPlan {
    pub runner: R10BleDeviceTaskRunnerPlan,
}

impl R10BleDeviceTaskHardwareCommandPlan {
    pub const fn from_runner_plan(runner: R10BleDeviceTaskRunnerPlan) -> Self {
        Self { runner }
    }

    pub fn from_state_start(state: &mut R10BleDeviceTaskState) -> Option<Self> {
        R10BleDeviceTaskRunnerPlan::from_state_start(state).map(Self::from_runner_plan)
    }

    pub fn command_at(&self, index: usize) -> Option<R10BleDeviceTaskHardwareCommandStep> {
        self.runner.lifecycle.get(index).and_then(|lifecycle| {
            R10BleDeviceTaskHardwareCommand::from_lifecycle(*lifecycle).map(|command| {
                R10BleDeviceTaskHardwareCommandStep {
                    lifecycle: *lifecycle,
                    command,
                    index: index as u8,
                    reader_events_enabled: self.runner.reader_events_enabled,
                }
            })
        })
    }

    pub fn command_label_at(&self, index: usize) -> Option<&'static str> {
        self.command_at(index).map(|step| step.command.as_str())
    }

    pub fn emit_commands<const N: usize>(
        &self,
        sink: &mut R10BleDeviceTaskHardwareCommandSink<N>,
    ) -> usize {
        let mut emitted = 0;
        let mut index = 0;

        while index < self.runner.lifecycle.len() {
            if let Some(step) = self.command_at(index) {
                if sink.push(step) {
                    emitted += 1;
                }
            }

            index += 1;
        }

        emitted
    }
}

impl R10BleDeviceTaskRunnerPlan {
    pub fn hardware_command_at(&self, index: usize) -> Option<R10BleDeviceTaskHardwareCommandStep> {
        R10BleDeviceTaskHardwareCommandPlan::from_runner_plan(*self).command_at(index)
    }

    pub fn hardware_command_label_at(&self, index: usize) -> Option<&'static str> {
        R10BleDeviceTaskHardwareCommandPlan::from_runner_plan(*self).command_label_at(index)
    }

    pub fn emit_hardware_commands<const N: usize>(
        &self,
        sink: &mut R10BleDeviceTaskHardwareCommandSink<N>,
    ) -> usize {
        R10BleDeviceTaskHardwareCommandPlan::from_runner_plan(*self).emit_commands(sink)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R10BleDeviceTaskHardwareOutcome {
    Pending,
    Started,
    Ok,
    Ignored,
    Failed,
    Timeout,
}

impl R10BleDeviceTaskHardwareOutcome {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Started => "started",
            Self::Ok => "ok",
            Self::Ignored => "ignored",
            Self::Failed => "failed",
            Self::Timeout => "timeout",
        }
    }

    pub const fn is_terminal(&self) -> bool {
        matches!(
            self,
            Self::Ok | Self::Ignored | Self::Failed | Self::Timeout
        )
    }

    pub const fn is_success(&self) -> bool {
        matches!(self, Self::Ok)
    }

    pub const fn should_retry(&self) -> bool {
        matches!(self, Self::Failed | Self::Timeout)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R10BleDeviceTaskHardwareTranscriptEntry {
    pub step: R10BleDeviceTaskHardwareCommandStep,
    pub outcome: R10BleDeviceTaskHardwareOutcome,
    pub sequence: u8,
}

impl R10BleDeviceTaskHardwareTranscriptEntry {
    pub const fn prefix_label(&self) -> &'static str {
        "ble_hw_command"
    }

    pub const fn lifecycle_label(&self) -> &'static str {
        self.step.lifecycle_label()
    }

    pub const fn command_label(&self) -> &'static str {
        self.step.command_label()
    }

    pub const fn outcome_label(&self) -> &'static str {
        self.outcome.as_str()
    }

    pub const fn reader_label(&self) -> &'static str {
        self.step.reader_label()
    }

    pub const fn is_terminal(&self) -> bool {
        self.outcome.is_terminal()
    }

    pub const fn should_retry(&self) -> bool {
        self.outcome.should_retry()
    }

    pub const fn should_emit_reader_event(&self) -> bool {
        self.outcome.is_success() && self.step.should_emit_reader_event()
    }

    pub const fn is_probe_log_only(&self) -> bool {
        self.step.is_probe_log_only()
    }

    pub fn is_monitor_safe(&self) -> bool {
        r10_ble_device_task_monitor_label_is_safe(self.prefix_label())
            && r10_ble_device_task_monitor_label_is_safe(self.lifecycle_label())
            && r10_ble_device_task_monitor_label_is_safe(self.command_label())
            && r10_ble_device_task_monitor_label_is_safe(self.outcome_label())
            && r10_ble_device_task_monitor_label_is_safe(self.reader_label())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct R10BleDeviceTaskHardwareTranscript<const N: usize> {
    entries: [Option<R10BleDeviceTaskHardwareTranscriptEntry>; N],
    len: usize,
    dropped: usize,
}

impl<const N: usize> Default for R10BleDeviceTaskHardwareTranscript<N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize> R10BleDeviceTaskHardwareTranscript<N> {
    pub fn new() -> Self {
        Self {
            entries: [None; N],
            len: 0,
            dropped: 0,
        }
    }

    pub fn push(&mut self, entry: R10BleDeviceTaskHardwareTranscriptEntry) -> bool {
        if self.len < N {
            self.entries[self.len] = Some(entry);
            self.len += 1;
            true
        } else {
            self.dropped += 1;
            false
        }
    }

    pub fn record_plan_with_outcome(
        &mut self,
        plan: &R10BleDeviceTaskHardwareCommandPlan,
        outcome: R10BleDeviceTaskHardwareOutcome,
    ) -> usize {
        plan.emit_transcript(self, outcome)
    }

    pub fn record_runner_with_outcome(
        &mut self,
        runner: &R10BleDeviceTaskRunnerPlan,
        outcome: R10BleDeviceTaskHardwareOutcome,
    ) -> usize {
        R10BleDeviceTaskHardwareCommandPlan::from_runner_plan(*runner)
            .emit_transcript(self, outcome)
    }

    pub const fn len(&self) -> usize {
        self.len
    }

    pub const fn dropped(&self) -> usize {
        self.dropped
    }

    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn entry_at(&self, index: usize) -> Option<R10BleDeviceTaskHardwareTranscriptEntry> {
        if index < self.len {
            self.entries[index]
        } else {
            None
        }
    }

    pub fn last(&self) -> Option<R10BleDeviceTaskHardwareTranscriptEntry> {
        if self.len == 0 {
            None
        } else {
            self.entry_at(self.len - 1)
        }
    }

    pub fn clear(&mut self) {
        let mut index = 0;

        while index < self.len {
            self.entries[index] = None;
            index += 1;
        }

        self.len = 0;
        self.dropped = 0;
    }
}

impl R10BleDeviceTaskHardwareCommandPlan {
    pub fn transcript_entry_at(
        &self,
        index: usize,
        outcome: R10BleDeviceTaskHardwareOutcome,
    ) -> Option<R10BleDeviceTaskHardwareTranscriptEntry> {
        self.command_at(index)
            .map(|step| R10BleDeviceTaskHardwareTranscriptEntry {
                step,
                outcome,
                sequence: index as u8,
            })
    }

    pub fn emit_transcript<const N: usize>(
        &self,
        transcript: &mut R10BleDeviceTaskHardwareTranscript<N>,
        outcome: R10BleDeviceTaskHardwareOutcome,
    ) -> usize {
        let mut emitted = 0;
        let mut index = 0;

        while index < self.runner.lifecycle.len() {
            if let Some(entry) = self.transcript_entry_at(index, outcome) {
                if transcript.push(entry) {
                    emitted += 1;
                }
            }

            index += 1;
        }

        emitted
    }
}

impl R10BleDeviceTaskRunnerPlan {
    pub fn hardware_transcript_entry_at(
        &self,
        index: usize,
        outcome: R10BleDeviceTaskHardwareOutcome,
    ) -> Option<R10BleDeviceTaskHardwareTranscriptEntry> {
        R10BleDeviceTaskHardwareCommandPlan::from_runner_plan(*self)
            .transcript_entry_at(index, outcome)
    }

    pub fn emit_hardware_transcript<const N: usize>(
        &self,
        transcript: &mut R10BleDeviceTaskHardwareTranscript<N>,
        outcome: R10BleDeviceTaskHardwareOutcome,
    ) -> usize {
        R10BleDeviceTaskHardwareCommandPlan::from_runner_plan(*self)
            .emit_transcript(transcript, outcome)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R10BleDeviceTaskHardwareMockOutcomeScript {
    Success,
    TimeoutNotify,
    FailedWrite,
    IgnoredNotify,
}

impl R10BleDeviceTaskHardwareMockOutcomeScript {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::TimeoutNotify => "timeout_notify",
            Self::FailedWrite => "failed_write",
            Self::IgnoredNotify => "ignored_notify",
        }
    }

    pub fn outcome_for_step(
        &self,
        step: &R10BleDeviceTaskHardwareCommandStep,
    ) -> R10BleDeviceTaskHardwareOutcome {
        match self {
            Self::Success => R10BleDeviceTaskHardwareOutcome::Ok,
            Self::TimeoutNotify => {
                if step.command.is_notify_handler() {
                    R10BleDeviceTaskHardwareOutcome::Timeout
                } else {
                    R10BleDeviceTaskHardwareOutcome::Ok
                }
            }
            Self::FailedWrite => {
                if step.command.is_write() {
                    R10BleDeviceTaskHardwareOutcome::Failed
                } else if step.command.is_notify_handler() {
                    R10BleDeviceTaskHardwareOutcome::Ignored
                } else {
                    R10BleDeviceTaskHardwareOutcome::Ok
                }
            }
            Self::IgnoredNotify => {
                if step.command.is_notify_handler() {
                    R10BleDeviceTaskHardwareOutcome::Ignored
                } else {
                    R10BleDeviceTaskHardwareOutcome::Ok
                }
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R10BleDeviceTaskHardwareMockExecutionReport {
    pub attempted: usize,
    pub recorded: usize,
    pub dropped: usize,
    pub reader_events_emitted: usize,
    pub retry_requested: bool,
    pub last_outcome: Option<R10BleDeviceTaskHardwareOutcome>,
}

impl R10BleDeviceTaskHardwareMockExecutionReport {
    pub const fn empty() -> Self {
        Self {
            attempted: 0,
            recorded: 0,
            dropped: 0,
            reader_events_emitted: 0,
            retry_requested: false,
            last_outcome: None,
        }
    }

    pub const fn completed(&self) -> bool {
        self.attempted > 0 && self.attempted == self.recorded && self.dropped == 0
    }

    pub const fn emitted_reader_events(&self) -> bool {
        self.reader_events_emitted > 0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R10BleDeviceTaskHardwareMockExecutor {
    pub script: R10BleDeviceTaskHardwareMockOutcomeScript,
}

impl R10BleDeviceTaskHardwareMockExecutor {
    pub const fn new(script: R10BleDeviceTaskHardwareMockOutcomeScript) -> Self {
        Self { script }
    }

    pub const fn success() -> Self {
        Self::new(R10BleDeviceTaskHardwareMockOutcomeScript::Success)
    }

    pub const fn timeout_notify() -> Self {
        Self::new(R10BleDeviceTaskHardwareMockOutcomeScript::TimeoutNotify)
    }

    pub const fn failed_write() -> Self {
        Self::new(R10BleDeviceTaskHardwareMockOutcomeScript::FailedWrite)
    }

    pub const fn ignored_notify() -> Self {
        Self::new(R10BleDeviceTaskHardwareMockOutcomeScript::IgnoredNotify)
    }

    pub fn execute_plan<const N: usize>(
        &self,
        plan: &R10BleDeviceTaskHardwareCommandPlan,
        transcript: &mut R10BleDeviceTaskHardwareTranscript<N>,
    ) -> R10BleDeviceTaskHardwareMockExecutionReport {
        let mut report = R10BleDeviceTaskHardwareMockExecutionReport::empty();
        let mut index = 0;

        while index < plan.runner.lifecycle.len() {
            if let Some(step) = plan.command_at(index) {
                let outcome = self.script.outcome_for_step(&step);
                let entry = R10BleDeviceTaskHardwareTranscriptEntry {
                    step,
                    outcome,
                    sequence: index as u8,
                };

                report.attempted += 1;
                report.last_outcome = Some(outcome);

                if entry.should_emit_reader_event() {
                    report.reader_events_emitted += 1;
                }

                if entry.should_retry() {
                    report.retry_requested = true;
                }

                if transcript.push(entry) {
                    report.recorded += 1;
                }
            }

            index += 1;
        }

        report.dropped = transcript.dropped();
        report
    }

    pub fn execute_runner<const N: usize>(
        &self,
        runner: &R10BleDeviceTaskRunnerPlan,
        transcript: &mut R10BleDeviceTaskHardwareTranscript<N>,
    ) -> R10BleDeviceTaskHardwareMockExecutionReport {
        let plan = R10BleDeviceTaskHardwareCommandPlan::from_runner_plan(*runner);
        self.execute_plan(&plan, transcript)
    }

    pub fn execute_state_start<const N: usize>(
        &self,
        state: &mut R10BleDeviceTaskState,
        transcript: &mut R10BleDeviceTaskHardwareTranscript<N>,
    ) -> Option<R10BleDeviceTaskHardwareMockExecutionReport> {
        R10BleDeviceTaskHardwareCommandPlan::from_state_start(state)
            .map(|plan| self.execute_plan(&plan, transcript))
    }
}

pub const R10_BLE_X4_DEPLOY_FEATURE: &str = "r10-ble-host";
pub const R10_BLE_X4_DEPLOY_SCRIPT: &str = "scripts/x4_r10_ble_probe_deploy.sh";
pub const R10_BLE_X4_DEPLOY_CHIP: &str = "esp32c3";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R10BleDeviceTaskX4DeployMode {
    Disabled,
    ProbeOnly,
    ReaderRemote,
}

impl R10BleDeviceTaskX4DeployMode {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::ProbeOnly => "probe_only",
            Self::ReaderRemote => "reader_remote",
        }
    }

    pub const fn should_flash(&self) -> bool {
        !matches!(self, Self::Disabled)
    }

    pub const fn reader_events_enabled(&self) -> bool {
        matches!(self, Self::ReaderRemote)
    }

    pub fn runtime_config(&self) -> R10BleRuntimeConfig {
        match self {
            Self::Disabled => R10BleRuntimeConfig::disabled(),
            Self::ProbeOnly => R10BleRuntimeConfig::probe_only_live_r10(),
            Self::ReaderRemote => R10BleRuntimeConfig::reader_remote_live_r10(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R10BleDeviceTaskX4DeployProfile {
    pub mode: R10BleDeviceTaskX4DeployMode,
}

impl R10BleDeviceTaskX4DeployProfile {
    pub const fn disabled() -> Self {
        Self {
            mode: R10BleDeviceTaskX4DeployMode::Disabled,
        }
    }

    pub const fn probe_only_live_r10() -> Self {
        Self {
            mode: R10BleDeviceTaskX4DeployMode::ProbeOnly,
        }
    }

    pub const fn reader_remote_live_r10() -> Self {
        Self {
            mode: R10BleDeviceTaskX4DeployMode::ReaderRemote,
        }
    }

    pub fn from_mode_label(label: &str) -> Option<Self> {
        match label {
            "disabled" => Some(Self::disabled()),
            "probe_only" | "probe-only" | "probe" => Some(Self::probe_only_live_r10()),
            "reader_remote" | "reader-remote" | "reader" => Some(Self::reader_remote_live_r10()),
            _ => None,
        }
    }

    pub const fn mode_label(&self) -> &'static str {
        self.mode.as_str()
    }

    pub const fn feature_label(&self) -> &'static str {
        R10_BLE_X4_DEPLOY_FEATURE
    }

    pub const fn deploy_script(&self) -> &'static str {
        R10_BLE_X4_DEPLOY_SCRIPT
    }

    pub const fn chip_label(&self) -> &'static str {
        R10_BLE_X4_DEPLOY_CHIP
    }

    pub const fn should_flash(&self) -> bool {
        self.mode.should_flash()
    }

    pub const fn reader_events_enabled(&self) -> bool {
        self.mode.reader_events_enabled()
    }

    pub fn runtime_config(&self) -> R10BleRuntimeConfig {
        self.mode.runtime_config()
    }

    pub fn start_request(&self) -> Option<R10BleDeviceTaskStartRequest> {
        match self.mode {
            R10BleDeviceTaskX4DeployMode::Disabled => None,
            R10BleDeviceTaskX4DeployMode::ProbeOnly => Some(R10BleDeviceTaskStartRequest {
                action: R10BleDeviceTaskAction::StartProbeOnly,
                config: self.runtime_config(),
            }),
            R10BleDeviceTaskX4DeployMode::ReaderRemote => Some(R10BleDeviceTaskStartRequest {
                action: R10BleDeviceTaskAction::StartReaderRemote,
                config: self.runtime_config(),
            }),
        }
    }

    pub fn runner_plan(&self) -> Option<R10BleDeviceTaskRunnerPlan> {
        self.start_request()
            .and_then(R10BleDeviceTaskRunnerPlan::from_start_request)
    }

    pub fn hardware_command_plan(&self) -> Option<R10BleDeviceTaskHardwareCommandPlan> {
        self.runner_plan()
            .map(R10BleDeviceTaskHardwareCommandPlan::from_runner_plan)
    }

    pub fn hardware_command_count(&self) -> usize {
        let Some(plan) = self.hardware_command_plan() else {
            return 0;
        };

        let mut count = 0;
        let mut index = 0;

        while index < plan.runner.lifecycle_len() {
            if plan.command_at(index).is_some() {
                count += 1;
            }

            index += 1;
        }

        count
    }

    pub fn dry_run_success_report<const N: usize>(
        &self,
        transcript: &mut R10BleDeviceTaskHardwareTranscript<N>,
    ) -> Option<R10BleDeviceTaskHardwareMockExecutionReport> {
        let runner = self.runner_plan()?;

        Some(R10BleDeviceTaskHardwareMockExecutor::success().execute_runner(&runner, transcript))
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
    #[test]
    fn r10_ble_device_task_monitor_event_fields_are_compact() {
        let request = R10BleDeviceTaskStartRequest {
            action: R10BleDeviceTaskAction::StartProbeOnly,
            config: R10BleRuntimeConfig::probe_only_live_r10(),
        };
        let plan = R10BleDeviceTaskRunnerPlan::from_start_request(request).unwrap();
        let event = plan.monitor_event_at(0).unwrap();

        assert_eq!(event.prefix_label(), "r10_ble_task");
        assert_eq!(event.action_label(), "start_probe_only");
        assert_eq!(event.lifecycle_label(), "start_scan");
        assert_eq!(event.reader_label(), "reader_off");
        assert_eq!(event.terminal_label(), "running");
        assert_eq!(event.index, 0);
        assert!(event.is_monitor_safe());
    }

    #[test]
    fn r10_ble_device_task_monitor_event_marks_terminal_events() {
        let request = R10BleDeviceTaskStartRequest {
            action: R10BleDeviceTaskAction::StartProbeOnly,
            config: R10BleRuntimeConfig::probe_only_live_r10(),
        };
        let plan = R10BleDeviceTaskRunnerPlan::from_start_request(request).unwrap();

        assert_eq!(
            plan.monitor_event_at(7).unwrap().terminal_label(),
            "terminal"
        );
        assert_eq!(
            plan.monitor_event_at(8).unwrap().terminal_label(),
            "terminal"
        );
    }

    #[test]
    fn r10_ble_device_task_monitor_sink_records_probe_lifecycle() {
        let request = R10BleDeviceTaskStartRequest {
            action: R10BleDeviceTaskAction::StartProbeOnly,
            config: R10BleRuntimeConfig::probe_only_live_r10(),
        };
        let plan = R10BleDeviceTaskRunnerPlan::from_start_request(request).unwrap();
        let mut sink = R10BleDeviceTaskMonitorSink::<9>::new();

        assert_eq!(sink.record_plan(&plan), 9);
        assert_eq!(sink.len(), 9);
        assert_eq!(sink.dropped(), 0);
        assert_eq!(sink.event_at(0).unwrap().lifecycle_label(), "start_scan");
        assert_eq!(sink.event_at(5).unwrap().lifecycle_label(), "remote_start");
        assert_eq!(sink.event_at(8).unwrap().lifecycle_label(), "timeout");
        assert_eq!(sink.last().unwrap().terminal_label(), "terminal");
    }

    #[test]
    fn r10_ble_device_task_monitor_sink_tracks_overflow() {
        let request = R10BleDeviceTaskStartRequest {
            action: R10BleDeviceTaskAction::StartProbeOnly,
            config: R10BleRuntimeConfig::probe_only_live_r10(),
        };
        let plan = R10BleDeviceTaskRunnerPlan::from_start_request(request).unwrap();
        let mut sink = R10BleDeviceTaskMonitorSink::<3>::new();

        assert_eq!(sink.record_plan(&plan), 3);
        assert_eq!(sink.len(), 3);
        assert_eq!(sink.dropped(), 6);
        assert_eq!(sink.event_at(0).unwrap().lifecycle_label(), "start_scan");
        assert_eq!(sink.event_at(2).unwrap().lifecycle_label(), "connect");
        assert_eq!(sink.event_at(3), None);
    }

    #[test]
    fn r10_ble_device_task_monitor_sink_clear_resets_state() {
        let request = R10BleDeviceTaskStartRequest {
            action: R10BleDeviceTaskAction::StartProbeOnly,
            config: R10BleRuntimeConfig::probe_only_live_r10(),
        };
        let plan = R10BleDeviceTaskRunnerPlan::from_start_request(request).unwrap();
        let mut sink = R10BleDeviceTaskMonitorSink::<4>::new();

        sink.record_plan(&plan);
        assert!(!sink.is_empty());
        assert!(sink.dropped() > 0);

        sink.clear();

        assert!(sink.is_empty());
        assert_eq!(sink.len(), 0);
        assert_eq!(sink.dropped(), 0);
        assert_eq!(sink.event_at(0), None);
    }

    #[test]
    fn r10_ble_device_task_monitor_reader_remote_emits_reader_on() {
        let request = R10BleDeviceTaskStartRequest {
            action: R10BleDeviceTaskAction::StartReaderRemote,
            config: R10BleRuntimeConfig::reader_remote_live_r10(),
        };
        let plan = R10BleDeviceTaskRunnerPlan::from_start_request(request).unwrap();
        let mut sink = R10BleDeviceTaskMonitorSink::<9>::new();

        assert_eq!(sink.record_plan(&plan), 9);

        for index in 0..sink.len() {
            let event = sink.event_at(index).unwrap();

            assert_eq!(event.reader_label(), "reader_on");
            assert_eq!(event.action_label(), "start_reader_remote");
            assert!(event.is_monitor_safe());
        }
    }

    #[test]
    fn r10_ble_device_task_monitor_disabled_state_emits_nothing() {
        let mut state = R10BleDeviceTaskState::default();
        let sink = R10BleDeviceTaskMonitorSink::<9>::new();

        let plan = R10BleDeviceTaskRunnerPlan::from_state_start(&mut state);

        assert_eq!(plan, None);
        assert_eq!(sink.len(), 0);
        assert_eq!(sink.dropped(), 0);
        assert!(!state.is_started());
    }

    #[test]
    fn r10_ble_device_task_monitor_labels_reject_unsafe_text() {
        assert!(r10_ble_device_task_monitor_label_is_safe("start_scan"));
        assert!(r10_ble_device_task_monitor_label_is_safe("reader_off"));

        assert!(!r10_ble_device_task_monitor_label_is_safe(""));
        assert!(!r10_ble_device_task_monitor_label_is_safe("StartScan"));
        assert!(!r10_ble_device_task_monitor_label_is_safe("start-scan"));
        assert!(!r10_ble_device_task_monitor_label_is_safe("start scan"));
        assert!(!r10_ble_device_task_monitor_label_is_safe("scan1"));
    }
    #[test]
    fn r10_ble_device_task_monitor_prefix_allows_fixed_r10_label_only() {
        assert!(r10_ble_device_task_monitor_prefix_is_safe("r10_ble_task"));

        assert!(!r10_ble_device_task_monitor_prefix_is_safe(""));
        assert!(!r10_ble_device_task_monitor_prefix_is_safe("r11_ble_task"));
        assert!(!r10_ble_device_task_monitor_prefix_is_safe("r10-ble-task"));
        assert!(!r10_ble_device_task_monitor_prefix_is_safe(
            "r10_ble_task_extra"
        ));
    }
    #[test]
    fn r10_ble_device_task_hardware_command_labels_are_stable_and_safe() {
        let labels = [
            R10BleDeviceTaskHardwareCommand::ScanStart.as_str(),
            R10BleDeviceTaskHardwareCommand::ConnectTarget.as_str(),
            R10BleDeviceTaskHardwareCommand::DiscoverGatt.as_str(),
            R10BleDeviceTaskHardwareCommand::SubscribeCccd.as_str(),
            R10BleDeviceTaskHardwareCommand::WriteRemoteStart.as_str(),
            R10BleDeviceTaskHardwareCommand::WritePoll.as_str(),
            R10BleDeviceTaskHardwareCommand::HandleNotify.as_str(),
        ];

        assert_eq!(
            labels,
            [
                "scan_start",
                "connect_target",
                "discover_gatt",
                "subscribe_cccd",
                "write_remote_start",
                "write_poll",
                "handle_notify",
            ]
        );

        for label in labels {
            assert!(r10_ble_device_task_monitor_label_is_safe(label));
        }
    }

    #[test]
    fn r10_ble_device_task_hardware_command_maps_probe_lifecycle() {
        let runner = R10BleDeviceTaskRunnerPlan::from_start_request(R10BleDeviceTaskStartRequest {
            action: R10BleDeviceTaskAction::StartProbeOnly,
            config: R10BleRuntimeConfig::probe_only_live_r10(),
        })
        .unwrap();
        let plan = R10BleDeviceTaskHardwareCommandPlan::from_runner_plan(runner);

        let expected = [
            Some("scan_start"),
            Some("connect_target"),
            Some("discover_gatt"),
            Some("subscribe_cccd"),
            Some("write_remote_start"),
            Some("write_poll"),
            Some("write_poll"),
            Some("handle_notify"),
            None,
        ];

        for (index, label) in expected.iter().enumerate() {
            assert_eq!(plan.command_label_at(index), *label);
        }
    }

    #[test]
    fn r10_ble_device_task_hardware_command_timeout_has_no_command() {
        assert_eq!(
            R10BleDeviceTaskHardwareCommand::from_lifecycle(
                R10BleDeviceTaskLifecycleEvent::Timeout
            ),
            None
        );
    }

    #[test]
    fn r10_ble_device_task_hardware_command_sink_records_probe_plan() {
        let runner = R10BleDeviceTaskRunnerPlan::from_start_request(R10BleDeviceTaskStartRequest {
            action: R10BleDeviceTaskAction::StartProbeOnly,
            config: R10BleRuntimeConfig::probe_only_live_r10(),
        })
        .unwrap();
        let mut sink = R10BleDeviceTaskHardwareCommandSink::<8>::new();

        assert_eq!(sink.record_plan(&runner), 8);
        assert_eq!(sink.len(), 8);
        assert_eq!(sink.dropped(), 0);
        assert_eq!(sink.step_at(0).unwrap().command_label(), "scan_start");
        assert_eq!(sink.step_at(5).unwrap().command_label(), "write_poll");
        assert_eq!(sink.last().unwrap().command_label(), "handle_notify");
        assert!(sink.last().unwrap().is_probe_log_only());
        assert!(!sink.last().unwrap().should_emit_reader_event());
    }

    #[test]
    fn r10_ble_device_task_hardware_command_sink_tracks_overflow() {
        let runner = R10BleDeviceTaskRunnerPlan::from_start_request(R10BleDeviceTaskStartRequest {
            action: R10BleDeviceTaskAction::StartProbeOnly,
            config: R10BleRuntimeConfig::probe_only_live_r10(),
        })
        .unwrap();
        let mut sink = R10BleDeviceTaskHardwareCommandSink::<3>::new();

        assert_eq!(sink.record_plan(&runner), 3);
        assert_eq!(sink.len(), 3);
        assert_eq!(sink.dropped(), 5);
        assert_eq!(sink.step_at(0).unwrap().command_label(), "scan_start");
        assert_eq!(sink.step_at(2).unwrap().command_label(), "discover_gatt");
        assert_eq!(sink.step_at(3), None);
    }

    #[test]
    fn r10_ble_device_task_hardware_command_reader_remote_notify_is_reader_capable() {
        let runner = R10BleDeviceTaskRunnerPlan::from_start_request(R10BleDeviceTaskStartRequest {
            action: R10BleDeviceTaskAction::StartReaderRemote,
            config: R10BleRuntimeConfig::reader_remote_live_r10(),
        })
        .unwrap();

        let notify = runner.hardware_command_at(7).unwrap();

        assert_eq!(
            notify.command,
            R10BleDeviceTaskHardwareCommand::HandleNotify
        );
        assert_eq!(notify.reader_label(), "reader_on");
        assert!(notify.should_emit_reader_event());
        assert!(!notify.is_probe_log_only());
        assert!(notify.is_monitor_safe());
    }

    #[test]
    fn r10_ble_device_task_hardware_command_probe_only_notify_is_log_only() {
        let runner = R10BleDeviceTaskRunnerPlan::from_start_request(R10BleDeviceTaskStartRequest {
            action: R10BleDeviceTaskAction::StartProbeOnly,
            config: R10BleRuntimeConfig::probe_only_live_r10(),
        })
        .unwrap();

        let notify = runner.hardware_command_at(7).unwrap();

        assert_eq!(
            notify.command,
            R10BleDeviceTaskHardwareCommand::HandleNotify
        );
        assert_eq!(notify.reader_label(), "reader_off");
        assert!(!notify.should_emit_reader_event());
        assert!(notify.is_probe_log_only());
        assert!(notify.is_monitor_safe());
    }

    #[test]
    fn r10_ble_device_task_hardware_command_disabled_state_emits_nothing() {
        let mut state = R10BleDeviceTaskState::default();
        let mut sink = R10BleDeviceTaskHardwareCommandSink::<8>::new();

        let plan = R10BleDeviceTaskHardwareCommandPlan::from_state_start(&mut state);

        assert_eq!(plan, None);
        assert_eq!(sink.len(), 0);
        assert_eq!(sink.dropped(), 0);
        assert!(sink.is_empty());
        assert!(!state.is_started());

        sink.clear();
        assert!(sink.is_empty());
    }
    #[test]
    fn r10_ble_device_task_hardware_transcript_outcome_labels_are_stable_and_safe() {
        let outcomes = [
            R10BleDeviceTaskHardwareOutcome::Pending,
            R10BleDeviceTaskHardwareOutcome::Started,
            R10BleDeviceTaskHardwareOutcome::Ok,
            R10BleDeviceTaskHardwareOutcome::Ignored,
            R10BleDeviceTaskHardwareOutcome::Failed,
            R10BleDeviceTaskHardwareOutcome::Timeout,
        ];

        let labels = outcomes.map(|outcome| outcome.as_str());

        assert_eq!(
            labels,
            ["pending", "started", "ok", "ignored", "failed", "timeout"]
        );

        for label in labels {
            assert!(r10_ble_device_task_monitor_label_is_safe(label));
        }
    }

    #[test]
    fn r10_ble_device_task_hardware_transcript_outcome_flags_are_explicit() {
        assert!(!R10BleDeviceTaskHardwareOutcome::Pending.is_terminal());
        assert!(!R10BleDeviceTaskHardwareOutcome::Started.is_terminal());
        assert!(R10BleDeviceTaskHardwareOutcome::Ok.is_terminal());
        assert!(R10BleDeviceTaskHardwareOutcome::Ignored.is_terminal());
        assert!(R10BleDeviceTaskHardwareOutcome::Failed.is_terminal());
        assert!(R10BleDeviceTaskHardwareOutcome::Timeout.is_terminal());

        assert!(R10BleDeviceTaskHardwareOutcome::Ok.is_success());
        assert!(!R10BleDeviceTaskHardwareOutcome::Ignored.is_success());
        assert!(R10BleDeviceTaskHardwareOutcome::Failed.should_retry());
        assert!(R10BleDeviceTaskHardwareOutcome::Timeout.should_retry());
    }

    #[test]
    fn r10_ble_device_task_hardware_transcript_entry_is_monitor_safe() {
        let runner = R10BleDeviceTaskRunnerPlan::from_start_request(R10BleDeviceTaskStartRequest {
            action: R10BleDeviceTaskAction::StartProbeOnly,
            config: R10BleRuntimeConfig::probe_only_live_r10(),
        })
        .unwrap();

        let entry = runner
            .hardware_transcript_entry_at(0, R10BleDeviceTaskHardwareOutcome::Started)
            .unwrap();

        assert_eq!(entry.prefix_label(), "ble_hw_command");
        assert_eq!(entry.lifecycle_label(), "start_scan");
        assert_eq!(entry.command_label(), "scan_start");
        assert_eq!(entry.outcome_label(), "started");
        assert_eq!(entry.reader_label(), "reader_off");
        assert_eq!(entry.sequence, 0);
        assert!(!entry.is_terminal());
        assert!(entry.is_monitor_safe());
    }

    #[test]
    fn r10_ble_device_task_hardware_transcript_records_pending_probe_plan() {
        let runner = R10BleDeviceTaskRunnerPlan::from_start_request(R10BleDeviceTaskStartRequest {
            action: R10BleDeviceTaskAction::StartProbeOnly,
            config: R10BleRuntimeConfig::probe_only_live_r10(),
        })
        .unwrap();
        let mut transcript = R10BleDeviceTaskHardwareTranscript::<8>::new();

        assert_eq!(
            transcript
                .record_runner_with_outcome(&runner, R10BleDeviceTaskHardwareOutcome::Pending),
            8
        );

        assert_eq!(transcript.len(), 8);
        assert_eq!(transcript.dropped(), 0);
        assert_eq!(
            transcript.entry_at(0).unwrap().command_label(),
            "scan_start"
        );
        assert_eq!(
            transcript.entry_at(7).unwrap().command_label(),
            "handle_notify"
        );
        assert_eq!(transcript.last().unwrap().outcome_label(), "pending");
        assert!(!transcript.last().unwrap().should_emit_reader_event());
    }

    #[test]
    fn r10_ble_device_task_hardware_transcript_tracks_overflow() {
        let runner = R10BleDeviceTaskRunnerPlan::from_start_request(R10BleDeviceTaskStartRequest {
            action: R10BleDeviceTaskAction::StartProbeOnly,
            config: R10BleRuntimeConfig::probe_only_live_r10(),
        })
        .unwrap();
        let mut transcript = R10BleDeviceTaskHardwareTranscript::<3>::new();

        assert_eq!(
            transcript
                .record_runner_with_outcome(&runner, R10BleDeviceTaskHardwareOutcome::Started),
            3
        );

        assert_eq!(transcript.len(), 3);
        assert_eq!(transcript.dropped(), 5);
        assert_eq!(transcript.entry_at(0).unwrap().outcome_label(), "started");
        assert_eq!(
            transcript.entry_at(2).unwrap().command_label(),
            "discover_gatt"
        );
        assert_eq!(transcript.entry_at(3), None);
    }

    #[test]
    fn r10_ble_device_task_hardware_transcript_clear_resets_state() {
        let runner = R10BleDeviceTaskRunnerPlan::from_start_request(R10BleDeviceTaskStartRequest {
            action: R10BleDeviceTaskAction::StartProbeOnly,
            config: R10BleRuntimeConfig::probe_only_live_r10(),
        })
        .unwrap();
        let mut transcript = R10BleDeviceTaskHardwareTranscript::<4>::new();

        transcript.record_runner_with_outcome(&runner, R10BleDeviceTaskHardwareOutcome::Failed);
        assert!(!transcript.is_empty());
        assert!(transcript.dropped() > 0);

        transcript.clear();

        assert!(transcript.is_empty());
        assert_eq!(transcript.len(), 0);
        assert_eq!(transcript.dropped(), 0);
        assert_eq!(transcript.entry_at(0), None);
    }

    #[test]
    fn r10_ble_device_task_hardware_transcript_reader_remote_notify_ok_emits_reader_event() {
        let runner = R10BleDeviceTaskRunnerPlan::from_start_request(R10BleDeviceTaskStartRequest {
            action: R10BleDeviceTaskAction::StartReaderRemote,
            config: R10BleRuntimeConfig::reader_remote_live_r10(),
        })
        .unwrap();

        let notify = runner
            .hardware_transcript_entry_at(7, R10BleDeviceTaskHardwareOutcome::Ok)
            .unwrap();

        assert_eq!(notify.command_label(), "handle_notify");
        assert_eq!(notify.reader_label(), "reader_on");
        assert_eq!(notify.outcome_label(), "ok");
        assert!(notify.should_emit_reader_event());
        assert!(!notify.is_probe_log_only());
        assert!(notify.is_monitor_safe());
    }

    #[test]
    fn r10_ble_device_task_hardware_transcript_probe_only_notify_ok_is_log_only() {
        let runner = R10BleDeviceTaskRunnerPlan::from_start_request(R10BleDeviceTaskStartRequest {
            action: R10BleDeviceTaskAction::StartProbeOnly,
            config: R10BleRuntimeConfig::probe_only_live_r10(),
        })
        .unwrap();

        let notify = runner
            .hardware_transcript_entry_at(7, R10BleDeviceTaskHardwareOutcome::Ok)
            .unwrap();

        assert_eq!(notify.command_label(), "handle_notify");
        assert_eq!(notify.reader_label(), "reader_off");
        assert_eq!(notify.outcome_label(), "ok");
        assert!(!notify.should_emit_reader_event());
        assert!(notify.is_probe_log_only());
        assert!(notify.is_monitor_safe());
    }

    #[test]
    fn r10_ble_device_task_hardware_transcript_failed_notify_suppresses_reader_event() {
        let runner = R10BleDeviceTaskRunnerPlan::from_start_request(R10BleDeviceTaskStartRequest {
            action: R10BleDeviceTaskAction::StartReaderRemote,
            config: R10BleRuntimeConfig::reader_remote_live_r10(),
        })
        .unwrap();

        let failed = runner
            .hardware_transcript_entry_at(7, R10BleDeviceTaskHardwareOutcome::Failed)
            .unwrap();
        let timeout = runner
            .hardware_transcript_entry_at(7, R10BleDeviceTaskHardwareOutcome::Timeout)
            .unwrap();
        let ignored = runner
            .hardware_transcript_entry_at(7, R10BleDeviceTaskHardwareOutcome::Ignored)
            .unwrap();

        assert!(!failed.should_emit_reader_event());
        assert!(!timeout.should_emit_reader_event());
        assert!(!ignored.should_emit_reader_event());
        assert!(failed.should_retry());
        assert!(timeout.should_retry());
        assert!(!ignored.should_retry());
    }

    #[test]
    fn r10_ble_device_task_hardware_transcript_disabled_state_emits_nothing() {
        let mut state = R10BleDeviceTaskState::default();
        let mut transcript = R10BleDeviceTaskHardwareTranscript::<8>::new();

        let plan = R10BleDeviceTaskHardwareCommandPlan::from_state_start(&mut state);

        assert_eq!(plan, None);
        assert_eq!(transcript.len(), 0);
        assert_eq!(transcript.dropped(), 0);
        assert!(transcript.is_empty());
        assert!(!state.is_started());

        transcript.clear();
        assert!(transcript.is_empty());
    }
    #[test]
    fn r10_ble_device_task_hardware_mock_script_labels_are_stable_and_safe() {
        let scripts = [
            R10BleDeviceTaskHardwareMockOutcomeScript::Success,
            R10BleDeviceTaskHardwareMockOutcomeScript::TimeoutNotify,
            R10BleDeviceTaskHardwareMockOutcomeScript::FailedWrite,
            R10BleDeviceTaskHardwareMockOutcomeScript::IgnoredNotify,
        ];

        let labels = scripts.map(|script| script.as_str());

        assert_eq!(
            labels,
            [
                "success",
                "timeout_notify",
                "failed_write",
                "ignored_notify"
            ]
        );

        for label in labels {
            assert!(r10_ble_device_task_monitor_label_is_safe(label));
        }
    }

    #[test]
    fn r10_ble_device_task_hardware_mock_probe_success_is_log_only() {
        let runner = R10BleDeviceTaskRunnerPlan::from_start_request(R10BleDeviceTaskStartRequest {
            action: R10BleDeviceTaskAction::StartProbeOnly,
            config: R10BleRuntimeConfig::probe_only_live_r10(),
        })
        .unwrap();
        let mut transcript = R10BleDeviceTaskHardwareTranscript::<8>::new();

        let report = R10BleDeviceTaskHardwareMockExecutor::success()
            .execute_runner(&runner, &mut transcript);

        assert_eq!(report.attempted, 8);
        assert_eq!(report.recorded, 8);
        assert_eq!(report.dropped, 0);
        assert_eq!(report.reader_events_emitted, 0);
        assert_eq!(
            report.last_outcome,
            Some(R10BleDeviceTaskHardwareOutcome::Ok)
        );
        assert!(report.completed());
        assert!(!report.emitted_reader_events());

        let notify = transcript.last().unwrap();
        assert_eq!(notify.command_label(), "handle_notify");
        assert_eq!(notify.outcome_label(), "ok");
        assert!(notify.is_probe_log_only());
        assert!(!notify.should_emit_reader_event());
    }

    #[test]
    fn r10_ble_device_task_hardware_mock_reader_remote_success_emits_one_reader_event() {
        let runner = R10BleDeviceTaskRunnerPlan::from_start_request(R10BleDeviceTaskStartRequest {
            action: R10BleDeviceTaskAction::StartReaderRemote,
            config: R10BleRuntimeConfig::reader_remote_live_r10(),
        })
        .unwrap();
        let mut transcript = R10BleDeviceTaskHardwareTranscript::<8>::new();

        let report = R10BleDeviceTaskHardwareMockExecutor::success()
            .execute_runner(&runner, &mut transcript);

        assert_eq!(report.attempted, 8);
        assert_eq!(report.recorded, 8);
        assert_eq!(report.reader_events_emitted, 1);
        assert_eq!(
            report.last_outcome,
            Some(R10BleDeviceTaskHardwareOutcome::Ok)
        );
        assert!(report.completed());
        assert!(report.emitted_reader_events());

        let notify = transcript.last().unwrap();
        assert_eq!(notify.command_label(), "handle_notify");
        assert_eq!(notify.reader_label(), "reader_on");
        assert!(notify.should_emit_reader_event());
    }

    #[test]
    fn r10_ble_device_task_hardware_mock_timeout_notify_requests_retry() {
        let runner = R10BleDeviceTaskRunnerPlan::from_start_request(R10BleDeviceTaskStartRequest {
            action: R10BleDeviceTaskAction::StartReaderRemote,
            config: R10BleRuntimeConfig::reader_remote_live_r10(),
        })
        .unwrap();
        let mut transcript = R10BleDeviceTaskHardwareTranscript::<8>::new();

        let report = R10BleDeviceTaskHardwareMockExecutor::timeout_notify()
            .execute_runner(&runner, &mut transcript);

        assert_eq!(report.attempted, 8);
        assert_eq!(report.recorded, 8);
        assert_eq!(report.reader_events_emitted, 0);
        assert_eq!(
            report.last_outcome,
            Some(R10BleDeviceTaskHardwareOutcome::Timeout)
        );
        assert!(report.retry_requested);
        assert!(!report.emitted_reader_events());

        let notify = transcript.last().unwrap();
        assert_eq!(notify.command_label(), "handle_notify");
        assert_eq!(notify.outcome_label(), "timeout");
        assert!(notify.should_retry());
        assert!(!notify.should_emit_reader_event());
    }

    #[test]
    fn r10_ble_device_task_hardware_mock_failed_write_marks_writes_failed() {
        let runner = R10BleDeviceTaskRunnerPlan::from_start_request(R10BleDeviceTaskStartRequest {
            action: R10BleDeviceTaskAction::StartReaderRemote,
            config: R10BleRuntimeConfig::reader_remote_live_r10(),
        })
        .unwrap();
        let mut transcript = R10BleDeviceTaskHardwareTranscript::<8>::new();

        let report = R10BleDeviceTaskHardwareMockExecutor::failed_write()
            .execute_runner(&runner, &mut transcript);

        assert_eq!(report.attempted, 8);
        assert_eq!(report.recorded, 8);
        assert_eq!(report.reader_events_emitted, 0);
        assert_eq!(
            report.last_outcome,
            Some(R10BleDeviceTaskHardwareOutcome::Ignored)
        );
        assert!(report.retry_requested);

        assert_eq!(
            transcript.entry_at(3).unwrap().command_label(),
            "subscribe_cccd"
        );
        assert_eq!(transcript.entry_at(3).unwrap().outcome_label(), "failed");
        assert_eq!(
            transcript.entry_at(4).unwrap().command_label(),
            "write_remote_start"
        );
        assert_eq!(transcript.entry_at(4).unwrap().outcome_label(), "failed");
        assert_eq!(transcript.last().unwrap().command_label(), "handle_notify");
        assert_eq!(transcript.last().unwrap().outcome_label(), "ignored");
    }

    #[test]
    fn r10_ble_device_task_hardware_mock_ignored_notify_suppresses_reader_event() {
        let runner = R10BleDeviceTaskRunnerPlan::from_start_request(R10BleDeviceTaskStartRequest {
            action: R10BleDeviceTaskAction::StartReaderRemote,
            config: R10BleRuntimeConfig::reader_remote_live_r10(),
        })
        .unwrap();
        let mut transcript = R10BleDeviceTaskHardwareTranscript::<8>::new();

        let report = R10BleDeviceTaskHardwareMockExecutor::ignored_notify()
            .execute_runner(&runner, &mut transcript);

        assert_eq!(report.attempted, 8);
        assert_eq!(report.recorded, 8);
        assert_eq!(report.reader_events_emitted, 0);
        assert_eq!(
            report.last_outcome,
            Some(R10BleDeviceTaskHardwareOutcome::Ignored)
        );
        assert!(!report.retry_requested);
        assert!(!report.emitted_reader_events());

        let notify = transcript.last().unwrap();
        assert_eq!(notify.command_label(), "handle_notify");
        assert_eq!(notify.outcome_label(), "ignored");
        assert!(!notify.should_emit_reader_event());
    }

    #[test]
    fn r10_ble_device_task_hardware_mock_transcript_overflow_is_reported() {
        let runner = R10BleDeviceTaskRunnerPlan::from_start_request(R10BleDeviceTaskStartRequest {
            action: R10BleDeviceTaskAction::StartProbeOnly,
            config: R10BleRuntimeConfig::probe_only_live_r10(),
        })
        .unwrap();
        let mut transcript = R10BleDeviceTaskHardwareTranscript::<3>::new();

        let report = R10BleDeviceTaskHardwareMockExecutor::success()
            .execute_runner(&runner, &mut transcript);

        assert_eq!(report.attempted, 8);
        assert_eq!(report.recorded, 3);
        assert_eq!(report.dropped, 5);
        assert!(!report.completed());
        assert_eq!(transcript.len(), 3);
        assert_eq!(transcript.dropped(), 5);
        assert_eq!(
            transcript.entry_at(0).unwrap().command_label(),
            "scan_start"
        );
        assert_eq!(
            transcript.entry_at(2).unwrap().command_label(),
            "discover_gatt"
        );
        assert_eq!(transcript.entry_at(3), None);
    }

    #[test]
    fn r10_ble_device_task_hardware_mock_disabled_state_emits_nothing() {
        let mut state = R10BleDeviceTaskState::default();
        let mut transcript = R10BleDeviceTaskHardwareTranscript::<8>::new();

        let report = R10BleDeviceTaskHardwareMockExecutor::success()
            .execute_state_start(&mut state, &mut transcript);

        assert_eq!(report, None);
        assert_eq!(transcript.len(), 0);
        assert_eq!(transcript.dropped(), 0);
        assert!(transcript.is_empty());
        assert!(!state.is_started());
    }
    #[test]
    fn r10_ble_device_task_x4_deploy_labels_are_stable_and_safe() {
        let modes = [
            R10BleDeviceTaskX4DeployMode::Disabled,
            R10BleDeviceTaskX4DeployMode::ProbeOnly,
            R10BleDeviceTaskX4DeployMode::ReaderRemote,
        ];

        let labels = modes.map(|mode| mode.as_str());

        assert_eq!(labels, ["disabled", "probe_only", "reader_remote"]);

        for label in labels {
            assert!(r10_ble_device_task_monitor_label_is_safe(label));
        }

        let profile = R10BleDeviceTaskX4DeployProfile::probe_only_live_r10();
        assert_eq!(profile.feature_label(), "r10-ble-host");
        assert_eq!(
            profile.deploy_script(),
            "scripts/x4_r10_ble_probe_deploy.sh"
        );
        assert_eq!(profile.chip_label(), "esp32c3");
    }

    #[test]
    fn r10_ble_device_task_x4_deploy_mode_parser_accepts_safe_aliases() {
        assert_eq!(
            R10BleDeviceTaskX4DeployProfile::from_mode_label("disabled"),
            Some(R10BleDeviceTaskX4DeployProfile::disabled())
        );
        assert_eq!(
            R10BleDeviceTaskX4DeployProfile::from_mode_label("probe_only"),
            Some(R10BleDeviceTaskX4DeployProfile::probe_only_live_r10())
        );
        assert_eq!(
            R10BleDeviceTaskX4DeployProfile::from_mode_label("probe-only"),
            Some(R10BleDeviceTaskX4DeployProfile::probe_only_live_r10())
        );
        assert_eq!(
            R10BleDeviceTaskX4DeployProfile::from_mode_label("reader_remote"),
            Some(R10BleDeviceTaskX4DeployProfile::reader_remote_live_r10())
        );
        assert_eq!(
            R10BleDeviceTaskX4DeployProfile::from_mode_label("live"),
            None
        );
    }

    #[test]
    fn r10_ble_device_task_x4_deploy_disabled_is_noop() {
        let profile = R10BleDeviceTaskX4DeployProfile::disabled();
        let mut transcript = R10BleDeviceTaskHardwareTranscript::<8>::new();

        assert_eq!(profile.mode_label(), "disabled");
        assert!(!profile.should_flash());
        assert!(!profile.reader_events_enabled());
        assert_eq!(profile.start_request(), None);
        assert_eq!(profile.runner_plan(), None);
        assert_eq!(profile.hardware_command_plan(), None);
        assert_eq!(profile.hardware_command_count(), 0);
        assert_eq!(profile.dry_run_success_report(&mut transcript), None);
        assert!(transcript.is_empty());
    }

    #[test]
    fn r10_ble_device_task_x4_deploy_probe_only_is_flashable_log_only() {
        let profile = R10BleDeviceTaskX4DeployProfile::probe_only_live_r10();
        let request = profile.start_request().unwrap();
        let runner = profile.runner_plan().unwrap();

        assert_eq!(profile.mode_label(), "probe_only");
        assert!(profile.should_flash());
        assert!(!profile.reader_events_enabled());
        assert_eq!(request.action, R10BleDeviceTaskAction::StartProbeOnly);
        assert!(runner.should_run_probe());
        assert!(!runner.should_emit_reader_events());
        assert_eq!(profile.hardware_command_count(), 8);
    }

    #[test]
    fn r10_ble_device_task_x4_deploy_reader_remote_is_explicit_opt_in() {
        let profile = R10BleDeviceTaskX4DeployProfile::reader_remote_live_r10();
        let request = profile.start_request().unwrap();
        let runner = profile.runner_plan().unwrap();

        assert_eq!(profile.mode_label(), "reader_remote");
        assert!(profile.should_flash());
        assert!(profile.reader_events_enabled());
        assert_eq!(request.action, R10BleDeviceTaskAction::StartReaderRemote);
        assert!(runner.should_run_reader_remote());
        assert!(runner.should_emit_reader_events());
        assert_eq!(profile.hardware_command_count(), 8);
    }

    #[test]
    fn r10_ble_device_task_x4_deploy_probe_only_dry_run_emits_no_reader_events() {
        let profile = R10BleDeviceTaskX4DeployProfile::probe_only_live_r10();
        let mut transcript = R10BleDeviceTaskHardwareTranscript::<8>::new();

        let report = profile.dry_run_success_report(&mut transcript).unwrap();

        assert_eq!(report.attempted, 8);
        assert_eq!(report.recorded, 8);
        assert_eq!(report.reader_events_emitted, 0);
        assert!(report.completed());
        assert!(!report.emitted_reader_events());
        assert_eq!(transcript.last().unwrap().command_label(), "handle_notify");
        assert_eq!(transcript.last().unwrap().reader_label(), "reader_off");
    }

    #[test]
    fn r10_ble_device_task_x4_deploy_reader_remote_dry_run_can_emit_reader_event() {
        let profile = R10BleDeviceTaskX4DeployProfile::reader_remote_live_r10();
        let mut transcript = R10BleDeviceTaskHardwareTranscript::<8>::new();

        let report = profile.dry_run_success_report(&mut transcript).unwrap();

        assert_eq!(report.attempted, 8);
        assert_eq!(report.recorded, 8);
        assert_eq!(report.reader_events_emitted, 1);
        assert!(report.completed());
        assert!(report.emitted_reader_events());
        assert_eq!(transcript.last().unwrap().command_label(), "handle_notify");
        assert_eq!(transcript.last().unwrap().reader_label(), "reader_on");
    }

    #[test]
    fn r10_ble_device_task_x4_deploy_profile_overflow_is_visible_before_flash() {
        let profile = R10BleDeviceTaskX4DeployProfile::probe_only_live_r10();
        let mut transcript = R10BleDeviceTaskHardwareTranscript::<3>::new();

        let report = profile.dry_run_success_report(&mut transcript).unwrap();

        assert_eq!(report.attempted, 8);
        assert_eq!(report.recorded, 3);
        assert_eq!(report.dropped, 5);
        assert!(!report.completed());
        assert_eq!(transcript.dropped(), 5);
    }
}
