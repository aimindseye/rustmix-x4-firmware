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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R10BleGattDiscoveryEvent {
    Service { start_handle: u16, end_handle: u16 },
    WriteCharacteristic { value_handle: u16 },
    NotifyCharacteristic { value_handle: u16 },
    NotifyCccd { handle: u16 },
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rustmix_x4::contracts::input_semantics::RustmixReaderAction;
    use crate::rustmix_x4::ring_remote::r10_remote_policy::R10RemoteAction;

    const MOTION: [u8; 16] = [0x02, 0x02, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x04];

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
