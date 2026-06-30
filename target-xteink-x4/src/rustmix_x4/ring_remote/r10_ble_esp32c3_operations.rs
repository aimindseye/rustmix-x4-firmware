// ESP32-C3 ProbeOnly backend operation queue for the COLMI R10 BLE remote.
//
// This module is hardware-neutral and host-testable. It converts the r4u
// backend task state into concrete BLE host operations that a later target-only
// esp-radio / trouble-host runner can execute.

use super::r10_ble_esp32c3_backend::{
    R10BleEsp32c3ProbeBackendEvent, R10BleEsp32c3ProbeBackendOutput,
    R10BleEsp32c3ProbeBackendState, R10BleEsp32c3ProbeBackendStep, R10BleEsp32c3ProbeBackendTask,
    r10_ble_esp32c3_probe_backend_plan,
};
use super::r10_ble_transport::{
    R10_BLE_LIVE_NOTIFY_CCCD_HANDLE, R10_BLE_LIVE_NOTIFY_VALUE_HANDLE,
    R10_BLE_LIVE_WRITE_VALUE_HANDLE, R10BleCommand,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R10BleEsp32c3ProbeOperation {
    InitRadio,
    BuildController,
    BuildHostStack,
    StartScan,
    ConnectTarget,
    DiscoverGatt,
    SubscribeCccd {
        cccd_handle: u16,
    },
    WriteRemoteStart {
        value_handle: u16,
        payload: &'static [u8],
    },
    WritePoll {
        value_handle: u16,
        payload: &'static [u8],
    },
    AwaitNotify {
        notify_handle: u16,
    },
    Backoff,
}

impl R10BleEsp32c3ProbeOperation {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::InitRadio => "init_radio",
            Self::BuildController => "build_controller",
            Self::BuildHostStack => "build_host_stack",
            Self::StartScan => "start_scan",
            Self::ConnectTarget => "connect_target",
            Self::DiscoverGatt => "discover_gatt",
            Self::SubscribeCccd { .. } => "subscribe_cccd",
            Self::WriteRemoteStart { .. } => "write_remote_start",
            Self::WritePoll { .. } => "write_poll",
            Self::AwaitNotify { .. } => "await_notify",
            Self::Backoff => "backoff",
        }
    }

    pub const fn is_write(&self) -> bool {
        matches!(
            self,
            Self::SubscribeCccd { .. } | Self::WriteRemoteStart { .. } | Self::WritePoll { .. }
        )
    }

    pub const fn is_reader_input_enabled(&self) -> bool {
        false
    }

    pub const fn expected_handle(&self) -> Option<u16> {
        match self {
            Self::SubscribeCccd { cccd_handle } => Some(*cccd_handle),
            Self::WriteRemoteStart { value_handle, .. } => Some(*value_handle),
            Self::WritePoll { value_handle, .. } => Some(*value_handle),
            Self::AwaitNotify { notify_handle } => Some(*notify_handle),
            _ => None,
        }
    }

    pub const fn payload(&self) -> Option<&'static [u8]> {
        match self {
            Self::WriteRemoteStart { payload, .. } => Some(*payload),
            Self::WritePoll { payload, .. } => Some(*payload),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R10BleEsp32c3ProbeOperationRecord {
    pub event: &'static str,
    pub target: &'static str,
    pub operation: &'static str,
    pub state: &'static str,
    pub handle: &'static str,
    pub payload: &'static str,
    pub reader: &'static str,
    pub status: &'static str,
}

impl R10BleEsp32c3ProbeOperationRecord {
    pub const fn from_operation(
        operation: R10BleEsp32c3ProbeOperation,
        state: R10BleEsp32c3ProbeBackendState,
    ) -> Self {
        Self {
            event: "ble_operation",
            target: "esp_c",
            operation: operation.as_str(),
            state: state.as_str(),
            handle: operation_handle_label(operation),
            payload: operation_payload_label(operation),
            reader: "reader_off",
            status: "ready",
        }
    }

    pub fn is_monitor_safe(&self) -> bool {
        super::r10_ble_device_task::r10_ble_device_task_monitor_label_is_safe(self.event)
            && super::r10_ble_device_task::r10_ble_device_task_monitor_label_is_safe(self.target)
            && super::r10_ble_device_task::r10_ble_device_task_monitor_label_is_safe(self.operation)
            && super::r10_ble_device_task::r10_ble_device_task_monitor_label_is_safe(self.state)
            && super::r10_ble_device_task::r10_ble_device_task_monitor_label_is_safe(self.handle)
            && super::r10_ble_device_task::r10_ble_device_task_monitor_label_is_safe(self.payload)
            && super::r10_ble_device_task::r10_ble_device_task_monitor_label_is_safe(self.reader)
            && super::r10_ble_device_task::r10_ble_device_task_monitor_label_is_safe(self.status)
    }
}

pub const fn operation_handle_label(operation: R10BleEsp32c3ProbeOperation) -> &'static str {
    match operation {
        R10BleEsp32c3ProbeOperation::SubscribeCccd { .. } => "notify_cccd",
        R10BleEsp32c3ProbeOperation::WriteRemoteStart { .. } => "write_value",
        R10BleEsp32c3ProbeOperation::WritePoll { .. } => "write_value",
        R10BleEsp32c3ProbeOperation::AwaitNotify { .. } => "notify_value",
        _ => "none",
    }
}

pub const fn operation_payload_label(operation: R10BleEsp32c3ProbeOperation) -> &'static str {
    match operation {
        R10BleEsp32c3ProbeOperation::SubscribeCccd { .. } => "cccd_enable",
        R10BleEsp32c3ProbeOperation::WriteRemoteStart { .. } => "remote_start",
        R10BleEsp32c3ProbeOperation::WritePoll { .. } => "poll",
        _ => "none",
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct R10BleEsp32c3ProbeOperationQueue<const N: usize> {
    items: [Option<R10BleEsp32c3ProbeOperation>; N],
    len: usize,
    cursor: usize,
    dropped: usize,
}

impl<const N: usize> R10BleEsp32c3ProbeOperationQueue<N> {
    pub const fn new() -> Self {
        Self {
            items: [None; N],
            len: 0,
            cursor: 0,
            dropped: 0,
        }
    }

    pub const fn len(&self) -> usize {
        self.len
    }

    pub const fn dropped(&self) -> usize {
        self.dropped
    }

    pub const fn remaining(&self) -> usize {
        self.len.saturating_sub(self.cursor)
    }

    pub const fn is_empty(&self) -> bool {
        self.remaining() == 0
    }

    pub fn clear(&mut self) {
        self.items = [None; N];
        self.len = 0;
        self.cursor = 0;
        self.dropped = 0;
    }

    pub fn push(&mut self, operation: R10BleEsp32c3ProbeOperation) -> bool {
        if self.len < N {
            self.items[self.len] = Some(operation);
            self.len += 1;
            true
        } else {
            self.dropped += 1;
            false
        }
    }

    pub fn peek(&self) -> Option<R10BleEsp32c3ProbeOperation> {
        if self.cursor < self.len {
            self.items[self.cursor]
        } else {
            None
        }
    }

    pub fn advance(&mut self) -> Option<R10BleEsp32c3ProbeOperation> {
        let current = self.peek();

        if current.is_some() {
            self.cursor += 1;
        }

        current
    }

    pub fn operation_at(&self, index: usize) -> Option<R10BleEsp32c3ProbeOperation> {
        if index < self.len {
            self.items[index]
        } else {
            None
        }
    }

    pub fn record_at(
        &self,
        index: usize,
        state: R10BleEsp32c3ProbeBackendState,
    ) -> Option<R10BleEsp32c3ProbeOperationRecord> {
        self.operation_at(index)
            .map(|operation| R10BleEsp32c3ProbeOperationRecord::from_operation(operation, state))
    }
}

impl Default for R10BleEsp32c3ProbeOperationQueue<12> {
    fn default() -> Self {
        Self::new()
    }
}

pub fn r10_ble_esp32c3_probe_initial_operation_queue<const N: usize>()
-> R10BleEsp32c3ProbeOperationQueue<N> {
    let mut queue = R10BleEsp32c3ProbeOperationQueue::new();

    let _ = queue.push(R10BleEsp32c3ProbeOperation::InitRadio);
    let _ = queue.push(R10BleEsp32c3ProbeOperation::BuildController);
    let _ = queue.push(R10BleEsp32c3ProbeOperation::BuildHostStack);
    let _ = queue.push(R10BleEsp32c3ProbeOperation::StartScan);

    queue
}

pub fn r10_ble_esp32c3_probe_session_operation_queue<const N: usize>()
-> R10BleEsp32c3ProbeOperationQueue<N> {
    let mut queue = r10_ble_esp32c3_probe_initial_operation_queue();

    let _ = queue.push(R10BleEsp32c3ProbeOperation::ConnectTarget);
    let _ = queue.push(R10BleEsp32c3ProbeOperation::DiscoverGatt);
    let _ = queue.push(R10BleEsp32c3ProbeOperation::SubscribeCccd {
        cccd_handle: R10_BLE_LIVE_NOTIFY_CCCD_HANDLE,
    });
    let _ = queue.push(R10BleEsp32c3ProbeOperation::WriteRemoteStart {
        value_handle: R10_BLE_LIVE_WRITE_VALUE_HANDLE,
        payload: R10BleCommand::StartRemote.packet(),
    });
    let _ = queue.push(R10BleEsp32c3ProbeOperation::WritePoll {
        value_handle: R10_BLE_LIVE_WRITE_VALUE_HANDLE,
        payload: R10BleCommand::PollRemote.packet(),
    });
    let _ = queue.push(R10BleEsp32c3ProbeOperation::AwaitNotify {
        notify_handle: R10_BLE_LIVE_NOTIFY_VALUE_HANDLE,
    });

    queue
}

pub fn r10_ble_esp32c3_probe_operation_for_state(
    state: R10BleEsp32c3ProbeBackendState,
) -> Option<R10BleEsp32c3ProbeOperation> {
    match state {
        R10BleEsp32c3ProbeBackendState::Created => Some(R10BleEsp32c3ProbeOperation::InitRadio),
        R10BleEsp32c3ProbeBackendState::AdapterReady => {
            Some(R10BleEsp32c3ProbeOperation::StartScan)
        }
        R10BleEsp32c3ProbeBackendState::Scanning => Some(R10BleEsp32c3ProbeOperation::StartScan),
        R10BleEsp32c3ProbeBackendState::Connecting => {
            Some(R10BleEsp32c3ProbeOperation::ConnectTarget)
        }
        R10BleEsp32c3ProbeBackendState::Discovering => {
            Some(R10BleEsp32c3ProbeOperation::DiscoverGatt)
        }
        R10BleEsp32c3ProbeBackendState::Subscribing => {
            Some(R10BleEsp32c3ProbeOperation::SubscribeCccd {
                cccd_handle: R10_BLE_LIVE_NOTIFY_CCCD_HANDLE,
            })
        }
        R10BleEsp32c3ProbeBackendState::StartingRemote => {
            Some(R10BleEsp32c3ProbeOperation::WriteRemoteStart {
                value_handle: R10_BLE_LIVE_WRITE_VALUE_HANDLE,
                payload: R10BleCommand::StartRemote.packet(),
            })
        }
        R10BleEsp32c3ProbeBackendState::Polling => Some(R10BleEsp32c3ProbeOperation::WritePoll {
            value_handle: R10_BLE_LIVE_WRITE_VALUE_HANDLE,
            payload: R10BleCommand::PollRemote.packet(),
        }),
        R10BleEsp32c3ProbeBackendState::Notified => {
            Some(R10BleEsp32c3ProbeOperation::AwaitNotify {
                notify_handle: R10_BLE_LIVE_NOTIFY_VALUE_HANDLE,
            })
        }
        R10BleEsp32c3ProbeBackendState::Backoff => Some(R10BleEsp32c3ProbeOperation::Backoff),
        R10BleEsp32c3ProbeBackendState::Completed => None,
    }
}

pub fn r10_ble_esp32c3_probe_operation_after_output(
    output: R10BleEsp32c3ProbeBackendOutput,
) -> Option<R10BleEsp32c3ProbeOperation> {
    r10_ble_esp32c3_probe_operation_for_state(output.state)
}

pub fn r10_ble_esp32c3_probe_step_operation(
    step: R10BleEsp32c3ProbeBackendStep,
) -> R10BleEsp32c3ProbeOperation {
    match step {
        R10BleEsp32c3ProbeBackendStep::InitRadio => R10BleEsp32c3ProbeOperation::InitRadio,
        R10BleEsp32c3ProbeBackendStep::BuildController => {
            R10BleEsp32c3ProbeOperation::BuildController
        }
        R10BleEsp32c3ProbeBackendStep::BuildHostStack => {
            R10BleEsp32c3ProbeOperation::BuildHostStack
        }
        R10BleEsp32c3ProbeBackendStep::StartScan => R10BleEsp32c3ProbeOperation::StartScan,
        R10BleEsp32c3ProbeBackendStep::ConnectMatched => R10BleEsp32c3ProbeOperation::ConnectTarget,
        R10BleEsp32c3ProbeBackendStep::DiscoverGatt => R10BleEsp32c3ProbeOperation::DiscoverGatt,
        R10BleEsp32c3ProbeBackendStep::SubscribeNotify => {
            R10BleEsp32c3ProbeOperation::SubscribeCccd {
                cccd_handle: R10_BLE_LIVE_NOTIFY_CCCD_HANDLE,
            }
        }
        R10BleEsp32c3ProbeBackendStep::WriteRemoteStart => {
            R10BleEsp32c3ProbeOperation::WriteRemoteStart {
                value_handle: R10_BLE_LIVE_WRITE_VALUE_HANDLE,
                payload: R10BleCommand::StartRemote.packet(),
            }
        }
        R10BleEsp32c3ProbeBackendStep::PollAndNotify => R10BleEsp32c3ProbeOperation::AwaitNotify {
            notify_handle: R10_BLE_LIVE_NOTIFY_VALUE_HANDLE,
        },
    }
}

pub fn r10_ble_esp32c3_probe_plan_operation_queue<const N: usize>()
-> R10BleEsp32c3ProbeOperationQueue<N> {
    let mut queue = R10BleEsp32c3ProbeOperationQueue::new();

    for step in r10_ble_esp32c3_probe_backend_plan() {
        let _ = queue.push(r10_ble_esp32c3_probe_step_operation(*step));
    }

    queue
}

#[cfg(all(target_arch = "riscv32", feature = "r10-ble-host"))]
pub fn r10_ble_x4_emit_esp32c3_probe_operation_queue_startup_log() {
    let queue = r10_ble_esp32c3_probe_session_operation_queue::<12>();

    esp_println::println!(
        "rustmix event=ble_operation_queue target=esp32c3 mode=probe_only reader=reader_off status=ready len={}",
        queue.len()
    );

    let mut index = 0;

    while index < queue.len() {
        if let Some(record) = queue.record_at(index, R10BleEsp32c3ProbeBackendState::Created) {
            esp_println::println!(
                "rustmix event={} target={} operation={} state={} handle={} payload={} reader={} status={}",
                record.event,
                record.target,
                record.operation,
                record.state,
                record.handle,
                record.payload,
                record.reader,
                record.status
            );
        }

        index += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rustmix_x4::ring_remote::r10_ble_transport::{
        R10_BLE_LIVE_NOTIFY_CCCD_HANDLE, R10_BLE_LIVE_NOTIFY_VALUE_HANDLE,
        R10_BLE_LIVE_WRITE_VALUE_HANDLE, R10BleCommand,
    };

    #[test]
    fn r10_ble_esp32c3_probe_operations_have_stable_safe_labels() {
        let queue = r10_ble_esp32c3_probe_session_operation_queue::<12>();

        assert_eq!(queue.len(), 10);

        let mut index = 0;

        while index < queue.len() {
            let operation = queue.operation_at(index).unwrap();

            assert!(
                super::super::r10_ble_device_task::r10_ble_device_task_monitor_label_is_safe(
                    operation.as_str()
                )
            );
            assert!(!operation.is_reader_input_enabled());

            index += 1;
        }
    }

    #[test]
    fn r10_ble_esp32c3_probe_initial_queue_starts_radio_and_scan() {
        let queue = r10_ble_esp32c3_probe_initial_operation_queue::<4>();

        assert_eq!(queue.len(), 4);
        assert_eq!(
            queue.operation_at(0),
            Some(R10BleEsp32c3ProbeOperation::InitRadio)
        );
        assert_eq!(
            queue.operation_at(1),
            Some(R10BleEsp32c3ProbeOperation::BuildController)
        );
        assert_eq!(
            queue.operation_at(2),
            Some(R10BleEsp32c3ProbeOperation::BuildHostStack)
        );
        assert_eq!(
            queue.operation_at(3),
            Some(R10BleEsp32c3ProbeOperation::StartScan)
        );
    }

    #[test]
    fn r10_ble_esp32c3_probe_session_queue_covers_notify() {
        let queue = r10_ble_esp32c3_probe_session_operation_queue::<12>();

        assert_eq!(
            queue.operation_at(4),
            Some(R10BleEsp32c3ProbeOperation::ConnectTarget)
        );
        assert_eq!(
            queue.operation_at(5),
            Some(R10BleEsp32c3ProbeOperation::DiscoverGatt)
        );
        assert_eq!(
            queue.operation_at(9),
            Some(R10BleEsp32c3ProbeOperation::AwaitNotify {
                notify_handle: R10_BLE_LIVE_NOTIFY_VALUE_HANDLE
            })
        );
        assert_eq!(queue.remaining(), 10);
    }

    #[test]
    fn r10_ble_esp32c3_probe_operation_queue_tracks_cursor_and_overflow() {
        let mut queue = R10BleEsp32c3ProbeOperationQueue::<2>::new();

        assert!(queue.push(R10BleEsp32c3ProbeOperation::InitRadio));
        assert!(queue.push(R10BleEsp32c3ProbeOperation::StartScan));
        assert!(!queue.push(R10BleEsp32c3ProbeOperation::ConnectTarget));
        assert_eq!(queue.dropped(), 1);
        assert_eq!(queue.peek(), Some(R10BleEsp32c3ProbeOperation::InitRadio));
        assert_eq!(
            queue.advance(),
            Some(R10BleEsp32c3ProbeOperation::InitRadio)
        );
        assert_eq!(queue.peek(), Some(R10BleEsp32c3ProbeOperation::StartScan));
        assert_eq!(queue.remaining(), 1);
    }

    #[test]
    fn r10_ble_esp32c3_probe_write_operations_preserve_handles_and_payloads() {
        let subscribe = R10BleEsp32c3ProbeOperation::SubscribeCccd {
            cccd_handle: R10_BLE_LIVE_NOTIFY_CCCD_HANDLE,
        };
        let start = R10BleEsp32c3ProbeOperation::WriteRemoteStart {
            value_handle: R10_BLE_LIVE_WRITE_VALUE_HANDLE,
            payload: R10BleCommand::StartRemote.packet(),
        };
        let poll = R10BleEsp32c3ProbeOperation::WritePoll {
            value_handle: R10_BLE_LIVE_WRITE_VALUE_HANDLE,
            payload: R10BleCommand::PollRemote.packet(),
        };

        assert!(subscribe.is_write());
        assert!(start.is_write());
        assert!(poll.is_write());
        assert_eq!(
            subscribe.expected_handle(),
            Some(R10_BLE_LIVE_NOTIFY_CCCD_HANDLE)
        );
        assert_eq!(
            start.expected_handle(),
            Some(R10_BLE_LIVE_WRITE_VALUE_HANDLE)
        );
        assert_eq!(
            poll.expected_handle(),
            Some(R10_BLE_LIVE_WRITE_VALUE_HANDLE)
        );
        assert_eq!(
            start.payload(),
            Some(R10BleCommand::StartRemote.packet().as_slice())
        );
        assert_eq!(
            poll.payload(),
            Some(R10BleCommand::PollRemote.packet().as_slice())
        );
    }

    #[test]
    fn r10_ble_esp32c3_probe_operation_records_are_monitor_safe() {
        let queue = r10_ble_esp32c3_probe_session_operation_queue::<12>();

        let mut index = 0;

        while index < queue.len() {
            let record = queue
                .record_at(index, R10BleEsp32c3ProbeBackendState::Created)
                .unwrap();

            assert_eq!(record.event, "ble_operation");
            assert_eq!(record.target, "esp_c");
            assert_eq!(record.reader, "reader_off");
            assert!(record.is_monitor_safe());

            index += 1;
        }
    }

    #[test]
    fn r10_ble_esp32c3_probe_state_mapping_is_explicit() {
        assert_eq!(
            r10_ble_esp32c3_probe_operation_for_state(R10BleEsp32c3ProbeBackendState::Created),
            Some(R10BleEsp32c3ProbeOperation::InitRadio)
        );
        assert_eq!(
            r10_ble_esp32c3_probe_operation_for_state(R10BleEsp32c3ProbeBackendState::Connecting),
            Some(R10BleEsp32c3ProbeOperation::ConnectTarget)
        );
        assert_eq!(
            r10_ble_esp32c3_probe_operation_for_state(R10BleEsp32c3ProbeBackendState::Discovering),
            Some(R10BleEsp32c3ProbeOperation::DiscoverGatt)
        );
        assert_eq!(
            r10_ble_esp32c3_probe_operation_for_state(R10BleEsp32c3ProbeBackendState::Completed),
            None
        );
    }

    #[test]
    fn r10_ble_esp32c3_probe_write_state_mapping_preserves_gatt_contract() {
        assert_eq!(
            r10_ble_esp32c3_probe_operation_for_state(R10BleEsp32c3ProbeBackendState::Subscribing),
            Some(R10BleEsp32c3ProbeOperation::SubscribeCccd {
                cccd_handle: R10_BLE_LIVE_NOTIFY_CCCD_HANDLE
            })
        );
        assert_eq!(
            r10_ble_esp32c3_probe_operation_for_state(
                R10BleEsp32c3ProbeBackendState::StartingRemote
            ),
            Some(R10BleEsp32c3ProbeOperation::WriteRemoteStart {
                value_handle: R10_BLE_LIVE_WRITE_VALUE_HANDLE,
                payload: R10BleCommand::StartRemote.packet()
            })
        );
        assert_eq!(
            r10_ble_esp32c3_probe_operation_for_state(R10BleEsp32c3ProbeBackendState::Polling),
            Some(R10BleEsp32c3ProbeOperation::WritePoll {
                value_handle: R10_BLE_LIVE_WRITE_VALUE_HANDLE,
                payload: R10BleCommand::PollRemote.packet()
            })
        );
    }

    #[test]
    fn r10_ble_esp32c3_probe_operation_after_backend_output_tracks_next_operation() {
        let mut backend = R10BleEsp32c3ProbeBackendTask::<8>::new();

        let scan = backend.handle_event(R10BleEsp32c3ProbeBackendEvent::AdapterReady);
        assert_eq!(
            r10_ble_esp32c3_probe_operation_after_output(scan),
            Some(R10BleEsp32c3ProbeOperation::StartScan)
        );

        let matched = backend.on_default_r10_advertisement();
        assert_eq!(
            r10_ble_esp32c3_probe_operation_after_output(matched),
            Some(R10BleEsp32c3ProbeOperation::ConnectTarget)
        );
    }

    #[test]
    fn r10_ble_esp32c3_probe_step_mapping_matches_backend_plan() {
        let queue = r10_ble_esp32c3_probe_plan_operation_queue::<16>();

        assert_eq!(queue.len(), r10_ble_esp32c3_probe_backend_plan().len());
        assert_eq!(
            queue.operation_at(0),
            Some(R10BleEsp32c3ProbeOperation::InitRadio)
        );
        assert_eq!(
            queue.operation_at(3),
            Some(R10BleEsp32c3ProbeOperation::StartScan)
        );
        assert_eq!(
            queue.operation_at(7),
            Some(R10BleEsp32c3ProbeOperation::WriteRemoteStart {
                value_handle: R10_BLE_LIVE_WRITE_VALUE_HANDLE,
                payload: R10BleCommand::StartRemote.packet()
            })
        );
        assert_eq!(
            queue.operation_at(8),
            Some(R10BleEsp32c3ProbeOperation::AwaitNotify {
                notify_handle: R10_BLE_LIVE_NOTIFY_VALUE_HANDLE
            })
        );
    }

    #[test]
    fn r10_ble_esp32c3_probe_queue_clear_resets_state() {
        let mut queue = r10_ble_esp32c3_probe_session_operation_queue::<12>();

        assert!(!queue.is_empty());
        let _ = queue.advance();
        queue.clear();

        assert_eq!(queue.len(), 0);
        assert_eq!(queue.remaining(), 0);
        assert_eq!(queue.dropped(), 0);
        assert_eq!(queue.peek(), None);
    }

    #[test]
    fn r10_ble_esp32c3_probe_notify_operation_is_not_a_write() {
        let notify = R10BleEsp32c3ProbeOperation::AwaitNotify {
            notify_handle: R10_BLE_LIVE_NOTIFY_VALUE_HANDLE,
        };

        assert!(!notify.is_write());
        assert_eq!(
            notify.expected_handle(),
            Some(R10_BLE_LIVE_NOTIFY_VALUE_HANDLE)
        );
        assert_eq!(notify.payload(), None);
        assert_eq!(operation_handle_label(notify), "notify_value");
        assert_eq!(operation_payload_label(notify), "none");
    }

    #[test]
    fn r10_ble_esp32c3_probe_backoff_operation_is_reader_off() {
        let op = r10_ble_esp32c3_probe_operation_for_state(R10BleEsp32c3ProbeBackendState::Backoff)
            .unwrap();
        let record = R10BleEsp32c3ProbeOperationRecord::from_operation(
            op,
            R10BleEsp32c3ProbeBackendState::Backoff,
        );

        assert_eq!(op, R10BleEsp32c3ProbeOperation::Backoff);
        assert!(!op.is_reader_input_enabled());
        assert_eq!(record.reader, "reader_off");
        assert_eq!(record.status, "ready");
        assert!(record.is_monitor_safe());
    }
}
