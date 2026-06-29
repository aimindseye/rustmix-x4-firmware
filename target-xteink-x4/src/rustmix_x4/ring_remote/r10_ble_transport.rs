// COLMI R10 BLE transport contract.
//
// This is not the hardware BLE implementation yet. It captures the UUIDs,
// command packets, polling cadence, and notify-to-action bridge so the later
// esp-radio + BLE-host task has a small, tested boundary.

use super::r10_input_bridge::try_enqueue_remote_action;
use super::r10_protocol::{R10_REMOTE_POLL, R10_REMOTE_START, R10_REMOTE_STOP};
use super::r10_remote_policy::{R10RemoteAction, R10RemotePolicy};

pub const R10_BLE_SERVICE_UUID: &str = "6e40fff0-b5a3-f393-e0a9-e50e24dcca9e";
pub const R10_BLE_WRITE_UUID: &str = "6e400002-b5a3-f393-e0a9-e50e24dcca9e";
pub const R10_BLE_NOTIFY_UUID: &str = "6e400003-b5a3-f393-e0a9-e50e24dcca9e";

pub const R10_BLE_POLL_INTERVAL_MS: u64 = 1_000;
pub const R10_BLE_RECONNECT_BACKOFF_MS: u64 = 3_000;

pub const R10_BLE_NOTIFY_CCCD_ENABLE: [u8; 2] = [0x01, 0x00];
pub const R10_BLE_NOTIFY_CCCD_DISABLE: [u8; 2] = [0x00, 0x00];

pub const R10_BLE_DEFAULT_TARGET_ADDRESS: &str = "31:39:46:36:E5:05";
pub const R10_BLE_DEFAULT_ADVERTISED_NAME: &str = "COLMI R10_E505";
pub const R10_BLE_DEFAULT_NAME_PREFIX: &str = "COLMI R10";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R10BleAdvertisedDevice<'a> {
    pub address: Option<&'a str>,
    pub name: Option<&'a str>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R10BleScanTarget {
    pub address: Option<&'static str>,
    pub name_prefix: Option<&'static str>,
}

impl R10BleScanTarget {
    pub const fn default_r10() -> Self {
        Self {
            address: Some(R10_BLE_DEFAULT_TARGET_ADDRESS),
            name_prefix: Some(R10_BLE_DEFAULT_NAME_PREFIX),
        }
    }

    pub const fn any_colmi_r10() -> Self {
        Self {
            address: None,
            name_prefix: Some(R10_BLE_DEFAULT_NAME_PREFIX),
        }
    }

    pub fn scan_device(&self, device: R10BleAdvertisedDevice<'_>) -> R10BleScanDecision {
        if let (Some(expected), Some(actual)) = (self.address, device.address) {
            if expected.eq_ignore_ascii_case(actual) {
                return R10BleScanDecision::MatchByAddress;
            }
        }

        if let (Some(prefix), Some(name)) = (self.name_prefix, device.name) {
            if name.starts_with(prefix) {
                return R10BleScanDecision::MatchByNamePrefix;
            }
        }

        R10BleScanDecision::Ignore
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R10BleScanDecision {
    Ignore,
    MatchByAddress,
    MatchByNamePrefix,
}

impl R10BleScanDecision {
    pub const fn is_match(&self) -> bool {
        matches!(self, Self::MatchByAddress | Self::MatchByNamePrefix)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R10BleGattDiscoveryEvent {
    Service { start_handle: u16, end_handle: u16 },
    WriteCharacteristic { value_handle: u16 },
    NotifyCharacteristic { value_handle: u16 },
    NotifyCccd { handle: u16 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R10BleGattDiscoveryStatus {
    Ready(R10BleGattHandles),
    Incomplete(R10BleGattHandles),
}

impl R10BleGattDiscoveryStatus {
    pub fn handles(&self) -> R10BleGattHandles {
        match self {
            Self::Ready(handles) | Self::Incomplete(handles) => *handles,
        }
    }

    pub const fn is_ready(&self) -> bool {
        matches!(self, Self::Ready(_))
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct R10BleGattHandles {
    pub service_start_handle: Option<u16>,
    pub service_end_handle: Option<u16>,
    pub write_value_handle: Option<u16>,
    pub notify_value_handle: Option<u16>,
    pub notify_cccd_handle: Option<u16>,
}

impl R10BleGattHandles {
    pub const fn unresolved() -> Self {
        Self {
            service_start_handle: None,
            service_end_handle: None,
            write_value_handle: None,
            notify_value_handle: None,
            notify_cccd_handle: None,
        }
    }

    pub const fn new(
        service_start_handle: u16,
        service_end_handle: u16,
        write_value_handle: u16,
        notify_value_handle: u16,
        notify_cccd_handle: u16,
    ) -> Self {
        Self {
            service_start_handle: Some(service_start_handle),
            service_end_handle: Some(service_end_handle),
            write_value_handle: Some(write_value_handle),
            notify_value_handle: Some(notify_value_handle),
            notify_cccd_handle: Some(notify_cccd_handle),
        }
    }

    pub fn apply_discovery_event(&mut self, event: R10BleGattDiscoveryEvent) {
        match event {
            R10BleGattDiscoveryEvent::Service {
                start_handle,
                end_handle,
            } => {
                self.service_start_handle = Some(start_handle);
                self.service_end_handle = Some(end_handle);
            }
            R10BleGattDiscoveryEvent::WriteCharacteristic { value_handle } => {
                self.write_value_handle = Some(value_handle);
            }
            R10BleGattDiscoveryEvent::NotifyCharacteristic { value_handle } => {
                self.notify_value_handle = Some(value_handle);
            }
            R10BleGattDiscoveryEvent::NotifyCccd { handle } => {
                self.notify_cccd_handle = Some(handle);
            }
        }
    }

    pub fn with_discovery_event(mut self, event: R10BleGattDiscoveryEvent) -> Self {
        self.apply_discovery_event(event);
        self
    }

    pub fn service_range_valid(&self) -> bool {
        match (self.service_start_handle, self.service_end_handle) {
            (Some(start), Some(end)) => start != 0 && start <= end,
            _ => false,
        }
    }

    pub fn contains_handle(&self, handle: u16) -> bool {
        match (self.service_start_handle, self.service_end_handle) {
            (Some(start), Some(end)) => start <= handle && handle <= end,
            _ => false,
        }
    }

    pub fn write_handle_ready(&self) -> bool {
        self.write_value_handle
            .map(|handle| self.contains_handle(handle))
            .unwrap_or(false)
    }

    pub fn notify_handle_ready(&self) -> bool {
        self.notify_value_handle
            .map(|handle| self.contains_handle(handle))
            .unwrap_or(false)
    }

    pub fn notify_cccd_ready(&self) -> bool {
        self.notify_cccd_handle
            .map(|handle| self.contains_handle(handle))
            .unwrap_or(false)
    }

    pub fn is_complete(&self) -> bool {
        self.service_range_valid()
            && self.write_handle_ready()
            && self.notify_handle_ready()
            && self.notify_cccd_ready()
    }

    pub fn can_start_remote_mode(&self) -> bool {
        self.is_complete()
    }

    pub fn discovery_status(&self) -> R10BleGattDiscoveryStatus {
        if self.is_complete() {
            R10BleGattDiscoveryStatus::Ready(*self)
        } else {
            R10BleGattDiscoveryStatus::Incomplete(*self)
        }
    }

    pub fn enable_notify_operation(&self) -> Option<R10BleGattOperation> {
        self.notify_cccd_handle
            .filter(|handle| self.contains_handle(*handle))
            .map(|handle| R10BleGattOperation::WriteCccd {
                handle,
                value: R10_BLE_NOTIFY_CCCD_ENABLE,
            })
    }

    pub fn disable_notify_operation(&self) -> Option<R10BleGattOperation> {
        self.notify_cccd_handle
            .filter(|handle| self.contains_handle(*handle))
            .map(|handle| R10BleGattOperation::WriteCccd {
                handle,
                value: R10_BLE_NOTIFY_CCCD_DISABLE,
            })
    }

    pub fn remote_command_operation(&self, command: R10BleCommand) -> Option<R10BleGattOperation> {
        self.write_value_handle
            .filter(|handle| self.contains_handle(*handle))
            .map(|handle| R10BleGattOperation::WriteRemoteCommand { handle, command })
    }

    pub fn start_remote_operation(&self) -> Option<R10BleGattOperation> {
        if self.can_start_remote_mode() {
            self.remote_command_operation(R10BleCommand::StartRemote)
        } else {
            None
        }
    }

    pub fn poll_remote_operation(&self) -> Option<R10BleGattOperation> {
        if self.can_start_remote_mode() {
            self.remote_command_operation(R10BleCommand::PollRemote)
        } else {
            None
        }
    }

    pub fn stop_remote_operation(&self) -> Option<R10BleGattOperation> {
        self.remote_command_operation(R10BleCommand::StopRemote)
    }

    pub fn startup_operations(&self) -> Option<[R10BleGattOperation; 2]> {
        Some([
            self.enable_notify_operation()?,
            self.start_remote_operation()?,
        ])
    }

    pub fn startup_writes(&self) -> Option<[R10BleGattWrite; 2]> {
        let ops = self.startup_operations()?;
        Some([ops[0].to_write(), ops[1].to_write()])
    }

    pub fn poll_write(&self) -> Option<R10BleGattWrite> {
        self.poll_remote_operation()
            .map(R10BleGattOperation::to_write)
    }

    pub fn stop_write(&self) -> Option<R10BleGattWrite> {
        self.stop_remote_operation()
            .map(R10BleGattOperation::to_write)
    }

    pub fn shutdown_writes(&self) -> Option<[R10BleGattWrite; 2]> {
        Some([
            self.stop_remote_operation()?.to_write(),
            self.disable_notify_operation()?.to_write(),
        ])
    }

    pub fn gate_notify(&self, handle: u16, payload: &[u8]) -> R10BleGattNotifyGate {
        let expected = self.notify_value_handle;

        if expected != Some(handle) {
            return R10BleGattNotifyGate::WrongHandle {
                expected,
                actual: handle,
            };
        }

        if payload.len() != 16 {
            return R10BleGattNotifyGate::WrongLength {
                actual: payload.len(),
            };
        }

        let mut packet = [0u8; 16];
        packet.copy_from_slice(payload);
        R10BleGattNotifyGate::Accepted(packet)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R10BleGattNotifyGate {
    Accepted([u8; 16]),
    WrongHandle { expected: Option<u16>, actual: u16 },
    WrongLength { actual: usize },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R10BleGattNotifyPolicyResult {
    Rejected(R10BleGattNotifyGate),
    Accepted(R10RemoteAction),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R10BleGattOperation {
    WriteCccd { handle: u16, value: [u8; 2] },
    WriteRemoteCommand { handle: u16, command: R10BleCommand },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R10BleGattWritePayload {
    Cccd([u8; 2]),
    RemoteCommand([u8; 16]),
}

impl R10BleGattWritePayload {
    pub fn as_slice(&self) -> &[u8] {
        match self {
            Self::Cccd(value) => value,
            Self::RemoteCommand(value) => value,
        }
    }

    pub const fn len(&self) -> usize {
        match self {
            Self::Cccd(_) => 2,
            Self::RemoteCommand(_) => 16,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R10BleGattWriteMode {
    WithResponse,
    WithoutResponse,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R10BleGattWrite {
    pub handle: u16,
    pub payload: R10BleGattWritePayload,
    pub mode: R10BleGattWriteMode,
}

impl R10BleGattWrite {
    pub fn payload(&self) -> &[u8] {
        self.payload.as_slice()
    }

    pub const fn payload_len(&self) -> usize {
        self.payload.len()
    }

    pub const fn write_mode(&self) -> R10BleGattWriteMode {
        self.mode
    }
}

impl R10BleGattOperation {
    pub fn to_write(self) -> R10BleGattWrite {
        match self {
            Self::WriteCccd { handle, value } => R10BleGattWrite {
                handle,
                payload: R10BleGattWritePayload::Cccd(value),
                mode: R10BleGattWriteMode::WithResponse,
            },
            Self::WriteRemoteCommand { handle, command } => R10BleGattWrite {
                handle,
                payload: R10BleGattWritePayload::RemoteCommand(*command.packet()),
                mode: R10BleGattWriteMode::WithoutResponse,
            },
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R10BleCommand {
    StartRemote,
    PollRemote,
    StopRemote,
}

impl R10BleCommand {
    pub fn packet(self) -> &'static [u8; 16] {
        match self {
            Self::StartRemote => R10_REMOTE_START.as_slice(),
            Self::PollRemote => R10_REMOTE_POLL.as_slice(),
            Self::StopRemote => R10_REMOTE_STOP.as_slice(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R10BleTransportState {
    Disabled,
    Idle,
    Scanning,
    Connecting,
    Discovering,
    Subscribing,
    StartingRemote,
    Polling,
    Disconnecting,
    Backoff,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R10BleRemoteSession {
    pub state: R10BleTransportState,
    pub policy: R10RemotePolicy,
    pub poll_interval_ms: u64,
    pub next_poll_due_ms: u64,
}

impl R10BleRemoteSession {
    pub const fn disabled() -> Self {
        Self {
            state: R10BleTransportState::Disabled,
            policy: R10RemotePolicy::disabled(),
            poll_interval_ms: R10_BLE_POLL_INTERVAL_MS,
            next_poll_due_ms: 0,
        }
    }

    pub const fn reader_remote(debounce_ms: u64) -> Self {
        Self {
            state: R10BleTransportState::Idle,
            policy: R10RemotePolicy::enabled_for_reader(debounce_ms),
            poll_interval_ms: R10_BLE_POLL_INTERVAL_MS,
            next_poll_due_ms: 0,
        }
    }

    pub fn on_notify(&mut self, now_ms: u64, packet: &[u8]) -> R10RemoteAction {
        self.policy.handle_packet(now_ms, packet)
    }

    pub fn on_notify_enqueue(&mut self, now_ms: u64, packet: &[u8]) -> bool {
        let action = self.on_notify(now_ms, packet);
        try_enqueue_remote_action(action)
    }

    pub fn on_gatt_notify(
        &mut self,
        handles: &R10BleGattHandles,
        handle: u16,
        payload: &[u8],
        now_ms: u64,
    ) -> R10BleGattNotifyPolicyResult {
        match handles.gate_notify(handle, payload) {
            R10BleGattNotifyGate::Accepted(packet) => {
                R10BleGattNotifyPolicyResult::Accepted(self.on_notify(now_ms, &packet))
            }
            rejected => R10BleGattNotifyPolicyResult::Rejected(rejected),
        }
    }

    pub fn on_gatt_notify_enqueue(
        &mut self,
        handles: &R10BleGattHandles,
        handle: u16,
        payload: &[u8],
        now_ms: u64,
    ) -> bool {
        match self.on_gatt_notify(handles, handle, payload, now_ms) {
            R10BleGattNotifyPolicyResult::Accepted(action) => try_enqueue_remote_action(action),
            R10BleGattNotifyPolicyResult::Rejected(_) => false,
        }
    }

    pub fn should_poll(&self, now_ms: u64) -> bool {
        matches!(self.state, R10BleTransportState::Polling) && now_ms >= self.next_poll_due_ms
    }

    pub fn mark_poll_sent(&mut self, now_ms: u64) {
        self.next_poll_due_ms = now_ms.saturating_add(self.poll_interval_ms);
    }

    pub fn poll_due_write(
        &mut self,
        handles: &R10BleGattHandles,
        now_ms: u64,
    ) -> Option<R10BleGattWrite> {
        if !self.should_poll(now_ms) {
            return None;
        }

        let write = handles.poll_write()?;
        self.mark_poll_sent(now_ms);
        Some(write)
    }

    pub fn begin_scan(&mut self) {
        self.state = R10BleTransportState::Scanning;
        self.next_poll_due_ms = 0;
    }

    pub fn begin_connect(&mut self) {
        self.state = R10BleTransportState::Connecting;
    }

    pub fn begin_discovery(&mut self) {
        self.state = R10BleTransportState::Discovering;
    }

    pub fn complete_discovery(&mut self, handles: R10BleGattHandles) -> R10BleGattDiscoveryStatus {
        let status = handles.discovery_status();

        if status.is_ready() {
            self.state = R10BleTransportState::Subscribing;
        }

        status
    }

    pub fn begin_subscribe(&mut self, handles: &R10BleGattHandles) -> Option<R10BleGattWrite> {
        let write = handles.enable_notify_operation()?.to_write();
        self.state = R10BleTransportState::Subscribing;
        Some(write)
    }

    pub fn complete_subscribe(&mut self) {
        self.state = R10BleTransportState::StartingRemote;
    }

    pub fn backoff_due(&self, now_ms: u64) -> bool {
        matches!(self.state, R10BleTransportState::Backoff) && now_ms >= self.next_poll_due_ms
    }

    pub fn retry_after_backoff(&mut self, now_ms: u64) -> bool {
        if !self.backoff_due(now_ms) {
            return false;
        }

        self.begin_scan();
        true
    }

    pub fn begin_remote_start(
        &mut self,
        handles: &R10BleGattHandles,
    ) -> Option<[R10BleGattWrite; 2]> {
        let writes = handles.startup_writes()?;
        self.state = R10BleTransportState::StartingRemote;
        Some(writes)
    }

    pub fn complete_remote_start(&mut self, now_ms: u64) {
        self.start_polling(now_ms);
    }

    pub fn begin_remote_shutdown(
        &mut self,
        handles: &R10BleGattHandles,
    ) -> Option<[R10BleGattWrite; 2]> {
        let writes = handles.shutdown_writes()?;
        self.state = R10BleTransportState::Disconnecting;
        Some(writes)
    }

    pub fn complete_remote_shutdown(&mut self) {
        self.state = R10BleTransportState::Idle;
        self.next_poll_due_ms = 0;
    }

    pub fn start_polling(&mut self, now_ms: u64) {
        self.state = R10BleTransportState::Polling;
        self.next_poll_due_ms = now_ms;
    }

    pub fn enter_backoff(&mut self, now_ms: u64) {
        self.state = R10BleTransportState::Backoff;
        self.next_poll_due_ms = now_ms.saturating_add(R10_BLE_RECONNECT_BACKOFF_MS);
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R10BleConnectionResult {
    Connected,
    ConnectFailed,
    LinkLost,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R10BleWritePhase {
    Subscribe,
    RemoteStart,
    Poll,
    Shutdown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R10BleWriteResult {
    Success(R10BleWritePhase),
    Failed(R10BleWritePhase),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R10BlePendingWrite {
    pub phase: R10BleWritePhase,
    pub write: R10BleGattWrite,
}

impl R10BlePendingWrite {
    pub const fn new(phase: R10BleWritePhase, write: R10BleGattWrite) -> Self {
        Self { phase, write }
    }

    pub const fn handle(&self) -> u16 {
        self.write.handle
    }

    pub fn payload(&self) -> &[u8] {
        self.write.payload()
    }

    pub const fn write_mode(&self) -> R10BleGattWriteMode {
        self.write.write_mode()
    }
}

pub const R10_BLE_PENDING_WRITE_QUEUE_CAPACITY: usize = 3;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R10BlePendingWriteQueue {
    writes: [Option<R10BlePendingWrite>; R10_BLE_PENDING_WRITE_QUEUE_CAPACITY],
    len: usize,
    cursor: usize,
}

impl R10BlePendingWriteQueue {
    pub const fn empty() -> Self {
        Self {
            writes: [None, None, None],
            len: 0,
            cursor: 0,
        }
    }

    pub fn from_one(write: R10BlePendingWrite) -> Self {
        Self {
            writes: [Some(write), None, None],
            len: 1,
            cursor: 0,
        }
    }

    pub fn from_two(writes: [R10BlePendingWrite; 2]) -> Self {
        Self {
            writes: [Some(writes[0]), Some(writes[1]), None],
            len: 2,
            cursor: 0,
        }
    }

    pub fn from_three(writes: [R10BlePendingWrite; 3]) -> Self {
        Self {
            writes: [Some(writes[0]), Some(writes[1]), Some(writes[2])],
            len: 3,
            cursor: 0,
        }
    }

    pub const fn len(&self) -> usize {
        self.len
    }

    pub const fn cursor(&self) -> usize {
        self.cursor
    }

    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub const fn is_done(&self) -> bool {
        self.cursor >= self.len
    }

    pub const fn remaining_len(&self) -> usize {
        if self.cursor >= self.len {
            0
        } else {
            self.len - self.cursor
        }
    }

    pub fn current(&self) -> Option<R10BlePendingWrite> {
        if self.cursor < self.len {
            self.writes[self.cursor]
        } else {
            None
        }
    }

    pub fn pending_at(&self, index: usize) -> Option<R10BlePendingWrite> {
        if index < self.len {
            self.writes[index]
        } else {
            None
        }
    }

    pub fn advance(&mut self) -> Option<R10BlePendingWrite> {
        if self.cursor < self.len {
            self.cursor += 1;
        }

        self.current()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R10BleRuntimeEffect {
    None,
    Scan(R10BleScanDecision),
    Connection(R10BleConnectionResult),
    WriteResult(R10BleWriteResult),
    PendingWrite(R10BlePendingWrite),
    PendingWrites([R10BlePendingWrite; 2]),
    PendingStartupWrites([R10BlePendingWrite; 3]),
    PendingWriteQueue(R10BlePendingWriteQueue),
    Discovery(R10BleGattDiscoveryStatus),
    Write(R10BleGattWrite),
    Writes([R10BleGattWrite; 2]),
    Notify(R10BleGattNotifyPolicyResult),
    BackoffReady,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R10BleRemoteRuntime {
    pub session: R10BleRemoteSession,
    pub handles: R10BleGattHandles,
}

impl R10BleRemoteRuntime {
    pub const fn disabled() -> Self {
        Self {
            session: R10BleRemoteSession::disabled(),
            handles: R10BleGattHandles::unresolved(),
        }
    }

    pub const fn reader_remote(debounce_ms: u64) -> Self {
        Self {
            session: R10BleRemoteSession::reader_remote(debounce_ms),
            handles: R10BleGattHandles::unresolved(),
        }
    }

    pub fn reset_handles(&mut self) {
        self.handles = R10BleGattHandles::unresolved();
    }

    pub fn apply_discovery_event(
        &mut self,
        event: R10BleGattDiscoveryEvent,
    ) -> R10BleGattDiscoveryStatus {
        self.handles.apply_discovery_event(event);
        self.handles.discovery_status()
    }

    pub fn complete_discovery(&mut self) -> R10BleGattDiscoveryStatus {
        self.session.complete_discovery(self.handles)
    }

    pub fn begin_subscribe(&mut self) -> Option<R10BleGattWrite> {
        self.session.begin_subscribe(&self.handles)
    }

    pub fn complete_subscribe(&mut self) {
        self.session.complete_subscribe();
    }

    pub fn begin_remote_start(&mut self) -> Option<[R10BleGattWrite; 2]> {
        self.session.begin_remote_start(&self.handles)
    }

    pub fn complete_remote_start(&mut self, now_ms: u64) {
        self.session.complete_remote_start(now_ms);
    }

    pub fn poll_due_write(&mut self, now_ms: u64) -> Option<R10BleGattWrite> {
        self.session.poll_due_write(&self.handles, now_ms)
    }

    pub fn on_gatt_notify(
        &mut self,
        handle: u16,
        payload: &[u8],
        now_ms: u64,
    ) -> R10BleGattNotifyPolicyResult {
        self.session
            .on_gatt_notify(&self.handles, handle, payload, now_ms)
    }

    pub fn on_gatt_notify_enqueue(&mut self, handle: u16, payload: &[u8], now_ms: u64) -> bool {
        self.session
            .on_gatt_notify_enqueue(&self.handles, handle, payload, now_ms)
    }

    pub fn begin_remote_shutdown(&mut self) -> Option<[R10BleGattWrite; 2]> {
        self.session.begin_remote_shutdown(&self.handles)
    }

    pub fn complete_remote_shutdown(&mut self) {
        self.session.complete_remote_shutdown();
    }

    pub fn disconnect_and_backoff(&mut self, now_ms: u64) {
        self.reset_handles();
        self.session.enter_backoff(now_ms);
    }

    pub fn retry_after_backoff(&mut self, now_ms: u64) -> bool {
        self.session.retry_after_backoff(now_ms)
    }

    pub fn connect_success_effect(&mut self) -> R10BleRuntimeEffect {
        self.reset_handles();
        self.session.begin_discovery();
        R10BleRuntimeEffect::Connection(R10BleConnectionResult::Connected)
    }

    pub fn connect_failed_effect(&mut self, now_ms: u64) -> R10BleRuntimeEffect {
        self.disconnect_and_backoff(now_ms);
        R10BleRuntimeEffect::Connection(R10BleConnectionResult::ConnectFailed)
    }

    pub fn link_lost_effect(&mut self, now_ms: u64) -> R10BleRuntimeEffect {
        self.disconnect_and_backoff(now_ms);
        R10BleRuntimeEffect::Connection(R10BleConnectionResult::LinkLost)
    }

    pub fn subscribe_pending_write_effect(&mut self) -> R10BleRuntimeEffect {
        self.begin_subscribe()
            .map(|write| R10BlePendingWrite::new(R10BleWritePhase::Subscribe, write))
            .map(R10BleRuntimeEffect::PendingWrite)
            .unwrap_or(R10BleRuntimeEffect::None)
    }

    pub fn remote_start_pending_writes_effect(&mut self) -> R10BleRuntimeEffect {
        self.begin_remote_start()
            .map(|writes| {
                [
                    R10BlePendingWrite::new(R10BleWritePhase::RemoteStart, writes[0]),
                    R10BlePendingWrite::new(R10BleWritePhase::RemoteStart, writes[1]),
                ]
            })
            .map(R10BleRuntimeEffect::PendingWrites)
            .unwrap_or(R10BleRuntimeEffect::None)
    }

    pub fn poll_tick_pending_write_effect(&mut self, now_ms: u64) -> R10BleRuntimeEffect {
        self.poll_due_write(now_ms)
            .map(|write| R10BlePendingWrite::new(R10BleWritePhase::Poll, write))
            .map(R10BleRuntimeEffect::PendingWrite)
            .unwrap_or(R10BleRuntimeEffect::None)
    }

    pub fn startup_pending_writes(&mut self) -> Option<[R10BlePendingWrite; 3]> {
        let subscribe = self.begin_subscribe()?;
        self.complete_subscribe();

        let startup = self.begin_remote_start()?;

        Some([
            R10BlePendingWrite::new(R10BleWritePhase::Subscribe, subscribe),
            R10BlePendingWrite::new(R10BleWritePhase::RemoteStart, startup[0]),
            R10BlePendingWrite::new(R10BleWritePhase::RemoteStart, startup[1]),
        ])
    }

    pub fn startup_pending_writes_effect(&mut self) -> R10BleRuntimeEffect {
        self.startup_pending_writes()
            .map(R10BleRuntimeEffect::PendingStartupWrites)
            .unwrap_or(R10BleRuntimeEffect::None)
    }

    pub fn shutdown_pending_writes(&mut self) -> Option<[R10BlePendingWrite; 2]> {
        self.begin_remote_shutdown().map(|writes| {
            [
                R10BlePendingWrite::new(R10BleWritePhase::Shutdown, writes[0]),
                R10BlePendingWrite::new(R10BleWritePhase::Shutdown, writes[1]),
            ]
        })
    }

    pub fn shutdown_pending_writes_effect(&mut self) -> R10BleRuntimeEffect {
        self.begin_remote_shutdown()
            .map(|writes| {
                [
                    R10BlePendingWrite::new(R10BleWritePhase::Shutdown, writes[0]),
                    R10BlePendingWrite::new(R10BleWritePhase::Shutdown, writes[1]),
                ]
            })
            .map(R10BleRuntimeEffect::PendingWrites)
            .unwrap_or(R10BleRuntimeEffect::None)
    }

    pub fn subscribe_pending_queue_effect(&mut self) -> R10BleRuntimeEffect {
        self.begin_subscribe()
            .map(|write| R10BlePendingWrite::new(R10BleWritePhase::Subscribe, write))
            .map(R10BlePendingWriteQueue::from_one)
            .map(R10BleRuntimeEffect::PendingWriteQueue)
            .unwrap_or(R10BleRuntimeEffect::None)
    }

    pub fn startup_pending_queue_effect(&mut self) -> R10BleRuntimeEffect {
        self.startup_pending_writes()
            .map(R10BlePendingWriteQueue::from_three)
            .map(R10BleRuntimeEffect::PendingWriteQueue)
            .unwrap_or(R10BleRuntimeEffect::None)
    }

    pub fn poll_pending_queue_effect(&mut self, now_ms: u64) -> R10BleRuntimeEffect {
        self.poll_due_write(now_ms)
            .map(|write| R10BlePendingWrite::new(R10BleWritePhase::Poll, write))
            .map(R10BlePendingWriteQueue::from_one)
            .map(R10BleRuntimeEffect::PendingWriteQueue)
            .unwrap_or(R10BleRuntimeEffect::None)
    }

    pub fn shutdown_pending_queue_effect(&mut self) -> R10BleRuntimeEffect {
        self.shutdown_pending_writes()
            .map(R10BlePendingWriteQueue::from_two)
            .map(R10BleRuntimeEffect::PendingWriteQueue)
            .unwrap_or(R10BleRuntimeEffect::None)
    }

    pub fn write_success_effect(
        &mut self,
        phase: R10BleWritePhase,
        now_ms: u64,
    ) -> R10BleRuntimeEffect {
        match phase {
            R10BleWritePhase::Subscribe => self.complete_subscribe(),
            R10BleWritePhase::RemoteStart => self.complete_remote_start(now_ms),
            R10BleWritePhase::Poll => {}
            R10BleWritePhase::Shutdown => self.complete_remote_shutdown(),
        }

        R10BleRuntimeEffect::WriteResult(R10BleWriteResult::Success(phase))
    }

    pub fn write_failed_effect(
        &mut self,
        phase: R10BleWritePhase,
        now_ms: u64,
    ) -> R10BleRuntimeEffect {
        self.disconnect_and_backoff(now_ms);
        R10BleRuntimeEffect::WriteResult(R10BleWriteResult::Failed(phase))
    }

    pub fn scan_device_effect(
        &mut self,
        target: R10BleScanTarget,
        device: R10BleAdvertisedDevice<'_>,
    ) -> R10BleRuntimeEffect {
        let decision = target.scan_device(device);

        if decision.is_match() {
            self.session.begin_connect();
        }

        R10BleRuntimeEffect::Scan(decision)
    }

    pub fn discovery_event_effect(
        &mut self,
        event: R10BleGattDiscoveryEvent,
    ) -> R10BleRuntimeEffect {
        R10BleRuntimeEffect::Discovery(self.apply_discovery_event(event))
    }

    pub fn discovery_complete_effect(&mut self) -> R10BleRuntimeEffect {
        R10BleRuntimeEffect::Discovery(self.complete_discovery())
    }

    pub fn subscribe_effect(&mut self) -> R10BleRuntimeEffect {
        self.begin_subscribe()
            .map(R10BleRuntimeEffect::Write)
            .unwrap_or(R10BleRuntimeEffect::None)
    }

    pub fn remote_start_effect(&mut self) -> R10BleRuntimeEffect {
        self.begin_remote_start()
            .map(R10BleRuntimeEffect::Writes)
            .unwrap_or(R10BleRuntimeEffect::None)
    }

    pub fn poll_tick_effect(&mut self, now_ms: u64) -> R10BleRuntimeEffect {
        self.poll_due_write(now_ms)
            .map(R10BleRuntimeEffect::Write)
            .unwrap_or(R10BleRuntimeEffect::None)
    }

    pub fn notify_effect(
        &mut self,
        handle: u16,
        payload: &[u8],
        now_ms: u64,
    ) -> R10BleRuntimeEffect {
        R10BleRuntimeEffect::Notify(self.on_gatt_notify(handle, payload, now_ms))
    }

    pub fn shutdown_effect(&mut self) -> R10BleRuntimeEffect {
        self.begin_remote_shutdown()
            .map(R10BleRuntimeEffect::Writes)
            .unwrap_or(R10BleRuntimeEffect::None)
    }

    pub fn disconnect_effect(&mut self, now_ms: u64) -> R10BleRuntimeEffect {
        self.disconnect_and_backoff(now_ms);
        R10BleRuntimeEffect::None
    }

    pub fn backoff_tick_effect(&mut self, now_ms: u64) -> R10BleRuntimeEffect {
        if self.retry_after_backoff(now_ms) {
            R10BleRuntimeEffect::BackoffReady
        } else {
            R10BleRuntimeEffect::None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rustmix_x4::contracts::input_semantics::RustmixReaderAction;
    use crate::rustmix_x4::ring_remote::r10_remote_policy::R10RemoteAction;

    const MOTION: [u8; 16] = [0x02, 0x02, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x04];

    #[test]
    fn r10_ble_pending_write_queue_tracks_current_advance_and_remaining() {
        let pending = R10BlePendingWrite::new(
            R10BleWritePhase::Subscribe,
            R10BleGattWrite {
                handle: 6,
                payload: R10BleGattWritePayload::Cccd(R10_BLE_NOTIFY_CCCD_ENABLE),
                mode: R10BleGattWriteMode::WithResponse,
            },
        );
        let mut queue = R10BlePendingWriteQueue::from_one(pending);

        assert_eq!(queue.len(), 1);
        assert_eq!(queue.cursor(), 0);
        assert_eq!(queue.remaining_len(), 1);
        assert_eq!(queue.current(), Some(pending));
        assert_eq!(queue.advance(), None);
        assert!(queue.is_done());
        assert_eq!(queue.remaining_len(), 0);
    }

    #[test]
    fn r10_ble_pending_write_queue_from_three_preserves_order() {
        let writes = [
            R10BlePendingWrite::new(
                R10BleWritePhase::Subscribe,
                R10BleGattHandles::new(1, 8, 3, 5, 6)
                    .enable_notify_operation()
                    .unwrap()
                    .to_write(),
            ),
            R10BlePendingWrite::new(
                R10BleWritePhase::RemoteStart,
                R10BleGattHandles::new(1, 8, 3, 5, 6)
                    .enable_notify_operation()
                    .unwrap()
                    .to_write(),
            ),
            R10BlePendingWrite::new(
                R10BleWritePhase::RemoteStart,
                R10BleGattHandles::new(1, 8, 3, 5, 6)
                    .start_remote_operation()
                    .unwrap()
                    .to_write(),
            ),
        ];

        let mut queue = R10BlePendingWriteQueue::from_three(writes);

        assert_eq!(queue.len(), 3);
        assert_eq!(queue.current(), Some(writes[0]));
        assert_eq!(queue.advance(), Some(writes[1]));
        assert_eq!(queue.advance(), Some(writes[2]));
        assert_eq!(queue.advance(), None);
        assert!(queue.is_done());
    }

    #[test]
    fn r10_ble_runtime_startup_pending_queue_effect_emits_cursor_queue() {
        let mut runtime = R10BleRemoteRuntime::reader_remote(3500);
        runtime.handles = R10BleGattHandles::new(1, 8, 3, 5, 6);

        match runtime.startup_pending_queue_effect() {
            R10BleRuntimeEffect::PendingWriteQueue(mut queue) => {
                assert_eq!(queue.len(), 3);
                assert_eq!(queue.current().unwrap().phase, R10BleWritePhase::Subscribe);
                assert_eq!(
                    queue.advance().unwrap().phase,
                    R10BleWritePhase::RemoteStart
                );
                assert_eq!(
                    queue.advance().unwrap().phase,
                    R10BleWritePhase::RemoteStart
                );
                assert_eq!(queue.advance(), None);
            }
            other => panic!("unexpected effect: {other:?}"),
        }

        assert_eq!(runtime.session.state, R10BleTransportState::StartingRemote);
    }

    #[test]
    fn r10_ble_runtime_subscribe_pending_queue_effect_emits_single_write_queue() {
        let mut runtime = R10BleRemoteRuntime::reader_remote(3500);
        runtime.handles = R10BleGattHandles::new(1, 8, 3, 5, 6);

        match runtime.subscribe_pending_queue_effect() {
            R10BleRuntimeEffect::PendingWriteQueue(queue) => {
                assert_eq!(queue.len(), 1);
                let current = queue.current().unwrap();
                assert_eq!(current.phase, R10BleWritePhase::Subscribe);
                assert_eq!(current.handle(), 6);
                assert_eq!(current.payload(), &[0x01, 0x00]);
            }
            other => panic!("unexpected effect: {other:?}"),
        }
    }

    #[test]
    fn r10_ble_runtime_poll_pending_queue_effect_emits_single_due_poll() {
        let mut runtime = R10BleRemoteRuntime::reader_remote(3500);
        runtime.handles = R10BleGattHandles::new(1, 8, 3, 5, 6);
        runtime.complete_remote_start(10_000);

        match runtime.poll_pending_queue_effect(10_000) {
            R10BleRuntimeEffect::PendingWriteQueue(queue) => {
                assert_eq!(queue.len(), 1);
                let current = queue.current().unwrap();
                assert_eq!(current.phase, R10BleWritePhase::Poll);
                assert_eq!(current.handle(), 3);
                assert_eq!(
                    current.payload(),
                    &[0x02, 0x05, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x07]
                );
            }
            other => panic!("unexpected effect: {other:?}"),
        }

        assert_eq!(
            runtime.poll_pending_queue_effect(10_999),
            R10BleRuntimeEffect::None
        );
    }

    #[test]
    fn r10_ble_runtime_shutdown_pending_queue_effect_emits_stop_then_unsubscribe() {
        let mut runtime = R10BleRemoteRuntime::reader_remote(3500);
        runtime.handles = R10BleGattHandles::new(1, 8, 3, 5, 6);
        runtime.complete_remote_start(10_000);

        match runtime.shutdown_pending_queue_effect() {
            R10BleRuntimeEffect::PendingWriteQueue(mut queue) => {
                assert_eq!(queue.len(), 2);
                assert_eq!(queue.current().unwrap().phase, R10BleWritePhase::Shutdown);
                assert_eq!(queue.current().unwrap().handle(), 3);
                assert_eq!(queue.advance().unwrap().handle(), 6);
                assert_eq!(queue.advance(), None);
            }
            other => panic!("unexpected effect: {other:?}"),
        }

        assert_eq!(runtime.session.state, R10BleTransportState::Disconnecting);
    }

    #[test]
    fn r10_ble_runtime_startup_pending_writes_queue_subscribe_then_start() {
        let mut runtime = R10BleRemoteRuntime::reader_remote(3500);
        runtime.handles = R10BleGattHandles::new(1, 8, 3, 5, 6);

        let writes = runtime.startup_pending_writes().unwrap();

        assert_eq!(runtime.session.state, R10BleTransportState::StartingRemote);

        assert_eq!(writes[0].phase, R10BleWritePhase::Subscribe);
        assert_eq!(writes[0].handle(), 6);
        assert_eq!(writes[0].payload(), &[0x01, 0x00]);

        assert_eq!(writes[1].phase, R10BleWritePhase::RemoteStart);
        assert_eq!(writes[1].handle(), 6);
        assert_eq!(writes[1].payload(), &[0x01, 0x00]);

        assert_eq!(writes[2].phase, R10BleWritePhase::RemoteStart);
        assert_eq!(writes[2].handle(), 3);
        assert_eq!(
            writes[2].payload(),
            &[0x02, 0x04, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x06]
        );
    }

    #[test]
    fn r10_ble_runtime_startup_pending_writes_effect_emits_three_step_queue() {
        let mut runtime = R10BleRemoteRuntime::reader_remote(3500);
        runtime.handles = R10BleGattHandles::new(1, 8, 3, 5, 6);

        match runtime.startup_pending_writes_effect() {
            R10BleRuntimeEffect::PendingStartupWrites(writes) => {
                assert_eq!(writes[0].phase, R10BleWritePhase::Subscribe);
                assert_eq!(writes[1].phase, R10BleWritePhase::RemoteStart);
                assert_eq!(writes[2].phase, R10BleWritePhase::RemoteStart);
            }
            other => panic!("unexpected effect: {other:?}"),
        }
    }

    #[test]
    fn r10_ble_runtime_startup_pending_writes_stays_none_until_handles_complete() {
        let mut runtime = R10BleRemoteRuntime::reader_remote(3500);
        runtime.handles = R10BleGattHandles {
            notify_cccd_handle: None,
            ..R10BleGattHandles::new(1, 8, 3, 5, 6)
        };

        assert_eq!(runtime.startup_pending_writes(), None);
        assert_eq!(
            runtime.startup_pending_writes_effect(),
            R10BleRuntimeEffect::None
        );
        assert_eq!(runtime.session.state, R10BleTransportState::Idle);
    }

    #[test]
    fn r10_ble_runtime_shutdown_pending_writes_helper_matches_effect_queue() {
        let mut runtime = R10BleRemoteRuntime::reader_remote(3500);
        runtime.handles = R10BleGattHandles::new(1, 8, 3, 5, 6);
        runtime.complete_remote_start(10_000);

        let writes = runtime.shutdown_pending_writes().unwrap();

        assert_eq!(runtime.session.state, R10BleTransportState::Disconnecting);
        assert_eq!(writes[0].phase, R10BleWritePhase::Shutdown);
        assert_eq!(writes[0].handle(), 3);
        assert_eq!(
            writes[0].payload(),
            &[0x02, 0x06, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x08]
        );
        assert_eq!(writes[1].phase, R10BleWritePhase::Shutdown);
        assert_eq!(writes[1].handle(), 6);
        assert_eq!(writes[1].payload(), &[0x00, 0x00]);
    }

    #[test]
    fn r10_ble_pending_write_exposes_phase_handle_payload_and_mode() {
        let write = R10BleGattWrite {
            handle: 6,
            payload: R10BleGattWritePayload::Cccd(R10_BLE_NOTIFY_CCCD_ENABLE),
            mode: R10BleGattWriteMode::WithResponse,
        };
        let pending = R10BlePendingWrite::new(R10BleWritePhase::Subscribe, write);

        assert_eq!(pending.phase, R10BleWritePhase::Subscribe);
        assert_eq!(pending.handle(), 6);
        assert_eq!(pending.payload(), &[0x01, 0x00]);
        assert_eq!(pending.write_mode(), R10BleGattWriteMode::WithResponse);
    }

    #[test]
    fn r10_ble_runtime_pending_subscribe_write_carries_subscribe_phase() {
        let mut runtime = R10BleRemoteRuntime::reader_remote(3500);
        runtime.handles = R10BleGattHandles::new(1, 8, 3, 5, 6);

        match runtime.subscribe_pending_write_effect() {
            R10BleRuntimeEffect::PendingWrite(pending) => {
                assert_eq!(pending.phase, R10BleWritePhase::Subscribe);
                assert_eq!(pending.handle(), 6);
                assert_eq!(pending.payload(), &[0x01, 0x00]);
                assert_eq!(pending.write_mode(), R10BleGattWriteMode::WithResponse);
            }
            other => panic!("unexpected effect: {other:?}"),
        }
    }

    #[test]
    fn r10_ble_runtime_pending_remote_start_writes_carry_remote_start_phase() {
        let mut runtime = R10BleRemoteRuntime::reader_remote(3500);
        runtime.handles = R10BleGattHandles::new(1, 8, 3, 5, 6);
        runtime.complete_subscribe();

        match runtime.remote_start_pending_writes_effect() {
            R10BleRuntimeEffect::PendingWrites(writes) => {
                assert_eq!(writes[0].phase, R10BleWritePhase::RemoteStart);
                assert_eq!(writes[0].handle(), 6);
                assert_eq!(writes[0].payload(), &[0x01, 0x00]);

                assert_eq!(writes[1].phase, R10BleWritePhase::RemoteStart);
                assert_eq!(writes[1].handle(), 3);
                assert_eq!(
                    writes[1].payload(),
                    &[0x02, 0x04, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x06]
                );
            }
            other => panic!("unexpected effect: {other:?}"),
        }
    }

    #[test]
    fn r10_ble_runtime_pending_poll_write_carries_poll_phase_only_when_due() {
        let mut runtime = R10BleRemoteRuntime::reader_remote(3500);
        runtime.handles = R10BleGattHandles::new(1, 8, 3, 5, 6);
        runtime.complete_remote_start(10_000);

        match runtime.poll_tick_pending_write_effect(10_000) {
            R10BleRuntimeEffect::PendingWrite(pending) => {
                assert_eq!(pending.phase, R10BleWritePhase::Poll);
                assert_eq!(pending.handle(), 3);
                assert_eq!(
                    pending.payload(),
                    &[0x02, 0x05, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x07]
                );
            }
            other => panic!("unexpected effect: {other:?}"),
        }

        assert_eq!(
            runtime.poll_tick_pending_write_effect(10_999),
            R10BleRuntimeEffect::None
        );
    }

    #[test]
    fn r10_ble_runtime_pending_shutdown_writes_carry_shutdown_phase() {
        let mut runtime = R10BleRemoteRuntime::reader_remote(3500);
        runtime.handles = R10BleGattHandles::new(1, 8, 3, 5, 6);
        runtime.complete_remote_start(10_000);

        match runtime.shutdown_pending_writes_effect() {
            R10BleRuntimeEffect::PendingWrites(writes) => {
                assert_eq!(writes[0].phase, R10BleWritePhase::Shutdown);
                assert_eq!(writes[0].handle(), 3);
                assert_eq!(
                    writes[0].payload(),
                    &[0x02, 0x06, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x08]
                );

                assert_eq!(writes[1].phase, R10BleWritePhase::Shutdown);
                assert_eq!(writes[1].handle(), 6);
                assert_eq!(writes[1].payload(), &[0x00, 0x00]);
            }
            other => panic!("unexpected effect: {other:?}"),
        }
    }

    #[test]
    fn r10_ble_runtime_write_success_subscribe_moves_to_starting_remote() {
        let mut runtime = R10BleRemoteRuntime::reader_remote(3500);
        runtime
            .session
            .begin_subscribe(&R10BleGattHandles::new(1, 8, 3, 5, 6));

        assert_eq!(
            runtime.write_success_effect(R10BleWritePhase::Subscribe, 10_000),
            R10BleRuntimeEffect::WriteResult(R10BleWriteResult::Success(
                R10BleWritePhase::Subscribe
            ))
        );
        assert_eq!(runtime.session.state, R10BleTransportState::StartingRemote);
    }

    #[test]
    fn r10_ble_runtime_write_success_remote_start_enters_polling() {
        let mut runtime = R10BleRemoteRuntime::reader_remote(3500);
        runtime
            .session
            .begin_remote_start(&R10BleGattHandles::new(1, 8, 3, 5, 6));

        assert_eq!(
            runtime.write_success_effect(R10BleWritePhase::RemoteStart, 10_000),
            R10BleRuntimeEffect::WriteResult(R10BleWriteResult::Success(
                R10BleWritePhase::RemoteStart
            ))
        );
        assert_eq!(runtime.session.state, R10BleTransportState::Polling);
        assert_eq!(runtime.session.next_poll_due_ms, 10_000);
    }

    #[test]
    fn r10_ble_runtime_write_success_poll_preserves_polling_state() {
        let mut runtime = R10BleRemoteRuntime::reader_remote(3500);
        runtime.handles = R10BleGattHandles::new(1, 8, 3, 5, 6);
        runtime.complete_remote_start(10_000);
        assert!(matches!(
            runtime.poll_tick_effect(10_000),
            R10BleRuntimeEffect::Write(_)
        ));

        assert_eq!(
            runtime.write_success_effect(R10BleWritePhase::Poll, 10_001),
            R10BleRuntimeEffect::WriteResult(R10BleWriteResult::Success(R10BleWritePhase::Poll))
        );
        assert_eq!(runtime.session.state, R10BleTransportState::Polling);
        assert_eq!(runtime.session.next_poll_due_ms, 11_000);
    }

    #[test]
    fn r10_ble_runtime_write_success_shutdown_returns_idle() {
        let mut runtime = R10BleRemoteRuntime::reader_remote(3500);
        runtime.handles = R10BleGattHandles::new(1, 8, 3, 5, 6);
        runtime.complete_remote_start(10_000);
        assert!(matches!(
            runtime.shutdown_effect(),
            R10BleRuntimeEffect::Writes(_)
        ));

        assert_eq!(
            runtime.write_success_effect(R10BleWritePhase::Shutdown, 20_000),
            R10BleRuntimeEffect::WriteResult(R10BleWriteResult::Success(
                R10BleWritePhase::Shutdown
            ))
        );
        assert_eq!(runtime.session.state, R10BleTransportState::Idle);
        assert_eq!(runtime.session.next_poll_due_ms, 0);
    }

    #[test]
    fn r10_ble_runtime_write_failure_clears_handles_and_enters_backoff() {
        let mut runtime = R10BleRemoteRuntime::reader_remote(3500);
        runtime.handles = R10BleGattHandles::new(1, 8, 3, 5, 6);
        runtime.complete_remote_start(10_000);

        assert_eq!(
            runtime.write_failed_effect(R10BleWritePhase::Poll, 20_000),
            R10BleRuntimeEffect::WriteResult(R10BleWriteResult::Failed(R10BleWritePhase::Poll))
        );
        assert_eq!(runtime.handles, R10BleGattHandles::unresolved());
        assert_eq!(runtime.session.state, R10BleTransportState::Backoff);
        assert_eq!(runtime.session.next_poll_due_ms, 23_000);
    }

    #[test]
    fn r10_ble_runtime_connect_success_clears_handles_and_enters_discovery() {
        let mut runtime = R10BleRemoteRuntime::reader_remote(3500);
        runtime.handles = R10BleGattHandles::new(1, 8, 3, 5, 6);
        runtime.session.begin_connect();

        assert_eq!(
            runtime.connect_success_effect(),
            R10BleRuntimeEffect::Connection(R10BleConnectionResult::Connected)
        );
        assert_eq!(runtime.handles, R10BleGattHandles::unresolved());
        assert_eq!(runtime.session.state, R10BleTransportState::Discovering);
    }

    #[test]
    fn r10_ble_runtime_connect_failed_clears_handles_and_enters_backoff() {
        let mut runtime = R10BleRemoteRuntime::reader_remote(3500);
        runtime.handles = R10BleGattHandles::new(1, 8, 3, 5, 6);
        runtime.session.begin_connect();

        assert_eq!(
            runtime.connect_failed_effect(10_000),
            R10BleRuntimeEffect::Connection(R10BleConnectionResult::ConnectFailed)
        );
        assert_eq!(runtime.handles, R10BleGattHandles::unresolved());
        assert_eq!(runtime.session.state, R10BleTransportState::Backoff);
        assert_eq!(runtime.session.next_poll_due_ms, 13_000);
    }

    #[test]
    fn r10_ble_runtime_link_lost_clears_handles_and_enters_backoff() {
        let mut runtime = R10BleRemoteRuntime::reader_remote(3500);
        runtime.handles = R10BleGattHandles::new(1, 8, 3, 5, 6);
        runtime.complete_remote_start(10_000);

        assert_eq!(
            runtime.link_lost_effect(20_000),
            R10BleRuntimeEffect::Connection(R10BleConnectionResult::LinkLost)
        );
        assert_eq!(runtime.handles, R10BleGattHandles::unresolved());
        assert_eq!(runtime.session.state, R10BleTransportState::Backoff);
        assert_eq!(runtime.session.next_poll_due_ms, 23_000);
    }

    #[test]
    fn r10_ble_runtime_link_lost_retry_returns_to_scanning() {
        let mut runtime = R10BleRemoteRuntime::reader_remote(3500);

        runtime.link_lost_effect(20_000);

        assert_eq!(
            runtime.backoff_tick_effect(22_999),
            R10BleRuntimeEffect::None
        );
        assert_eq!(runtime.session.state, R10BleTransportState::Backoff);

        assert_eq!(
            runtime.backoff_tick_effect(23_000),
            R10BleRuntimeEffect::BackoffReady
        );
        assert_eq!(runtime.session.state, R10BleTransportState::Scanning);
    }

    #[test]
    fn r10_ble_scan_target_matches_exact_default_address_case_insensitive() {
        let target = R10BleScanTarget::default_r10();

        assert_eq!(
            target.scan_device(R10BleAdvertisedDevice {
                address: Some("31:39:46:36:e5:05"),
                name: Some("Unknown"),
            }),
            R10BleScanDecision::MatchByAddress
        );
    }

    #[test]
    fn r10_ble_scan_target_matches_colmi_r10_name_prefix() {
        let target = R10BleScanTarget::any_colmi_r10();

        assert_eq!(
            target.scan_device(R10BleAdvertisedDevice {
                address: Some("AA:BB:CC:DD:EE:FF"),
                name: Some(R10_BLE_DEFAULT_ADVERTISED_NAME),
            }),
            R10BleScanDecision::MatchByNamePrefix
        );
    }

    #[test]
    fn r10_ble_scan_target_ignores_unrelated_device() {
        let target = R10BleScanTarget::default_r10();

        assert_eq!(
            target.scan_device(R10BleAdvertisedDevice {
                address: Some("AA:BB:CC:DD:EE:FF"),
                name: Some("Keyboard"),
            }),
            R10BleScanDecision::Ignore
        );
    }

    #[test]
    fn r10_ble_scan_decision_match_flag_is_explicit() {
        assert!(!R10BleScanDecision::Ignore.is_match());
        assert!(R10BleScanDecision::MatchByAddress.is_match());
        assert!(R10BleScanDecision::MatchByNamePrefix.is_match());
    }

    #[test]
    fn r10_ble_runtime_scan_effect_moves_to_connecting_on_match() {
        let mut runtime = R10BleRemoteRuntime::reader_remote(3500);

        assert_eq!(
            runtime.scan_device_effect(
                R10BleScanTarget::default_r10(),
                R10BleAdvertisedDevice {
                    address: Some(R10_BLE_DEFAULT_TARGET_ADDRESS),
                    name: Some("Other"),
                },
            ),
            R10BleRuntimeEffect::Scan(R10BleScanDecision::MatchByAddress)
        );
        assert_eq!(runtime.session.state, R10BleTransportState::Connecting);
    }

    #[test]
    fn r10_ble_runtime_scan_effect_keeps_scanning_on_ignore() {
        let mut runtime = R10BleRemoteRuntime::reader_remote(3500);
        runtime.session.begin_scan();

        assert_eq!(
            runtime.scan_device_effect(
                R10BleScanTarget::default_r10(),
                R10BleAdvertisedDevice {
                    address: Some("AA:BB:CC:DD:EE:FF"),
                    name: Some("Keyboard"),
                },
            ),
            R10BleRuntimeEffect::Scan(R10BleScanDecision::Ignore)
        );
        assert_eq!(runtime.session.state, R10BleTransportState::Scanning);
    }

    #[test]
    fn r10_ble_runtime_effect_accumulates_discovery_until_ready() {
        let mut runtime = R10BleRemoteRuntime::reader_remote(3500);

        assert_eq!(
            runtime.discovery_event_effect(R10BleGattDiscoveryEvent::Service {
                start_handle: 1,
                end_handle: 8,
            }),
            R10BleRuntimeEffect::Discovery(R10BleGattDiscoveryStatus::Incomplete(runtime.handles))
        );

        runtime.discovery_event_effect(R10BleGattDiscoveryEvent::WriteCharacteristic {
            value_handle: 3,
        });
        runtime.discovery_event_effect(R10BleGattDiscoveryEvent::NotifyCharacteristic {
            value_handle: 5,
        });

        assert_eq!(
            runtime.discovery_event_effect(R10BleGattDiscoveryEvent::NotifyCccd { handle: 6 }),
            R10BleRuntimeEffect::Discovery(R10BleGattDiscoveryStatus::Ready(runtime.handles))
        );
    }

    #[test]
    fn r10_ble_runtime_effect_subscribe_and_start_emit_ordered_writes() {
        let mut runtime = R10BleRemoteRuntime::reader_remote(3500);
        runtime.handles = R10BleGattHandles::new(1, 8, 3, 5, 6);

        assert_eq!(
            runtime.subscribe_effect(),
            R10BleRuntimeEffect::Write(R10BleGattWrite {
                handle: 6,
                payload: R10BleGattWritePayload::Cccd(R10_BLE_NOTIFY_CCCD_ENABLE),
                mode: R10BleGattWriteMode::WithResponse,
            })
        );

        runtime.complete_subscribe();

        match runtime.remote_start_effect() {
            R10BleRuntimeEffect::Writes(writes) => {
                assert_eq!(writes[0].handle, 6);
                assert_eq!(writes[0].payload(), &[0x01, 0x00]);
                assert_eq!(writes[1].handle, 3);
                assert_eq!(
                    writes[1].payload(),
                    &[0x02, 0x04, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x06]
                );
            }
            other => panic!("unexpected effect: {other:?}"),
        }
    }

    #[test]
    fn r10_ble_runtime_effect_poll_tick_emits_only_when_due() {
        let mut runtime = R10BleRemoteRuntime::reader_remote(3500);
        runtime.handles = R10BleGattHandles::new(1, 8, 3, 5, 6);
        runtime.complete_remote_start(10_000);

        match runtime.poll_tick_effect(10_000) {
            R10BleRuntimeEffect::Write(write) => {
                assert_eq!(write.handle, 3);
                assert_eq!(
                    write.payload(),
                    &[0x02, 0x05, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x07]
                );
            }
            other => panic!("unexpected effect: {other:?}"),
        }

        assert_eq!(runtime.poll_tick_effect(10_999), R10BleRuntimeEffect::None);
    }

    #[test]
    fn r10_ble_runtime_effect_notify_uses_policy_bridge() {
        let mut runtime = R10BleRemoteRuntime::reader_remote(3500);
        runtime.handles = R10BleGattHandles::new(1, 8, 3, 5, 6);

        assert_eq!(
            runtime.notify_effect(5, &MOTION, 10_000),
            R10BleRuntimeEffect::Notify(R10BleGattNotifyPolicyResult::Accepted(
                R10RemoteAction::Reader(RustmixReaderAction::NextPage)
            ))
        );

        assert_eq!(
            runtime.notify_effect(7, &MOTION, 11_000),
            R10BleRuntimeEffect::Notify(R10BleGattNotifyPolicyResult::Rejected(
                R10BleGattNotifyGate::WrongHandle {
                    expected: Some(5),
                    actual: 7,
                }
            ))
        );
    }

    #[test]
    fn r10_ble_runtime_effect_shutdown_emits_stop_then_unsubscribe() {
        let mut runtime = R10BleRemoteRuntime::reader_remote(3500);
        runtime.handles = R10BleGattHandles::new(1, 8, 3, 5, 6);
        runtime.complete_remote_start(10_000);

        match runtime.shutdown_effect() {
            R10BleRuntimeEffect::Writes(writes) => {
                assert_eq!(writes[0].handle, 3);
                assert_eq!(
                    writes[0].payload(),
                    &[0x02, 0x06, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x08]
                );
                assert_eq!(writes[1].handle, 6);
                assert_eq!(writes[1].payload(), &[0x00, 0x00]);
            }
            other => panic!("unexpected effect: {other:?}"),
        }
    }

    #[test]
    fn r10_ble_runtime_effect_disconnect_and_backoff_retry() {
        let mut runtime = R10BleRemoteRuntime::reader_remote(3500);
        runtime.handles = R10BleGattHandles::new(1, 8, 3, 5, 6);

        assert_eq!(runtime.disconnect_effect(10_000), R10BleRuntimeEffect::None);
        assert_eq!(runtime.handles, R10BleGattHandles::unresolved());
        assert_eq!(runtime.session.state, R10BleTransportState::Backoff);

        assert_eq!(
            runtime.backoff_tick_effect(12_999),
            R10BleRuntimeEffect::None
        );
        assert_eq!(runtime.session.state, R10BleTransportState::Backoff);

        assert_eq!(
            runtime.backoff_tick_effect(13_000),
            R10BleRuntimeEffect::BackoffReady
        );
        assert_eq!(runtime.session.state, R10BleTransportState::Scanning);
    }

    #[test]
    fn r10_ble_runtime_reader_remote_starts_idle_with_unresolved_handles() {
        let runtime = R10BleRemoteRuntime::reader_remote(3500);

        assert_eq!(runtime.session.state, R10BleTransportState::Idle);
        assert_eq!(runtime.handles, R10BleGattHandles::unresolved());
    }

    #[test]
    fn r10_ble_runtime_accumulates_discovery_and_completes_ready_state() {
        let mut runtime = R10BleRemoteRuntime::reader_remote(3500);

        runtime.session.begin_discovery();
        assert_eq!(
            runtime.apply_discovery_event(R10BleGattDiscoveryEvent::Service {
                start_handle: 1,
                end_handle: 8,
            }),
            R10BleGattDiscoveryStatus::Incomplete(runtime.handles)
        );
        runtime.apply_discovery_event(R10BleGattDiscoveryEvent::WriteCharacteristic {
            value_handle: 3,
        });
        runtime.apply_discovery_event(R10BleGattDiscoveryEvent::NotifyCharacteristic {
            value_handle: 5,
        });
        let status =
            runtime.apply_discovery_event(R10BleGattDiscoveryEvent::NotifyCccd { handle: 6 });

        assert_eq!(status, R10BleGattDiscoveryStatus::Ready(runtime.handles));
        assert_eq!(runtime.complete_discovery(), status);
        assert_eq!(runtime.session.state, R10BleTransportState::Subscribing);
    }

    #[test]
    fn r10_ble_runtime_plans_subscribe_start_and_poll_from_stored_handles() {
        let mut runtime = R10BleRemoteRuntime::reader_remote(3500);
        runtime.handles = R10BleGattHandles::new(1, 8, 3, 5, 6);

        let subscribe = runtime.begin_subscribe().unwrap();
        assert_eq!(subscribe.handle, 6);
        assert_eq!(subscribe.payload(), &[0x01, 0x00]);
        assert_eq!(runtime.session.state, R10BleTransportState::Subscribing);

        runtime.complete_subscribe();
        let startup = runtime.begin_remote_start().unwrap();
        assert_eq!(startup[0].payload(), &[0x01, 0x00]);
        assert_eq!(
            startup[1].payload(),
            &[0x02, 0x04, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x06]
        );

        runtime.complete_remote_start(10_000);
        let poll = runtime.poll_due_write(10_000).unwrap();
        assert_eq!(poll.handle, 3);
        assert_eq!(
            poll.payload(),
            &[0x02, 0x05, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x07]
        );
    }

    #[test]
    fn r10_ble_runtime_routes_notify_through_stored_handles() {
        let mut runtime = R10BleRemoteRuntime::reader_remote(3500);
        runtime.handles = R10BleGattHandles::new(1, 8, 3, 5, 6);

        assert_eq!(
            runtime.on_gatt_notify(5, &MOTION, 10_000),
            R10BleGattNotifyPolicyResult::Accepted(R10RemoteAction::Reader(
                RustmixReaderAction::NextPage
            ))
        );
    }

    #[test]
    fn r10_ble_runtime_plans_shutdown_from_stored_handles() {
        let mut runtime = R10BleRemoteRuntime::reader_remote(3500);
        runtime.handles = R10BleGattHandles::new(1, 8, 3, 5, 6);

        runtime.complete_remote_start(10_000);
        let shutdown = runtime.begin_remote_shutdown().unwrap();

        assert_eq!(runtime.session.state, R10BleTransportState::Disconnecting);
        assert_eq!(
            shutdown[0].payload(),
            &[0x02, 0x06, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x08]
        );
        assert_eq!(shutdown[1].payload(), &[0x00, 0x00]);

        runtime.complete_remote_shutdown();
        assert_eq!(runtime.session.state, R10BleTransportState::Idle);
        assert_eq!(runtime.session.next_poll_due_ms, 0);
    }

    #[test]
    fn r10_ble_runtime_disconnect_clears_handles_and_retries_after_backoff() {
        let mut runtime = R10BleRemoteRuntime::reader_remote(3500);
        runtime.handles = R10BleGattHandles::new(1, 8, 3, 5, 6);

        runtime.disconnect_and_backoff(10_000);

        assert_eq!(runtime.handles, R10BleGattHandles::unresolved());
        assert_eq!(runtime.session.state, R10BleTransportState::Backoff);
        assert_eq!(runtime.session.next_poll_due_ms, 13_000);
        assert!(!runtime.retry_after_backoff(12_999));
        assert_eq!(runtime.session.state, R10BleTransportState::Backoff);

        assert!(runtime.retry_after_backoff(13_000));
        assert_eq!(runtime.session.state, R10BleTransportState::Scanning);
        assert_eq!(runtime.session.next_poll_due_ms, 0);
    }

    #[test]
    fn r10_ble_gatt_discovery_status_reports_ready_only_when_complete() {
        let complete = R10BleGattHandles::new(1, 8, 3, 5, 6);
        assert_eq!(
            complete.discovery_status(),
            R10BleGattDiscoveryStatus::Ready(complete)
        );

        let incomplete = R10BleGattHandles {
            write_value_handle: None,
            ..complete
        };
        assert_eq!(
            incomplete.discovery_status(),
            R10BleGattDiscoveryStatus::Incomplete(incomplete)
        );
    }

    #[test]
    fn r10_ble_gatt_discovery_status_exposes_handles_and_ready_flag() {
        let handles = R10BleGattHandles::new(1, 8, 3, 5, 6);

        let ready = R10BleGattDiscoveryStatus::Ready(handles);
        assert!(ready.is_ready());
        assert_eq!(ready.handles(), handles);

        let incomplete = R10BleGattDiscoveryStatus::Incomplete(R10BleGattHandles::unresolved());
        assert!(!incomplete.is_ready());
        assert_eq!(incomplete.handles(), R10BleGattHandles::unresolved());
    }

    #[test]
    fn r10_ble_session_complete_discovery_moves_to_subscribing_when_ready() {
        let handles = R10BleGattHandles::new(1, 8, 3, 5, 6);
        let mut session = R10BleRemoteSession::reader_remote(3500);

        session.begin_discovery();
        let status = session.complete_discovery(handles);

        assert_eq!(status, R10BleGattDiscoveryStatus::Ready(handles));
        assert_eq!(session.state, R10BleTransportState::Subscribing);
    }

    #[test]
    fn r10_ble_session_complete_discovery_keeps_discovering_when_incomplete() {
        let handles = R10BleGattHandles {
            notify_value_handle: None,
            ..R10BleGattHandles::new(1, 8, 3, 5, 6)
        };
        let mut session = R10BleRemoteSession::reader_remote(3500);

        session.begin_discovery();
        let status = session.complete_discovery(handles);

        assert_eq!(status, R10BleGattDiscoveryStatus::Incomplete(handles));
        assert_eq!(session.state, R10BleTransportState::Discovering);
    }

    #[test]
    fn r10_ble_session_begin_scan_marks_scanning_and_clears_timer() {
        let mut session = R10BleRemoteSession::reader_remote(3500);

        session.start_polling(10_000);
        session.begin_scan();

        assert_eq!(session.state, R10BleTransportState::Scanning);
        assert_eq!(session.next_poll_due_ms, 0);
    }

    #[test]
    fn r10_ble_session_connection_state_steps_are_explicit() {
        let mut session = R10BleRemoteSession::reader_remote(3500);

        session.begin_scan();
        assert_eq!(session.state, R10BleTransportState::Scanning);

        session.begin_connect();
        assert_eq!(session.state, R10BleTransportState::Connecting);

        session.begin_discovery();
        assert_eq!(session.state, R10BleTransportState::Discovering);
    }

    #[test]
    fn r10_ble_session_begin_subscribe_returns_cccd_enable_write() {
        let handles = R10BleGattHandles::new(1, 8, 3, 5, 6);
        let mut session = R10BleRemoteSession::reader_remote(3500);

        session.begin_discovery();
        let write = session.begin_subscribe(&handles).unwrap();

        assert_eq!(session.state, R10BleTransportState::Subscribing);
        assert_eq!(write.handle, 6);
        assert_eq!(write.payload(), &[0x01, 0x00]);
        assert_eq!(write.write_mode(), R10BleGattWriteMode::WithResponse);
    }

    #[test]
    fn r10_ble_session_begin_subscribe_keeps_state_when_cccd_missing() {
        let handles = R10BleGattHandles {
            notify_cccd_handle: None,
            ..R10BleGattHandles::new(1, 8, 3, 5, 6)
        };
        let mut session = R10BleRemoteSession::reader_remote(3500);

        session.begin_discovery();

        assert_eq!(session.begin_subscribe(&handles), None);
        assert_eq!(session.state, R10BleTransportState::Discovering);
    }

    #[test]
    fn r10_ble_session_complete_subscribe_marks_starting_remote() {
        let mut session = R10BleRemoteSession::reader_remote(3500);

        session.begin_scan();
        session.complete_subscribe();

        assert_eq!(session.state, R10BleTransportState::StartingRemote);
    }

    #[test]
    fn r10_ble_session_retry_after_backoff_waits_until_due_then_scans() {
        let mut session = R10BleRemoteSession::reader_remote(3500);

        session.enter_backoff(10_000);

        assert_eq!(session.state, R10BleTransportState::Backoff);
        assert_eq!(session.next_poll_due_ms, 13_000);
        assert!(!session.backoff_due(12_999));
        assert!(!session.retry_after_backoff(12_999));
        assert_eq!(session.state, R10BleTransportState::Backoff);

        assert!(session.backoff_due(13_000));
        assert!(session.retry_after_backoff(13_000));
        assert_eq!(session.state, R10BleTransportState::Scanning);
        assert_eq!(session.next_poll_due_ms, 0);
    }

    #[test]
    fn r10_ble_session_begin_remote_start_returns_startup_writes_and_marks_starting() {
        let handles = R10BleGattHandles::new(1, 8, 3, 5, 6);
        let mut session = R10BleRemoteSession::reader_remote(3500);

        let writes = session.begin_remote_start(&handles).unwrap();

        assert_eq!(session.state, R10BleTransportState::StartingRemote);
        assert_eq!(writes[0].handle, 6);
        assert_eq!(writes[0].payload(), &[0x01, 0x00]);
        assert_eq!(writes[1].handle, 3);
        assert_eq!(
            writes[1].payload(),
            &[0x02, 0x04, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x06]
        );
    }

    #[test]
    fn r10_ble_session_begin_remote_start_keeps_state_when_handles_incomplete() {
        let handles = R10BleGattHandles {
            notify_cccd_handle: None,
            ..R10BleGattHandles::new(1, 8, 3, 5, 6)
        };
        let mut session = R10BleRemoteSession::reader_remote(3500);

        assert_eq!(session.begin_remote_start(&handles), None);
        assert_eq!(session.state, R10BleTransportState::Idle);
        assert_eq!(session.next_poll_due_ms, 0);
    }

    #[test]
    fn r10_ble_session_complete_remote_start_enters_polling_immediately() {
        let mut session = R10BleRemoteSession::reader_remote(3500);

        session.complete_remote_start(10_000);

        assert_eq!(session.state, R10BleTransportState::Polling);
        assert_eq!(session.next_poll_due_ms, 10_000);
    }

    #[test]
    fn r10_ble_session_begin_remote_shutdown_returns_stop_then_unsubscribe() {
        let handles = R10BleGattHandles::new(1, 8, 3, 5, 6);
        let mut session = R10BleRemoteSession::reader_remote(3500);

        session.start_polling(10_000);
        let writes = session.begin_remote_shutdown(&handles).unwrap();

        assert_eq!(session.state, R10BleTransportState::Disconnecting);
        assert_eq!(writes[0].handle, 3);
        assert_eq!(
            writes[0].payload(),
            &[0x02, 0x06, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x08]
        );
        assert_eq!(writes[1].handle, 6);
        assert_eq!(writes[1].payload(), &[0x00, 0x00]);
    }

    #[test]
    fn r10_ble_session_complete_remote_shutdown_returns_idle_and_clears_timer() {
        let mut session = R10BleRemoteSession::reader_remote(3500);

        session.start_polling(10_000);
        session.complete_remote_shutdown();

        assert_eq!(session.state, R10BleTransportState::Idle);
        assert_eq!(session.next_poll_due_ms, 0);
    }

    #[test]
    fn r10_ble_session_poll_due_write_requires_polling_state() {
        let handles = R10BleGattHandles::new(1, 8, 3, 5, 6);
        let mut session = R10BleRemoteSession::reader_remote(3500);

        assert_eq!(session.poll_due_write(&handles, 10_000), None);
    }

    #[test]
    fn r10_ble_session_poll_due_write_serializes_due_poll_and_advances_timer() {
        let handles = R10BleGattHandles::new(1, 8, 3, 5, 6);
        let mut session = R10BleRemoteSession::reader_remote(3500);

        session.start_polling(10_000);

        let write = session.poll_due_write(&handles, 10_000).unwrap();
        assert_eq!(write.handle, 3);
        assert_eq!(
            write.payload(),
            &[0x02, 0x05, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x07]
        );
        assert_eq!(session.next_poll_due_ms, 11_000);
    }

    #[test]
    fn r10_ble_session_poll_due_write_waits_until_next_due_time() {
        let handles = R10BleGattHandles::new(1, 8, 3, 5, 6);
        let mut session = R10BleRemoteSession::reader_remote(3500);

        session.start_polling(10_000);
        assert!(session.poll_due_write(&handles, 10_000).is_some());

        assert_eq!(session.poll_due_write(&handles, 10_999), None);
        assert!(session.poll_due_write(&handles, 11_000).is_some());
        assert_eq!(session.next_poll_due_ms, 12_000);
    }

    #[test]
    fn r10_ble_session_poll_due_write_does_not_advance_when_handles_incomplete() {
        let handles = R10BleGattHandles {
            notify_cccd_handle: None,
            ..R10BleGattHandles::new(1, 8, 3, 5, 6)
        };
        let mut session = R10BleRemoteSession::reader_remote(3500);

        session.start_polling(10_000);

        assert_eq!(session.poll_due_write(&handles, 10_000), None);
        assert_eq!(session.next_poll_due_ms, 10_000);
    }

    #[test]
    fn r10_ble_gatt_notify_bridge_accepts_motion_into_reader_action() {
        let handles = R10BleGattHandles::new(1, 8, 3, 5, 6);
        let mut session = R10BleRemoteSession::reader_remote(3500);

        assert_eq!(
            session.on_gatt_notify(&handles, 5, &MOTION, 10_000),
            R10BleGattNotifyPolicyResult::Accepted(R10RemoteAction::Reader(
                RustmixReaderAction::NextPage
            ))
        );
    }

    #[test]
    fn r10_ble_gatt_notify_bridge_preserves_policy_none_for_debounce() {
        let handles = R10BleGattHandles::new(1, 8, 3, 5, 6);
        let mut session = R10BleRemoteSession::reader_remote(3500);

        assert_eq!(
            session.on_gatt_notify(&handles, 5, &MOTION, 10_000),
            R10BleGattNotifyPolicyResult::Accepted(R10RemoteAction::Reader(
                RustmixReaderAction::NextPage
            ))
        );
        assert_eq!(
            session.on_gatt_notify(&handles, 5, &MOTION, 11_000),
            R10BleGattNotifyPolicyResult::Accepted(R10RemoteAction::None)
        );
    }

    #[test]
    fn r10_ble_gatt_notify_bridge_rejects_wrong_handle_before_policy() {
        let handles = R10BleGattHandles::new(1, 8, 3, 5, 6);
        let mut session = R10BleRemoteSession::reader_remote(3500);

        assert_eq!(
            session.on_gatt_notify(&handles, 7, &MOTION, 10_000),
            R10BleGattNotifyPolicyResult::Rejected(R10BleGattNotifyGate::WrongHandle {
                expected: Some(5),
                actual: 7,
            })
        );
    }

    #[test]
    fn r10_ble_gatt_notify_bridge_rejects_wrong_length_before_policy() {
        let handles = R10BleGattHandles::new(1, 8, 3, 5, 6);
        let mut session = R10BleRemoteSession::reader_remote(3500);

        assert_eq!(
            session.on_gatt_notify(&handles, 5, &[0x02, 0x02, 0x04], 10_000),
            R10BleGattNotifyPolicyResult::Rejected(R10BleGattNotifyGate::WrongLength { actual: 3 })
        );
    }

    #[test]
    fn r10_ble_gatt_notify_gate_accepts_matching_16_byte_notify() {
        let handles = R10BleGattHandles::new(1, 8, 3, 5, 6);

        assert_eq!(
            handles.gate_notify(5, &MOTION),
            R10BleGattNotifyGate::Accepted(MOTION)
        );
    }

    #[test]
    fn r10_ble_gatt_notify_gate_rejects_wrong_handle() {
        let handles = R10BleGattHandles::new(1, 8, 3, 5, 6);

        assert_eq!(
            handles.gate_notify(7, &MOTION),
            R10BleGattNotifyGate::WrongHandle {
                expected: Some(5),
                actual: 7,
            }
        );
    }

    #[test]
    fn r10_ble_gatt_notify_gate_rejects_unresolved_notify_handle() {
        let handles = R10BleGattHandles {
            notify_value_handle: None,
            ..R10BleGattHandles::new(1, 8, 3, 5, 6)
        };

        assert_eq!(
            handles.gate_notify(5, &MOTION),
            R10BleGattNotifyGate::WrongHandle {
                expected: None,
                actual: 5,
            }
        );
    }

    #[test]
    fn r10_ble_gatt_notify_gate_rejects_wrong_payload_length() {
        let handles = R10BleGattHandles::new(1, 8, 3, 5, 6);

        assert_eq!(
            handles.gate_notify(5, &[0x02, 0x02, 0x04]),
            R10BleGattNotifyGate::WrongLength { actual: 3 }
        );
    }

    #[test]
    fn r10_ble_gatt_write_modes_separate_cccd_from_remote_commands() {
        let cccd = R10BleGattOperation::WriteCccd {
            handle: 6,
            value: R10_BLE_NOTIFY_CCCD_ENABLE,
        }
        .to_write();

        let command = R10BleGattOperation::WriteRemoteCommand {
            handle: 3,
            command: R10BleCommand::PollRemote,
        }
        .to_write();

        assert_eq!(cccd.write_mode(), R10BleGattWriteMode::WithResponse);
        assert_eq!(command.write_mode(), R10BleGattWriteMode::WithoutResponse);
    }

    #[test]
    fn r10_ble_gatt_write_sequences_preserve_expected_write_modes() {
        let handles = R10BleGattHandles::new(1, 8, 3, 5, 6);

        let startup = handles.startup_writes().unwrap();
        assert_eq!(startup[0].write_mode(), R10BleGattWriteMode::WithResponse);
        assert_eq!(
            startup[1].write_mode(),
            R10BleGattWriteMode::WithoutResponse
        );

        let poll = handles.poll_write().unwrap();
        assert_eq!(poll.write_mode(), R10BleGattWriteMode::WithoutResponse);

        let shutdown = handles.shutdown_writes().unwrap();
        assert_eq!(
            shutdown[0].write_mode(),
            R10BleGattWriteMode::WithoutResponse
        );
        assert_eq!(shutdown[1].write_mode(), R10BleGattWriteMode::WithResponse);
    }

    #[test]
    fn r10_ble_gatt_startup_writes_are_serialized_in_order() {
        let handles = R10BleGattHandles::new(1, 8, 3, 5, 6);
        let writes = handles.startup_writes().unwrap();

        assert_eq!(writes[0].handle, 6);
        assert_eq!(writes[0].payload(), &[0x01, 0x00]);

        assert_eq!(writes[1].handle, 3);
        assert_eq!(
            writes[1].payload(),
            &[0x02, 0x04, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x06]
        );
    }

    #[test]
    fn r10_ble_gatt_poll_write_uses_poll_packet() {
        let handles = R10BleGattHandles::new(1, 8, 3, 5, 6);
        let write = handles.poll_write().unwrap();

        assert_eq!(write.handle, 3);
        assert_eq!(
            write.payload(),
            &[0x02, 0x05, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x07]
        );
    }

    #[test]
    fn r10_ble_gatt_shutdown_writes_stop_before_unsubscribe() {
        let handles = R10BleGattHandles::new(1, 8, 3, 5, 6);
        let writes = handles.shutdown_writes().unwrap();

        assert_eq!(writes[0].handle, 3);
        assert_eq!(
            writes[0].payload(),
            &[0x02, 0x06, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x08]
        );

        assert_eq!(writes[1].handle, 6);
        assert_eq!(writes[1].payload(), &[0x00, 0x00]);
    }

    #[test]
    fn r10_ble_gatt_write_sequences_stay_none_until_handles_are_complete() {
        let incomplete = R10BleGattHandles {
            notify_cccd_handle: None,
            ..R10BleGattHandles::new(1, 8, 3, 5, 6)
        };

        assert_eq!(incomplete.startup_writes(), None);
        assert_eq!(incomplete.poll_write(), None);
        assert!(incomplete.stop_write().is_some());
        assert_eq!(incomplete.shutdown_writes(), None);
    }

    #[test]
    fn r10_ble_gatt_cccd_operation_serializes_to_write_payload() {
        let op = R10BleGattOperation::WriteCccd {
            handle: 6,
            value: R10_BLE_NOTIFY_CCCD_ENABLE,
        };
        let write = op.to_write();

        assert_eq!(write.handle, 6);
        assert_eq!(write.payload_len(), 2);
        assert_eq!(write.payload(), &[0x01, 0x00]);
    }

    #[test]
    fn r10_ble_gatt_remote_command_operation_serializes_to_stock_packet() {
        let op = R10BleGattOperation::WriteRemoteCommand {
            handle: 3,
            command: R10BleCommand::StartRemote,
        };
        let write = op.to_write();

        assert_eq!(write.handle, 3);
        assert_eq!(write.payload_len(), 16);
        assert_eq!(
            write.payload(),
            &[0x02, 0x04, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x06]
        );
    }

    #[test]
    fn r10_ble_gatt_startup_operations_serialize_in_execution_order() {
        let handles = R10BleGattHandles::new(1, 8, 3, 5, 6);
        let ops = handles.startup_operations().unwrap();
        let writes = [ops[0].to_write(), ops[1].to_write()];

        assert_eq!(writes[0].handle, 6);
        assert_eq!(writes[0].payload(), &[0x01, 0x00]);
        assert_eq!(writes[1].handle, 3);
        assert_eq!(
            writes[1].payload(),
            &[0x02, 0x04, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x06]
        );
    }

    #[test]
    fn r10_ble_gatt_poll_and_stop_operations_serialize_to_known_packets() {
        let handles = R10BleGattHandles::new(1, 8, 3, 5, 6);

        let poll = handles.poll_remote_operation().unwrap().to_write();
        assert_eq!(poll.handle, 3);
        assert_eq!(
            poll.payload(),
            &[0x02, 0x05, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x07]
        );

        let stop = handles.stop_remote_operation().unwrap().to_write();
        assert_eq!(stop.handle, 3);
        assert_eq!(
            stop.payload(),
            &[0x02, 0x06, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x08]
        );
    }

    #[test]
    fn r10_ble_gatt_operations_require_resolved_handles() {
        let unresolved = R10BleGattHandles::unresolved();

        assert_eq!(unresolved.enable_notify_operation(), None);
        assert_eq!(unresolved.start_remote_operation(), None);
        assert_eq!(unresolved.poll_remote_operation(), None);
        assert_eq!(unresolved.stop_remote_operation(), None);
        assert_eq!(unresolved.startup_operations(), None);
    }

    #[test]
    fn r10_ble_gatt_operations_map_to_cccd_and_write_handles() {
        let handles = R10BleGattHandles::new(1, 8, 3, 5, 6);

        assert_eq!(
            handles.enable_notify_operation(),
            Some(R10BleGattOperation::WriteCccd {
                handle: 6,
                value: R10_BLE_NOTIFY_CCCD_ENABLE,
            })
        );
        assert_eq!(
            handles.disable_notify_operation(),
            Some(R10BleGattOperation::WriteCccd {
                handle: 6,
                value: R10_BLE_NOTIFY_CCCD_DISABLE,
            })
        );
        assert_eq!(
            handles.start_remote_operation(),
            Some(R10BleGattOperation::WriteRemoteCommand {
                handle: 3,
                command: R10BleCommand::StartRemote,
            })
        );
        assert_eq!(
            handles.poll_remote_operation(),
            Some(R10BleGattOperation::WriteRemoteCommand {
                handle: 3,
                command: R10BleCommand::PollRemote,
            })
        );
        assert_eq!(
            handles.stop_remote_operation(),
            Some(R10BleGattOperation::WriteRemoteCommand {
                handle: 3,
                command: R10BleCommand::StopRemote,
            })
        );
    }

    #[test]
    fn r10_ble_gatt_startup_operations_subscribe_before_start() {
        let handles = R10BleGattHandles::new(1, 8, 3, 5, 6);

        assert_eq!(
            handles.startup_operations(),
            Some([
                R10BleGattOperation::WriteCccd {
                    handle: 6,
                    value: R10_BLE_NOTIFY_CCCD_ENABLE,
                },
                R10BleGattOperation::WriteRemoteCommand {
                    handle: 3,
                    command: R10BleCommand::StartRemote,
                },
            ])
        );
    }

    #[test]
    fn r10_ble_gatt_start_operation_requires_complete_remote_contract() {
        let missing_notify_cccd = R10BleGattHandles {
            notify_cccd_handle: None,
            ..R10BleGattHandles::new(1, 8, 3, 5, 6)
        };

        assert_eq!(
            missing_notify_cccd.remote_command_operation(R10BleCommand::StartRemote),
            Some(R10BleGattOperation::WriteRemoteCommand {
                handle: 3,
                command: R10BleCommand::StartRemote,
            })
        );
        assert_eq!(missing_notify_cccd.start_remote_operation(), None);
        assert_eq!(missing_notify_cccd.poll_remote_operation(), None);
        assert_eq!(missing_notify_cccd.startup_operations(), None);
        assert_eq!(missing_notify_cccd.stop_remote_operation().is_some(), true);
    }

    #[test]
    fn r10_ble_gatt_discovery_events_accumulate_complete_handles() {
        let mut handles = R10BleGattHandles::unresolved();

        handles.apply_discovery_event(R10BleGattDiscoveryEvent::Service {
            start_handle: 1,
            end_handle: 8,
        });
        handles.apply_discovery_event(R10BleGattDiscoveryEvent::WriteCharacteristic {
            value_handle: 3,
        });
        handles.apply_discovery_event(R10BleGattDiscoveryEvent::NotifyCharacteristic {
            value_handle: 5,
        });
        handles.apply_discovery_event(R10BleGattDiscoveryEvent::NotifyCccd { handle: 6 });

        assert_eq!(handles, R10BleGattHandles::new(1, 8, 3, 5, 6));
        assert!(handles.can_start_remote_mode());
    }

    #[test]
    fn r10_ble_gatt_discovery_event_builder_keeps_incomplete_state_safe() {
        let handles = R10BleGattHandles::unresolved()
            .with_discovery_event(R10BleGattDiscoveryEvent::Service {
                start_handle: 1,
                end_handle: 8,
            })
            .with_discovery_event(R10BleGattDiscoveryEvent::WriteCharacteristic {
                value_handle: 3,
            });

        assert!(handles.service_range_valid());
        assert!(handles.write_handle_ready());
        assert!(!handles.notify_handle_ready());
        assert!(!handles.notify_cccd_ready());
        assert!(!handles.is_complete());
        assert!(!handles.can_start_remote_mode());
    }

    #[test]
    fn r10_ble_gatt_discovery_rejects_handles_outside_service_range() {
        let handles = R10BleGattHandles::unresolved()
            .with_discovery_event(R10BleGattDiscoveryEvent::Service {
                start_handle: 10,
                end_handle: 20,
            })
            .with_discovery_event(R10BleGattDiscoveryEvent::WriteCharacteristic { value_handle: 3 })
            .with_discovery_event(R10BleGattDiscoveryEvent::NotifyCharacteristic {
                value_handle: 15,
            })
            .with_discovery_event(R10BleGattDiscoveryEvent::NotifyCccd { handle: 16 });

        assert!(!handles.write_handle_ready());
        assert!(handles.notify_handle_ready());
        assert!(handles.notify_cccd_ready());
        assert!(!handles.is_complete());
    }

    #[test]
    fn r10_ble_gatt_handles_start_unresolved() {
        let handles = R10BleGattHandles::unresolved();

        assert!(!handles.service_range_valid());
        assert!(!handles.is_complete());
        assert!(!handles.can_start_remote_mode());
    }

    #[test]
    fn r10_ble_gatt_handles_require_service_range_and_characteristics() {
        let handles = R10BleGattHandles::new(1, 8, 3, 5, 6);

        assert!(handles.service_range_valid());
        assert!(handles.write_handle_ready());
        assert!(handles.notify_handle_ready());
        assert!(handles.notify_cccd_ready());
        assert!(handles.is_complete());
        assert!(handles.can_start_remote_mode());
    }

    #[test]
    fn r10_ble_gatt_handles_reject_missing_or_out_of_range_handles() {
        let missing_cccd = R10BleGattHandles {
            notify_cccd_handle: None,
            ..R10BleGattHandles::new(1, 8, 3, 5, 6)
        };
        assert!(!missing_cccd.is_complete());
        assert!(!missing_cccd.can_start_remote_mode());

        let outside_service = R10BleGattHandles::new(1, 8, 3, 5, 12);
        assert!(!outside_service.notify_cccd_ready());
        assert!(!outside_service.is_complete());

        let inverted_service = R10BleGattHandles::new(8, 1, 3, 5, 6);
        assert!(!inverted_service.service_range_valid());
        assert!(!inverted_service.is_complete());
    }

    #[test]
    fn r10_ble_notify_cccd_payloads_match_gatt_subscription_values() {
        assert_eq!(R10_BLE_NOTIFY_CCCD_ENABLE, [0x01, 0x00]);
        assert_eq!(R10_BLE_NOTIFY_CCCD_DISABLE, [0x00, 0x00]);
    }

    #[test]
    fn r10_ble_uuid_contract_matches_stock_gatt() {
        assert_eq!(R10_BLE_SERVICE_UUID, "6e40fff0-b5a3-f393-e0a9-e50e24dcca9e");
        assert_eq!(R10_BLE_WRITE_UUID, "6e400002-b5a3-f393-e0a9-e50e24dcca9e");
        assert_eq!(R10_BLE_NOTIFY_UUID, "6e400003-b5a3-f393-e0a9-e50e24dcca9e");
    }

    #[test]
    fn r10_ble_commands_use_known_stock_packets() {
        assert_eq!(
            R10BleCommand::StartRemote.packet(),
            &[0x02, 0x04, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x06]
        );
        assert_eq!(
            R10BleCommand::PollRemote.packet(),
            &[0x02, 0x05, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x07]
        );
        assert_eq!(
            R10BleCommand::StopRemote.packet(),
            &[0x02, 0x06, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x08]
        );
    }

    #[test]
    fn r10_ble_notify_uses_debounced_policy() {
        let mut session = R10BleRemoteSession::reader_remote(3500);
        assert_eq!(
            session.on_notify(10_000, &MOTION),
            R10RemoteAction::Reader(RustmixReaderAction::NextPage)
        );
        assert_eq!(session.on_notify(11_000, &MOTION), R10RemoteAction::None);
    }

    #[test]
    fn r10_ble_poll_timer_is_explicit() {
        let mut session = R10BleRemoteSession::reader_remote(3500);
        assert!(!session.should_poll(10_000));

        session.start_polling(10_000);
        assert!(session.should_poll(10_000));

        session.mark_poll_sent(10_000);
        assert!(!session.should_poll(10_999));
        assert!(session.should_poll(11_000));
    }
}
