// ESP32-C3 ProbeOnly runner boundary for the COLMI R10 BLE remote.
//
// This module consumes the r4v operation queue and converts operation results
// into the r4u backend callback API. It remains hardware-neutral and
// host-testable; a later target-only async BLE loop will execute the operations.

use super::r10_ble_esp32c3_backend::{
    R10BleEsp32c3ProbeBackendEvent, R10BleEsp32c3ProbeBackendOutput,
    R10BleEsp32c3ProbeBackendState, R10BleEsp32c3ProbeBackendTask,
};
use super::r10_ble_esp32c3_operations::{
    R10BleEsp32c3ProbeOperation, R10BleEsp32c3ProbeOperationQueue,
    r10_ble_esp32c3_probe_session_operation_queue,
};
use super::r10_ble_transport::R10_BLE_LIVE_NOTIFY_VALUE_HANDLE;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R10BleEsp32c3ProbeRunnerPhase {
    Created,
    RadioReady,
    ControllerReady,
    HostReady,
    Scanning,
    Matched,
    Connecting,
    Discovering,
    Subscribing,
    StartingRemote,
    Polling,
    AwaitingNotify,
    Notified,
    Backoff,
    Complete,
}

impl R10BleEsp32c3ProbeRunnerPhase {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Created => "created",
            Self::RadioReady => "radio_ready",
            Self::ControllerReady => "controller_ready",
            Self::HostReady => "host_ready",
            Self::Scanning => "scanning",
            Self::Matched => "matched",
            Self::Connecting => "connecting",
            Self::Discovering => "discovering",
            Self::Subscribing => "subscribing",
            Self::StartingRemote => "starting_remote",
            Self::Polling => "polling",
            Self::AwaitingNotify => "awaiting_notify",
            Self::Notified => "notified",
            Self::Backoff => "backoff",
            Self::Complete => "complete",
        }
    }

    pub const fn is_terminal(&self) -> bool {
        matches!(self, Self::Backoff | Self::Complete)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R10BleEsp32c3ProbeRunnerStatus {
    Ready,
    Pending,
    Ok,
    Ignored,
    Failed,
    Awaiting,
    Complete,
}

impl R10BleEsp32c3ProbeRunnerStatus {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::Pending => "pending",
            Self::Ok => "ok",
            Self::Ignored => "ignored",
            Self::Failed => "failed",
            Self::Awaiting => "awaiting",
            Self::Complete => "complete",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R10BleEsp32c3ProbeRunnerRecord {
    pub event: &'static str,
    pub target: &'static str,
    pub phase: &'static str,
    pub operation: &'static str,
    pub reader: &'static str,
    pub status: &'static str,
}

impl R10BleEsp32c3ProbeRunnerRecord {
    pub const fn new(
        phase: R10BleEsp32c3ProbeRunnerPhase,
        operation: Option<R10BleEsp32c3ProbeOperation>,
        status: R10BleEsp32c3ProbeRunnerStatus,
    ) -> Self {
        Self {
            event: "ble_runner",
            target: "esp_c",
            phase: phase.as_str(),
            operation: runner_operation_label(operation),
            reader: "reader_off",
            status: status.as_str(),
        }
    }

    pub fn is_monitor_safe(&self) -> bool {
        super::r10_ble_device_task::r10_ble_device_task_monitor_label_is_safe(self.event)
            && super::r10_ble_device_task::r10_ble_device_task_monitor_label_is_safe(self.target)
            && super::r10_ble_device_task::r10_ble_device_task_monitor_label_is_safe(self.phase)
            && super::r10_ble_device_task::r10_ble_device_task_monitor_label_is_safe(self.operation)
            && super::r10_ble_device_task::r10_ble_device_task_monitor_label_is_safe(self.reader)
            && super::r10_ble_device_task::r10_ble_device_task_monitor_label_is_safe(self.status)
    }
}

pub const fn runner_operation_label(
    operation: Option<R10BleEsp32c3ProbeOperation>,
) -> &'static str {
    match operation {
        Some(operation) => operation.as_str(),
        None => "none",
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R10BleEsp32c3ProbeRunnerResult {
    pub phase: R10BleEsp32c3ProbeRunnerPhase,
    pub operation: Option<R10BleEsp32c3ProbeOperation>,
    pub backend_output: Option<R10BleEsp32c3ProbeBackendOutput>,
    pub record: R10BleEsp32c3ProbeRunnerRecord,
    pub reader_input_enabled: bool,
}

impl R10BleEsp32c3ProbeRunnerResult {
    pub const fn is_reader_input_enabled(&self) -> bool {
        false
    }

    pub fn is_monitor_safe(&self) -> bool {
        self.record.is_monitor_safe()
            && self
                .backend_output
                .map(|output| output.is_monitor_safe())
                .unwrap_or(true)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct R10BleEsp32c3ProbeRunner {
    backend: R10BleEsp32c3ProbeBackendTask<16>,
    queue: R10BleEsp32c3ProbeOperationQueue<12>,
    phase: R10BleEsp32c3ProbeRunnerPhase,
    last_result: Option<R10BleEsp32c3ProbeRunnerResult>,
}

impl R10BleEsp32c3ProbeRunner {
    pub fn new() -> Self {
        Self {
            backend: R10BleEsp32c3ProbeBackendTask::new(),
            queue: r10_ble_esp32c3_probe_session_operation_queue(),
            phase: R10BleEsp32c3ProbeRunnerPhase::Created,
            last_result: None,
        }
    }

    pub const fn phase(&self) -> R10BleEsp32c3ProbeRunnerPhase {
        self.phase
    }

    pub fn backend(&self) -> &R10BleEsp32c3ProbeBackendTask<16> {
        &self.backend
    }

    pub fn current_operation(&self) -> Option<R10BleEsp32c3ProbeOperation> {
        self.queue.peek()
    }

    pub fn queue_remaining(&self) -> usize {
        self.queue.remaining()
    }

    pub const fn is_reader_input_enabled(&self) -> bool {
        false
    }

    pub fn last_result(&self) -> Option<R10BleEsp32c3ProbeRunnerResult> {
        self.last_result
    }

    pub fn startup_record(&self) -> R10BleEsp32c3ProbeRunnerRecord {
        R10BleEsp32c3ProbeRunnerRecord::new(
            self.phase,
            self.current_operation(),
            R10BleEsp32c3ProbeRunnerStatus::Ready,
        )
    }

    fn result(
        &mut self,
        operation: Option<R10BleEsp32c3ProbeOperation>,
        backend_output: Option<R10BleEsp32c3ProbeBackendOutput>,
        status: R10BleEsp32c3ProbeRunnerStatus,
    ) -> R10BleEsp32c3ProbeRunnerResult {
        let result = R10BleEsp32c3ProbeRunnerResult {
            phase: self.phase,
            operation,
            backend_output,
            record: R10BleEsp32c3ProbeRunnerRecord::new(self.phase, operation, status),
            reader_input_enabled: false,
        };

        self.last_result = Some(result);

        result
    }

    pub fn complete_current_success(&mut self, now_ms: u64) -> R10BleEsp32c3ProbeRunnerResult {
        let operation = self.current_operation();

        match operation {
            Some(R10BleEsp32c3ProbeOperation::InitRadio) => {
                let _ = self.queue.advance();
                self.phase = R10BleEsp32c3ProbeRunnerPhase::RadioReady;
                self.result(operation, None, R10BleEsp32c3ProbeRunnerStatus::Ok)
            }
            Some(R10BleEsp32c3ProbeOperation::BuildController) => {
                let _ = self.queue.advance();
                self.phase = R10BleEsp32c3ProbeRunnerPhase::ControllerReady;
                self.result(operation, None, R10BleEsp32c3ProbeRunnerStatus::Ok)
            }
            Some(R10BleEsp32c3ProbeOperation::BuildHostStack) => {
                let backend_output = self
                    .backend
                    .handle_event(R10BleEsp32c3ProbeBackendEvent::Start);
                let _ = self.queue.advance();
                self.phase = R10BleEsp32c3ProbeRunnerPhase::HostReady;
                self.result(
                    operation,
                    Some(backend_output),
                    R10BleEsp32c3ProbeRunnerStatus::Ok,
                )
            }
            Some(R10BleEsp32c3ProbeOperation::StartScan) => {
                let backend_output = self.backend.on_scan_started();
                let _ = self.queue.advance();
                self.phase = R10BleEsp32c3ProbeRunnerPhase::Scanning;
                self.result(
                    operation,
                    Some(backend_output),
                    R10BleEsp32c3ProbeRunnerStatus::Ok,
                )
            }
            Some(R10BleEsp32c3ProbeOperation::ConnectTarget) => {
                let backend_output = self
                    .backend
                    .handle_event(R10BleEsp32c3ProbeBackendEvent::ConnectOk);
                let _ = self.queue.advance();
                self.phase = R10BleEsp32c3ProbeRunnerPhase::Discovering;
                self.result(
                    operation,
                    Some(backend_output),
                    R10BleEsp32c3ProbeRunnerStatus::Ok,
                )
            }
            Some(R10BleEsp32c3ProbeOperation::DiscoverGatt) => {
                let backend_output = self
                    .backend
                    .handle_event(R10BleEsp32c3ProbeBackendEvent::LiveGattContract);
                let _ = self.queue.advance();
                self.phase = R10BleEsp32c3ProbeRunnerPhase::Subscribing;
                self.result(
                    operation,
                    Some(backend_output),
                    R10BleEsp32c3ProbeRunnerStatus::Ok,
                )
            }
            Some(R10BleEsp32c3ProbeOperation::SubscribeCccd { .. }) => {
                let backend_output =
                    self.backend
                        .handle_event(R10BleEsp32c3ProbeBackendEvent::SubscribeWritten {
                            success: true,
                            now_ms,
                        });
                let _ = self.queue.advance();
                self.phase = R10BleEsp32c3ProbeRunnerPhase::StartingRemote;
                self.result(
                    operation,
                    Some(backend_output),
                    R10BleEsp32c3ProbeRunnerStatus::Ok,
                )
            }
            Some(R10BleEsp32c3ProbeOperation::WriteRemoteStart { .. }) => {
                let backend_output =
                    self.backend
                        .handle_event(R10BleEsp32c3ProbeBackendEvent::RemoteStartWritten {
                            success: true,
                            now_ms,
                        });
                let _ = self.queue.advance();
                self.phase = R10BleEsp32c3ProbeRunnerPhase::Polling;
                self.result(
                    operation,
                    Some(backend_output),
                    R10BleEsp32c3ProbeRunnerStatus::Ok,
                )
            }
            Some(R10BleEsp32c3ProbeOperation::WritePoll { .. }) => {
                let backend_output =
                    self.backend
                        .handle_event(R10BleEsp32c3ProbeBackendEvent::PollWritten {
                            success: true,
                            now_ms,
                        });
                let _ = self.queue.advance();
                self.phase = R10BleEsp32c3ProbeRunnerPhase::AwaitingNotify;
                self.result(
                    operation,
                    Some(backend_output),
                    R10BleEsp32c3ProbeRunnerStatus::Ok,
                )
            }
            Some(R10BleEsp32c3ProbeOperation::AwaitNotify { .. }) => {
                self.phase = R10BleEsp32c3ProbeRunnerPhase::AwaitingNotify;
                self.result(operation, None, R10BleEsp32c3ProbeRunnerStatus::Awaiting)
            }
            Some(R10BleEsp32c3ProbeOperation::Backoff) => {
                let _ = self.queue.advance();
                self.phase = R10BleEsp32c3ProbeRunnerPhase::Backoff;
                self.result(operation, None, R10BleEsp32c3ProbeRunnerStatus::Ok)
            }
            None => {
                self.phase = R10BleEsp32c3ProbeRunnerPhase::Complete;
                self.result(None, None, R10BleEsp32c3ProbeRunnerStatus::Complete)
            }
        }
    }

    pub fn fail_current(&mut self, now_ms: u64) -> R10BleEsp32c3ProbeRunnerResult {
        let operation = self.current_operation();

        let backend_output = match operation {
            Some(R10BleEsp32c3ProbeOperation::ConnectTarget) => Some(
                self.backend
                    .handle_event(R10BleEsp32c3ProbeBackendEvent::ConnectFailed { now_ms }),
            ),
            Some(R10BleEsp32c3ProbeOperation::SubscribeCccd { .. }) => Some(
                self.backend
                    .handle_event(R10BleEsp32c3ProbeBackendEvent::SubscribeWritten {
                        success: false,
                        now_ms,
                    }),
            ),
            Some(R10BleEsp32c3ProbeOperation::WriteRemoteStart { .. }) => Some(
                self.backend
                    .handle_event(R10BleEsp32c3ProbeBackendEvent::RemoteStartWritten {
                        success: false,
                        now_ms,
                    }),
            ),
            Some(R10BleEsp32c3ProbeOperation::WritePoll { .. }) => Some(self.backend.handle_event(
                R10BleEsp32c3ProbeBackendEvent::PollWritten {
                    success: false,
                    now_ms,
                },
            )),
            _ => Some(
                self.backend
                    .handle_event(R10BleEsp32c3ProbeBackendEvent::Timeout),
            ),
        };

        self.phase = R10BleEsp32c3ProbeRunnerPhase::Backoff;

        self.result(
            operation,
            backend_output,
            R10BleEsp32c3ProbeRunnerStatus::Failed,
        )
    }

    pub fn on_advertisement(
        &mut self,
        address: Option<&str>,
        name: Option<&str>,
    ) -> R10BleEsp32c3ProbeRunnerResult {
        let operation = self.current_operation();

        let backend_output = self
            .backend
            .handle_event(R10BleEsp32c3ProbeBackendEvent::Advertisement { address, name });

        self.phase = if backend_output.state == R10BleEsp32c3ProbeBackendState::Connecting {
            R10BleEsp32c3ProbeRunnerPhase::Matched
        } else {
            R10BleEsp32c3ProbeRunnerPhase::Scanning
        };

        let status = if self.phase == R10BleEsp32c3ProbeRunnerPhase::Matched {
            R10BleEsp32c3ProbeRunnerStatus::Ok
        } else {
            R10BleEsp32c3ProbeRunnerStatus::Ignored
        };

        self.result(operation, Some(backend_output), status)
    }

    pub fn on_default_r10_advertisement(&mut self) -> R10BleEsp32c3ProbeRunnerResult {
        self.on_advertisement(
            Some(super::r10_ble_transport::R10_BLE_DEFAULT_TARGET_ADDRESS),
            Some(super::r10_ble_transport::R10_BLE_DEFAULT_ADVERTISED_NAME),
        )
    }

    pub fn on_notify(&mut self, payload: &[u8], now_ms: u64) -> R10BleEsp32c3ProbeRunnerResult {
        let operation = self.current_operation();

        if !matches!(
            operation,
            Some(R10BleEsp32c3ProbeOperation::AwaitNotify { .. })
        ) {
            return self.result(operation, None, R10BleEsp32c3ProbeRunnerStatus::Ignored);
        }

        let backend_output = self
            .backend
            .handle_event(R10BleEsp32c3ProbeBackendEvent::Notify {
                handle: R10_BLE_LIVE_NOTIFY_VALUE_HANDLE,
                payload,
                now_ms,
            });

        let _ = self.queue.advance();
        self.phase = R10BleEsp32c3ProbeRunnerPhase::Notified;

        self.result(
            operation,
            Some(backend_output),
            R10BleEsp32c3ProbeRunnerStatus::Ok,
        )
    }
}

impl Default for R10BleEsp32c3ProbeRunner {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(all(target_arch = "riscv32", feature = "r10-ble-host"))]
pub fn r10_ble_x4_emit_esp32c3_probe_runner_startup_log() {
    let runner = R10BleEsp32c3ProbeRunner::new();
    let record = runner.startup_record();

    esp_println::println!(
        "rustmix event={} target={} phase={} operation={} reader={} status={}",
        record.event,
        record.target,
        record.phase,
        record.operation,
        record.reader,
        record.status
    );

    esp_println::println!(
        "rustmix event=ble_runner target=esp_c phase=created operation_queue=session reader=reader_off status=ready"
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rustmix_x4::ring_remote::r10_ble_transport::R10BleCommand;

    fn motion_payload() -> [u8; 16] {
        [0x02, 0x02, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x04]
    }

    fn advance_to_scan(runner: &mut R10BleEsp32c3ProbeRunner) {
        assert_eq!(
            runner.complete_current_success(1).phase,
            R10BleEsp32c3ProbeRunnerPhase::RadioReady
        );
        assert_eq!(
            runner.complete_current_success(2).phase,
            R10BleEsp32c3ProbeRunnerPhase::ControllerReady
        );
        assert_eq!(
            runner.complete_current_success(3).phase,
            R10BleEsp32c3ProbeRunnerPhase::HostReady
        );
        assert_eq!(
            runner.complete_current_success(4).phase,
            R10BleEsp32c3ProbeRunnerPhase::Scanning
        );
    }

    fn advance_to_await_notify(runner: &mut R10BleEsp32c3ProbeRunner) {
        advance_to_scan(runner);
        let matched = runner.on_default_r10_advertisement();
        assert_eq!(matched.phase, R10BleEsp32c3ProbeRunnerPhase::Matched);

        assert_eq!(
            runner.complete_current_success(5).phase,
            R10BleEsp32c3ProbeRunnerPhase::Discovering
        );
        assert_eq!(
            runner.complete_current_success(6).phase,
            R10BleEsp32c3ProbeRunnerPhase::Subscribing
        );
        assert_eq!(
            runner.complete_current_success(7).phase,
            R10BleEsp32c3ProbeRunnerPhase::StartingRemote
        );
        assert_eq!(
            runner.complete_current_success(8).phase,
            R10BleEsp32c3ProbeRunnerPhase::Polling
        );
        assert_eq!(
            runner.complete_current_success(9).phase,
            R10BleEsp32c3ProbeRunnerPhase::AwaitingNotify
        );
    }

    #[test]
    fn r10_ble_esp32c3_probe_runner_defaults_to_reader_off() {
        let runner = R10BleEsp32c3ProbeRunner::new();

        assert_eq!(runner.phase(), R10BleEsp32c3ProbeRunnerPhase::Created);
        assert_eq!(
            runner.current_operation(),
            Some(R10BleEsp32c3ProbeOperation::InitRadio)
        );
        assert_eq!(runner.queue_remaining(), 10);
        assert!(!runner.is_reader_input_enabled());

        let record = runner.startup_record();

        assert_eq!(record.event, "ble_runner");
        assert_eq!(record.target, "esp_c");
        assert_eq!(record.reader, "reader_off");
        assert!(record.is_monitor_safe());
    }

    #[test]
    fn r10_ble_esp32c3_probe_runner_labels_are_monitor_safe() {
        let phases = [
            R10BleEsp32c3ProbeRunnerPhase::Created,
            R10BleEsp32c3ProbeRunnerPhase::RadioReady,
            R10BleEsp32c3ProbeRunnerPhase::ControllerReady,
            R10BleEsp32c3ProbeRunnerPhase::HostReady,
            R10BleEsp32c3ProbeRunnerPhase::Scanning,
            R10BleEsp32c3ProbeRunnerPhase::Matched,
            R10BleEsp32c3ProbeRunnerPhase::Connecting,
            R10BleEsp32c3ProbeRunnerPhase::Discovering,
            R10BleEsp32c3ProbeRunnerPhase::Subscribing,
            R10BleEsp32c3ProbeRunnerPhase::StartingRemote,
            R10BleEsp32c3ProbeRunnerPhase::Polling,
            R10BleEsp32c3ProbeRunnerPhase::AwaitingNotify,
            R10BleEsp32c3ProbeRunnerPhase::Notified,
            R10BleEsp32c3ProbeRunnerPhase::Backoff,
            R10BleEsp32c3ProbeRunnerPhase::Complete,
        ];

        for phase in phases {
            assert!(
                super::super::r10_ble_device_task::r10_ble_device_task_monitor_label_is_safe(
                    phase.as_str()
                )
            );
        }

        let statuses = [
            R10BleEsp32c3ProbeRunnerStatus::Ready,
            R10BleEsp32c3ProbeRunnerStatus::Pending,
            R10BleEsp32c3ProbeRunnerStatus::Ok,
            R10BleEsp32c3ProbeRunnerStatus::Ignored,
            R10BleEsp32c3ProbeRunnerStatus::Failed,
            R10BleEsp32c3ProbeRunnerStatus::Awaiting,
            R10BleEsp32c3ProbeRunnerStatus::Complete,
        ];

        for status in statuses {
            assert!(
                super::super::r10_ble_device_task::r10_ble_device_task_monitor_label_is_safe(
                    status.as_str()
                )
            );
        }
    }

    #[test]
    fn r10_ble_esp32c3_probe_runner_advances_startup_to_scan() {
        let mut runner = R10BleEsp32c3ProbeRunner::new();

        advance_to_scan(&mut runner);

        assert_eq!(runner.phase(), R10BleEsp32c3ProbeRunnerPhase::Scanning);
        assert_eq!(
            runner.current_operation(),
            Some(R10BleEsp32c3ProbeOperation::ConnectTarget)
        );
        assert_eq!(
            runner.backend().state(),
            R10BleEsp32c3ProbeBackendState::Scanning
        );
    }

    #[test]
    fn r10_ble_esp32c3_probe_runner_matches_r10_advertisement_without_reader_input() {
        let mut runner = R10BleEsp32c3ProbeRunner::new();

        advance_to_scan(&mut runner);

        let result = runner.on_default_r10_advertisement();

        assert_eq!(result.phase, R10BleEsp32c3ProbeRunnerPhase::Matched);
        assert_eq!(
            result.operation,
            Some(R10BleEsp32c3ProbeOperation::ConnectTarget)
        );
        assert_eq!(
            result.backend_output.unwrap().state,
            R10BleEsp32c3ProbeBackendState::Connecting
        );
        assert!(!result.is_reader_input_enabled());
        assert!(result.is_monitor_safe());
    }

    #[test]
    fn r10_ble_esp32c3_probe_runner_ignores_unmatched_advertisement() {
        let mut runner = R10BleEsp32c3ProbeRunner::new();

        advance_to_scan(&mut runner);

        let result = runner.on_advertisement(Some("00:00:00:00:00:00"), Some("Other"));

        assert_eq!(result.phase, R10BleEsp32c3ProbeRunnerPhase::Scanning);
        assert_eq!(result.record.status, "ignored");
        assert_eq!(
            runner.current_operation(),
            Some(R10BleEsp32c3ProbeOperation::ConnectTarget)
        );
        assert!(result.is_monitor_safe());
    }

    #[test]
    fn r10_ble_esp32c3_probe_runner_connect_gatt_and_writes_flow_to_await_notify() {
        let mut runner = R10BleEsp32c3ProbeRunner::new();

        advance_to_await_notify(&mut runner);

        assert_eq!(
            runner.phase(),
            R10BleEsp32c3ProbeRunnerPhase::AwaitingNotify
        );
        assert_eq!(
            runner.current_operation(),
            Some(R10BleEsp32c3ProbeOperation::AwaitNotify {
                notify_handle: R10_BLE_LIVE_NOTIFY_VALUE_HANDLE
            })
        );
        assert!(runner.backend().bridge().report().notify_subscribed);
        assert!(runner.backend().bridge().report().remote_started);
    }

    #[test]
    fn r10_ble_esp32c3_probe_runner_motion_notify_is_log_only() {
        let mut runner = R10BleEsp32c3ProbeRunner::new();

        advance_to_await_notify(&mut runner);

        let payload = motion_payload();
        let result = runner.on_notify(&payload, 10_000);

        assert_eq!(result.phase, R10BleEsp32c3ProbeRunnerPhase::Notified);
        assert_eq!(result.record.reader, "reader_off");
        assert!(!result.is_reader_input_enabled());
        assert_eq!(runner.backend().bridge().report().motion_count, 1);
        assert!(runner.backend().bridge().report().connectivity_validated());
        assert!(result.is_monitor_safe());
    }

    #[test]
    fn r10_ble_esp32c3_probe_runner_bad_notify_is_ignored_and_reader_off() {
        let mut runner = R10BleEsp32c3ProbeRunner::new();

        advance_to_await_notify(&mut runner);

        let bad = [0x02, 0x02, 0, 0];
        let result = runner.on_notify(&bad, 10_000);

        assert_eq!(result.phase, R10BleEsp32c3ProbeRunnerPhase::Notified);
        assert_eq!(result.record.reader, "reader_off");
        assert_eq!(
            result
                .backend_output
                .unwrap()
                .record
                .unwrap()
                .value_for_key("outcome"),
            Some("ignored")
        );
        assert!(!result.is_reader_input_enabled());
    }

    #[test]
    fn r10_ble_esp32c3_probe_runner_notify_before_await_is_ignored() {
        let mut runner = R10BleEsp32c3ProbeRunner::new();

        let payload = motion_payload();
        let result = runner.on_notify(&payload, 10_000);

        assert_eq!(result.record.status, "ignored");
        assert_eq!(result.backend_output, None);
        assert_eq!(runner.phase(), R10BleEsp32c3ProbeRunnerPhase::Created);
    }

    #[test]
    fn r10_ble_esp32c3_probe_runner_connect_failure_enters_backoff() {
        let mut runner = R10BleEsp32c3ProbeRunner::new();

        advance_to_scan(&mut runner);
        let _ = runner.on_default_r10_advertisement();

        let result = runner.fail_current(12_000);

        assert_eq!(result.phase, R10BleEsp32c3ProbeRunnerPhase::Backoff);
        assert_eq!(result.record.status, "failed");
        assert!(runner.phase().is_terminal());
        assert!(!result.is_reader_input_enabled());
    }

    #[test]
    fn r10_ble_esp32c3_probe_runner_write_failure_enters_backoff() {
        let mut runner = R10BleEsp32c3ProbeRunner::new();

        advance_to_scan(&mut runner);
        let _ = runner.on_default_r10_advertisement();
        let _ = runner.complete_current_success(5);
        let _ = runner.complete_current_success(6);

        assert_eq!(
            runner.current_operation(),
            Some(R10BleEsp32c3ProbeOperation::SubscribeCccd {
                cccd_handle: super::super::r10_ble_transport::R10_BLE_LIVE_NOTIFY_CCCD_HANDLE
            })
        );

        let result = runner.fail_current(12_000);

        assert_eq!(result.phase, R10BleEsp32c3ProbeRunnerPhase::Backoff);
        assert_eq!(
            result
                .backend_output
                .unwrap()
                .record
                .unwrap()
                .value_for_key("outcome"),
            Some("failed")
        );
        assert!(!result.is_reader_input_enabled());
    }

    #[test]
    fn r10_ble_esp32c3_probe_runner_records_are_monitor_safe_through_success_flow() {
        let mut runner = R10BleEsp32c3ProbeRunner::new();

        for now in 1..=4 {
            let result = runner.complete_current_success(now);
            assert!(result.is_monitor_safe());
            assert_eq!(result.record.reader, "reader_off");
        }

        let matched = runner.on_default_r10_advertisement();
        assert!(matched.is_monitor_safe());

        for now in 5..=9 {
            let result = runner.complete_current_success(now);
            assert!(result.is_monitor_safe());
            assert_eq!(result.record.reader, "reader_off");
        }

        let payload = motion_payload();
        let notify = runner.on_notify(&payload, 10_000);
        assert!(notify.is_monitor_safe());
    }

    #[test]
    fn r10_ble_esp32c3_probe_runner_preserves_stock_write_payloads() {
        let mut runner = R10BleEsp32c3ProbeRunner::new();

        while !matches!(
            runner.current_operation(),
            Some(R10BleEsp32c3ProbeOperation::WriteRemoteStart { .. })
        ) {
            let _ = runner.complete_current_success(1);
            if runner.phase() == R10BleEsp32c3ProbeRunnerPhase::Scanning {
                let _ = runner.on_default_r10_advertisement();
            }
        }

        match runner.current_operation().unwrap() {
            R10BleEsp32c3ProbeOperation::WriteRemoteStart { payload, .. } => {
                assert_eq!(payload, R10BleCommand::StartRemote.packet().as_slice());
            }
            _ => panic!("expected remote start write"),
        }

        let _ = runner.complete_current_success(10);

        match runner.current_operation().unwrap() {
            R10BleEsp32c3ProbeOperation::WritePoll { payload, .. } => {
                assert_eq!(payload, R10BleCommand::PollRemote.packet().as_slice());
            }
            _ => panic!("expected poll write"),
        }
    }

    #[test]
    fn r10_ble_esp32c3_probe_runner_complete_without_operation_is_terminal() {
        let mut runner = R10BleEsp32c3ProbeRunner::new();

        advance_to_await_notify(&mut runner);
        let payload = motion_payload();
        let _ = runner.on_notify(&payload, 10_000);

        assert_eq!(runner.current_operation(), None);

        let result = runner.complete_current_success(11_000);

        assert_eq!(result.phase, R10BleEsp32c3ProbeRunnerPhase::Complete);
        assert!(runner.phase().is_terminal());
        assert_eq!(result.record.status, "complete");
        assert!(!result.is_reader_input_enabled());
    }
}
