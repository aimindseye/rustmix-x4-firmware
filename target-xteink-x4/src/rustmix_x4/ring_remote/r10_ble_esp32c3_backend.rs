// ESP32-C3 ProbeOnly backend task for the COLMI R10 BLE remote.
//
// This module is the firmware-side backend controller that concrete
// `esp-radio` / `trouble-host` callbacks feed. It owns the ProbeOnly bridge
// added in r4t, keeps Reader input disabled, and converts backend callbacks
// into monitor-safe runtime serial records.

use super::r10_ble_device_task::{
    R10BleDeviceTaskX4ProbeOnlyBleBridge, R10BleDeviceTaskX4ProbeOnlyBridgeEvent,
    R10BleDeviceTaskX4ProbeOnlyBridgeStage, R10BleDeviceTaskX4RuntimeSerialRecord,
};
use super::r10_ble_transport::{
    R10_BLE_DEFAULT_ADVERTISED_NAME, R10_BLE_DEFAULT_TARGET_ADDRESS, R10BleAdvertisedDevice,
    R10BleGattHandles,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R10BleEsp32c3ProbeBackendStep {
    InitRadio,
    BuildController,
    BuildHostStack,
    StartScan,
    ConnectMatched,
    DiscoverGatt,
    SubscribeNotify,
    WriteRemoteStart,
    PollAndNotify,
}

impl R10BleEsp32c3ProbeBackendStep {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::InitRadio => "init_radio",
            Self::BuildController => "build_controller",
            Self::BuildHostStack => "build_host_stack",
            Self::StartScan => "start_scan",
            Self::ConnectMatched => "connect_matched",
            Self::DiscoverGatt => "discover_gatt",
            Self::SubscribeNotify => "subscribe_notify",
            Self::WriteRemoteStart => "write_remote_start",
            Self::PollAndNotify => "poll_and_notify",
        }
    }
}

pub const R10_BLE_ESP32C3_PROBE_BACKEND_PLAN: [R10BleEsp32c3ProbeBackendStep; 9] = [
    R10BleEsp32c3ProbeBackendStep::InitRadio,
    R10BleEsp32c3ProbeBackendStep::BuildController,
    R10BleEsp32c3ProbeBackendStep::BuildHostStack,
    R10BleEsp32c3ProbeBackendStep::StartScan,
    R10BleEsp32c3ProbeBackendStep::ConnectMatched,
    R10BleEsp32c3ProbeBackendStep::DiscoverGatt,
    R10BleEsp32c3ProbeBackendStep::SubscribeNotify,
    R10BleEsp32c3ProbeBackendStep::WriteRemoteStart,
    R10BleEsp32c3ProbeBackendStep::PollAndNotify,
];

pub const fn r10_ble_esp32c3_probe_backend_plan() -> &'static [R10BleEsp32c3ProbeBackendStep] {
    &R10_BLE_ESP32C3_PROBE_BACKEND_PLAN
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R10BleEsp32c3ProbeBackendState {
    Created,
    AdapterReady,
    Scanning,
    Connecting,
    Discovering,
    Subscribing,
    StartingRemote,
    Polling,
    Notified,
    Backoff,
    Completed,
}

impl R10BleEsp32c3ProbeBackendState {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Created => "created",
            Self::AdapterReady => "adapter_ready",
            Self::Scanning => "scanning",
            Self::Connecting => "connecting",
            Self::Discovering => "discovering",
            Self::Subscribing => "subscribing",
            Self::StartingRemote => "starting_remote",
            Self::Polling => "polling",
            Self::Notified => "notified",
            Self::Backoff => "backoff",
            Self::Completed => "completed",
        }
    }

    pub const fn is_active(&self) -> bool {
        matches!(
            self,
            Self::AdapterReady
                | Self::Scanning
                | Self::Connecting
                | Self::Discovering
                | Self::Subscribing
                | Self::StartingRemote
                | Self::Polling
                | Self::Notified
        )
    }

    pub const fn is_terminal(&self) -> bool {
        matches!(self, Self::Backoff | Self::Completed)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R10BleEsp32c3ProbeBackendEvent<'a> {
    Start,
    AdapterReady,
    ScanStarted,
    Advertisement {
        address: Option<&'a str>,
        name: Option<&'a str>,
    },
    ConnectOk,
    ConnectFailed {
        now_ms: u64,
    },
    GattHandles {
        handles: R10BleGattHandles,
    },
    LiveGattContract,
    SubscribeWritten {
        success: bool,
        now_ms: u64,
    },
    RemoteStartWritten {
        success: bool,
        now_ms: u64,
    },
    PollWritten {
        success: bool,
        now_ms: u64,
    },
    Notify {
        handle: u16,
        payload: &'a [u8],
        now_ms: u64,
    },
    Timeout,
    Complete,
}

impl<'a> R10BleEsp32c3ProbeBackendEvent<'a> {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::AdapterReady => "adapter_ready",
            Self::ScanStarted => "scan_started",
            Self::Advertisement { .. } => "advertisement",
            Self::ConnectOk => "connect_ok",
            Self::ConnectFailed { .. } => "connect_failed",
            Self::GattHandles { .. } => "gatt_handles",
            Self::LiveGattContract => "live_gatt_contract",
            Self::SubscribeWritten { .. } => "subscribe_written",
            Self::RemoteStartWritten { .. } => "remote_start_written",
            Self::PollWritten { .. } => "poll_written",
            Self::Notify { .. } => "notify",
            Self::Timeout => "timeout",
            Self::Complete => "complete",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R10BleEsp32c3ProbeBackendOutput {
    pub state: R10BleEsp32c3ProbeBackendState,
    pub bridge_event: Option<R10BleDeviceTaskX4ProbeOnlyBridgeEvent>,
    pub record: Option<R10BleDeviceTaskX4RuntimeSerialRecord>,
    pub reader_input_enabled: bool,
}

impl R10BleEsp32c3ProbeBackendOutput {
    pub const fn empty(state: R10BleEsp32c3ProbeBackendState) -> Self {
        Self {
            state,
            bridge_event: None,
            record: None,
            reader_input_enabled: false,
        }
    }

    pub fn from_bridge_event(
        state: R10BleEsp32c3ProbeBackendState,
        event: R10BleDeviceTaskX4ProbeOnlyBridgeEvent,
    ) -> Self {
        Self {
            state,
            bridge_event: Some(event),
            record: event.record,
            reader_input_enabled: false,
        }
    }

    pub fn is_monitor_safe(&self) -> bool {
        self.bridge_event
            .map(|event| event.is_monitor_safe())
            .unwrap_or(true)
            && self
                .record
                .map(|record| record.is_monitor_safe())
                .unwrap_or(true)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct R10BleEsp32c3ProbeBackendTask<const N: usize> {
    bridge: R10BleDeviceTaskX4ProbeOnlyBleBridge<N>,
    state: R10BleEsp32c3ProbeBackendState,
    last_record: Option<R10BleDeviceTaskX4RuntimeSerialRecord>,
}

impl<const N: usize> R10BleEsp32c3ProbeBackendTask<N> {
    pub fn new() -> Self {
        Self {
            bridge: R10BleDeviceTaskX4ProbeOnlyBleBridge::new(),
            state: R10BleEsp32c3ProbeBackendState::Created,
            last_record: None,
        }
    }

    pub const fn state(&self) -> R10BleEsp32c3ProbeBackendState {
        self.state
    }

    pub const fn is_reader_input_enabled(&self) -> bool {
        false
    }

    pub fn bridge(&self) -> &R10BleDeviceTaskX4ProbeOnlyBleBridge<N> {
        &self.bridge
    }

    pub fn last_record(&self) -> Option<R10BleDeviceTaskX4RuntimeSerialRecord> {
        self.last_record
    }

    pub fn profile_record(&self) -> R10BleDeviceTaskX4RuntimeSerialRecord {
        self.bridge.profile_record()
    }

    pub fn trigger_record(&self) -> R10BleDeviceTaskX4RuntimeSerialRecord {
        self.bridge.trigger_record()
    }

    pub fn handle_event<'a>(
        &mut self,
        event: R10BleEsp32c3ProbeBackendEvent<'a>,
    ) -> R10BleEsp32c3ProbeBackendOutput {
        let output = match event {
            R10BleEsp32c3ProbeBackendEvent::Start => {
                self.state = R10BleEsp32c3ProbeBackendState::AdapterReady;
                R10BleEsp32c3ProbeBackendOutput {
                    state: self.state,
                    bridge_event: None,
                    record: Some(self.trigger_record()),
                    reader_input_enabled: false,
                }
            }
            R10BleEsp32c3ProbeBackendEvent::AdapterReady => {
                self.state = R10BleEsp32c3ProbeBackendState::AdapterReady;
                R10BleEsp32c3ProbeBackendOutput::empty(self.state)
            }
            R10BleEsp32c3ProbeBackendEvent::ScanStarted => {
                self.state = R10BleEsp32c3ProbeBackendState::Scanning;
                R10BleEsp32c3ProbeBackendOutput::from_bridge_event(
                    self.state,
                    self.bridge.start_scan(),
                )
            }
            R10BleEsp32c3ProbeBackendEvent::Advertisement { address, name } => {
                let bridge_event = self
                    .bridge
                    .on_advertised_device(R10BleAdvertisedDevice { address, name });

                self.state =
                    if bridge_event.stage == R10BleDeviceTaskX4ProbeOnlyBridgeStage::TargetSeen {
                        R10BleEsp32c3ProbeBackendState::Connecting
                    } else {
                        R10BleEsp32c3ProbeBackendState::Scanning
                    };

                R10BleEsp32c3ProbeBackendOutput::from_bridge_event(self.state, bridge_event)
            }
            R10BleEsp32c3ProbeBackendEvent::ConnectOk => {
                self.state = R10BleEsp32c3ProbeBackendState::Discovering;
                R10BleEsp32c3ProbeBackendOutput::from_bridge_event(
                    self.state,
                    self.bridge.on_connected(),
                )
            }
            R10BleEsp32c3ProbeBackendEvent::ConnectFailed { now_ms } => {
                self.state = R10BleEsp32c3ProbeBackendState::Backoff;
                R10BleEsp32c3ProbeBackendOutput::from_bridge_event(
                    self.state,
                    self.bridge.on_connect_failed(now_ms),
                )
            }
            R10BleEsp32c3ProbeBackendEvent::GattHandles { handles } => {
                let bridge_event = self.bridge.on_discovered_handles(handles);

                self.state =
                    if bridge_event.stage == R10BleDeviceTaskX4ProbeOnlyBridgeStage::GattReady {
                        R10BleEsp32c3ProbeBackendState::Subscribing
                    } else {
                        R10BleEsp32c3ProbeBackendState::Backoff
                    };

                R10BleEsp32c3ProbeBackendOutput::from_bridge_event(self.state, bridge_event)
            }
            R10BleEsp32c3ProbeBackendEvent::LiveGattContract => {
                let bridge_event = self.bridge.on_live_gatt_contract_discovered();
                self.state = R10BleEsp32c3ProbeBackendState::Subscribing;
                R10BleEsp32c3ProbeBackendOutput::from_bridge_event(self.state, bridge_event)
            }
            R10BleEsp32c3ProbeBackendEvent::SubscribeWritten { success, now_ms } => {
                let bridge_event = self.bridge.on_subscribe_written(success, now_ms);
                self.state = if success {
                    R10BleEsp32c3ProbeBackendState::StartingRemote
                } else {
                    R10BleEsp32c3ProbeBackendState::Backoff
                };
                R10BleEsp32c3ProbeBackendOutput::from_bridge_event(self.state, bridge_event)
            }
            R10BleEsp32c3ProbeBackendEvent::RemoteStartWritten { success, now_ms } => {
                let bridge_event = self.bridge.on_remote_start_written(success, now_ms);
                self.state = if success {
                    R10BleEsp32c3ProbeBackendState::Polling
                } else {
                    R10BleEsp32c3ProbeBackendState::Backoff
                };
                R10BleEsp32c3ProbeBackendOutput::from_bridge_event(self.state, bridge_event)
            }
            R10BleEsp32c3ProbeBackendEvent::PollWritten { success, now_ms } => {
                let bridge_event = self.bridge.on_poll_written(success, now_ms);
                self.state = if success {
                    R10BleEsp32c3ProbeBackendState::Polling
                } else {
                    R10BleEsp32c3ProbeBackendState::Backoff
                };
                R10BleEsp32c3ProbeBackendOutput::from_bridge_event(self.state, bridge_event)
            }
            R10BleEsp32c3ProbeBackendEvent::Notify {
                handle,
                payload,
                now_ms,
            } => {
                let bridge_event = self.bridge.on_notify(handle, payload, now_ms);
                self.state = R10BleEsp32c3ProbeBackendState::Notified;
                R10BleEsp32c3ProbeBackendOutput::from_bridge_event(self.state, bridge_event)
            }
            R10BleEsp32c3ProbeBackendEvent::Timeout => {
                self.state = R10BleEsp32c3ProbeBackendState::Backoff;
                R10BleEsp32c3ProbeBackendOutput::from_bridge_event(
                    self.state,
                    self.bridge.on_timeout(),
                )
            }
            R10BleEsp32c3ProbeBackendEvent::Complete => {
                self.state = R10BleEsp32c3ProbeBackendState::Completed;
                R10BleEsp32c3ProbeBackendOutput::empty(self.state)
            }
        };

        self.last_record = output.record;
        output
    }

    pub fn on_scan_started(&mut self) -> R10BleEsp32c3ProbeBackendOutput {
        self.handle_event(R10BleEsp32c3ProbeBackendEvent::ScanStarted)
    }

    pub fn on_default_r10_advertisement(&mut self) -> R10BleEsp32c3ProbeBackendOutput {
        self.handle_event(R10BleEsp32c3ProbeBackendEvent::Advertisement {
            address: Some(R10_BLE_DEFAULT_TARGET_ADDRESS),
            name: Some(R10_BLE_DEFAULT_ADVERTISED_NAME),
        })
    }
}

impl Default for R10BleEsp32c3ProbeBackendTask<8> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(all(target_arch = "riscv32", feature = "r10-ble-host"))]
pub fn r10_ble_esp32c3_probe_backend_type_boundary() -> (&'static str, &'static str, &'static str) {
    (
        core::any::type_name::<super::r10_ble_host::R10BleController>(),
        core::any::type_name::<super::r10_ble_host::R10BleHostResources>(),
        core::any::type_name::<super::r10_ble_host::R10BleStack<'static>>(),
    )
}

#[cfg(all(not(target_arch = "riscv32"), feature = "r10-ble-host"))]
pub fn r10_ble_esp32c3_probe_backend_type_boundary() -> (&'static str, &'static str, &'static str) {
    (
        "host_test_r10_ble_controller_boundary",
        "host_test_r10_ble_host_resources_boundary",
        "host_test_r10_ble_stack_boundary",
    )
}

#[cfg(all(target_arch = "riscv32", feature = "r10-ble-host"))]
fn print_backend_record(record: R10BleDeviceTaskX4RuntimeSerialRecord) {
    esp_println::println!(
        "rustmix event={} kind={} mode={} source={} lifecycle={} command={} outcome={} reader={} status={}",
        record.value_for_key("event").unwrap_or("none"),
        record.value_for_key("kind").unwrap_or("none"),
        record.value_for_key("mode").unwrap_or("none"),
        record.value_for_key("source").unwrap_or("none"),
        record.value_for_key("lifecycle").unwrap_or("none"),
        record.value_for_key("command").unwrap_or("none"),
        record.value_for_key("outcome").unwrap_or("none"),
        record.value_for_key("reader").unwrap_or("none"),
        record.value_for_key("status").unwrap_or("none"),
    );
}

#[cfg(all(target_arch = "riscv32", feature = "r10-ble-host"))]
pub fn r10_ble_x4_emit_esp32c3_probe_backend_startup_log() {
    let task = R10BleEsp32c3ProbeBackendTask::<8>::new();

    esp_println::println!(
        "rustmix event=ble_backend kind=task target=esp32c3 mode=probe_only reader=reader_off status=ready"
    );

    for step in r10_ble_esp32c3_probe_backend_plan() {
        esp_println::println!(
            "rustmix event=ble_backend kind=plan target=esp32c3 step={} mode=probe_only reader=reader_off status=ready",
            step.as_str()
        );
    }

    print_backend_record(task.profile_record());
    print_backend_record(task.trigger_record());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rustmix_x4::ring_remote::r10_ble_transport::{
        R10_BLE_LIVE_NOTIFY_VALUE_HANDLE, R10BleGattHandles,
    };

    #[test]
    fn r10_ble_esp32c3_probe_backend_plan_is_ordered() {
        let plan = r10_ble_esp32c3_probe_backend_plan();

        assert_eq!(plan.len(), 9);
        assert_eq!(plan[0], R10BleEsp32c3ProbeBackendStep::InitRadio);
        assert_eq!(plan[1], R10BleEsp32c3ProbeBackendStep::BuildController);
        assert_eq!(plan[2], R10BleEsp32c3ProbeBackendStep::BuildHostStack);
        assert_eq!(plan[3], R10BleEsp32c3ProbeBackendStep::StartScan);
        assert_eq!(plan[8], R10BleEsp32c3ProbeBackendStep::PollAndNotify);
    }

    #[test]
    fn r10_ble_esp32c3_probe_backend_labels_are_monitor_safe() {
        for step in r10_ble_esp32c3_probe_backend_plan() {
            assert!(
                super::super::r10_ble_device_task::r10_ble_device_task_monitor_label_is_safe(
                    step.as_str()
                )
            );
        }

        let states = [
            R10BleEsp32c3ProbeBackendState::Created,
            R10BleEsp32c3ProbeBackendState::AdapterReady,
            R10BleEsp32c3ProbeBackendState::Scanning,
            R10BleEsp32c3ProbeBackendState::Connecting,
            R10BleEsp32c3ProbeBackendState::Discovering,
            R10BleEsp32c3ProbeBackendState::Subscribing,
            R10BleEsp32c3ProbeBackendState::StartingRemote,
            R10BleEsp32c3ProbeBackendState::Polling,
            R10BleEsp32c3ProbeBackendState::Notified,
            R10BleEsp32c3ProbeBackendState::Backoff,
            R10BleEsp32c3ProbeBackendState::Completed,
        ];

        for state in states {
            assert!(
                super::super::r10_ble_device_task::r10_ble_device_task_monitor_label_is_safe(
                    state.as_str()
                )
            );
        }
    }

    #[test]
    fn r10_ble_esp32c3_probe_backend_defaults_to_probe_only_reader_off() {
        let task = R10BleEsp32c3ProbeBackendTask::<8>::new();

        assert_eq!(task.state(), R10BleEsp32c3ProbeBackendState::Created);
        assert!(!task.is_reader_input_enabled());
        assert_eq!(
            task.profile_record().value_for_key("reader"),
            Some("reader_off")
        );
        assert_eq!(
            task.trigger_record().value_for_key("mode"),
            Some("probe_only")
        );
    }

    #[test]
    fn r10_ble_esp32c3_probe_backend_start_event_emits_trigger_record() {
        let mut task = R10BleEsp32c3ProbeBackendTask::<8>::new();
        let output = task.handle_event(R10BleEsp32c3ProbeBackendEvent::Start);

        assert_eq!(output.state, R10BleEsp32c3ProbeBackendState::AdapterReady);
        assert_eq!(output.record.unwrap().kind_value(), "trigger");
        assert_eq!(
            output.record.unwrap().value_for_key("reader"),
            Some("reader_off")
        );
        assert!(!output.reader_input_enabled);
        assert!(output.is_monitor_safe());
    }

    #[test]
    fn r10_ble_esp32c3_probe_backend_scan_and_match_move_to_connecting() {
        let mut task = R10BleEsp32c3ProbeBackendTask::<8>::new();

        let scan = task.on_scan_started();
        let matched = task.on_default_r10_advertisement();

        assert_eq!(scan.state, R10BleEsp32c3ProbeBackendState::Scanning);
        assert_eq!(matched.state, R10BleEsp32c3ProbeBackendState::Connecting);
        assert_eq!(
            matched.bridge_event.unwrap().stage,
            R10BleDeviceTaskX4ProbeOnlyBridgeStage::TargetSeen
        );
        assert_eq!(
            matched.record.unwrap().value_for_key("command"),
            Some("connect_target")
        );
        assert_eq!(
            matched.record.unwrap().value_for_key("reader"),
            Some("reader_off")
        );
    }

    #[test]
    fn r10_ble_esp32c3_probe_backend_ignored_advertisement_stays_scanning() {
        let mut task = R10BleEsp32c3ProbeBackendTask::<8>::new();

        let ignored = task.handle_event(R10BleEsp32c3ProbeBackendEvent::Advertisement {
            address: Some("00:00:00:00:00:00"),
            name: Some("Other Device"),
        });

        assert_eq!(ignored.state, R10BleEsp32c3ProbeBackendState::Scanning);
        assert_eq!(
            ignored.bridge_event.unwrap().stage,
            R10BleDeviceTaskX4ProbeOnlyBridgeStage::AdvertisementIgnored
        );
        assert_eq!(
            ignored.record.unwrap().value_for_key("outcome"),
            Some("ignored")
        );
    }

    #[test]
    fn r10_ble_esp32c3_probe_backend_connect_gatt_write_notify_flow_is_log_only() {
        let mut task = R10BleEsp32c3ProbeBackendTask::<16>::new();
        let motion = [0x02, 0x02, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x04];

        let _ = task.on_scan_started();
        let _ = task.on_default_r10_advertisement();

        let connected = task.handle_event(R10BleEsp32c3ProbeBackendEvent::ConnectOk);
        let gatt = task.handle_event(R10BleEsp32c3ProbeBackendEvent::LiveGattContract);
        let subscribe = task.handle_event(R10BleEsp32c3ProbeBackendEvent::SubscribeWritten {
            success: true,
            now_ms: 10_000,
        });
        let start = task.handle_event(R10BleEsp32c3ProbeBackendEvent::RemoteStartWritten {
            success: true,
            now_ms: 10_001,
        });
        let poll = task.handle_event(R10BleEsp32c3ProbeBackendEvent::PollWritten {
            success: true,
            now_ms: 11_001,
        });
        let notify = task.handle_event(R10BleEsp32c3ProbeBackendEvent::Notify {
            handle: R10_BLE_LIVE_NOTIFY_VALUE_HANDLE,
            payload: &motion,
            now_ms: 12_000,
        });

        assert_eq!(connected.state, R10BleEsp32c3ProbeBackendState::Discovering);
        assert_eq!(gatt.state, R10BleEsp32c3ProbeBackendState::Subscribing);
        assert_eq!(
            subscribe.state,
            R10BleEsp32c3ProbeBackendState::StartingRemote
        );
        assert_eq!(start.state, R10BleEsp32c3ProbeBackendState::Polling);
        assert_eq!(poll.state, R10BleEsp32c3ProbeBackendState::Polling);
        assert_eq!(notify.state, R10BleEsp32c3ProbeBackendState::Notified);

        assert_eq!(
            notify.record.unwrap().value_for_key("lifecycle"),
            Some("notify")
        );
        assert_eq!(
            notify.record.unwrap().value_for_key("reader"),
            Some("reader_off")
        );
        assert!(!notify.reader_input_enabled);
        assert!(task.bridge().report().connectivity_validated());
        assert_eq!(task.bridge().report().motion_count, 1);
    }

    #[test]
    fn r10_ble_esp32c3_probe_backend_connect_failure_enters_backoff() {
        let mut task = R10BleEsp32c3ProbeBackendTask::<8>::new();

        let failed =
            task.handle_event(R10BleEsp32c3ProbeBackendEvent::ConnectFailed { now_ms: 20_000 });

        assert_eq!(failed.state, R10BleEsp32c3ProbeBackendState::Backoff);
        assert!(failed.state.is_terminal());
        assert_eq!(
            failed.bridge_event.unwrap().stage,
            R10BleDeviceTaskX4ProbeOnlyBridgeStage::ConnectFailed
        );
    }

    #[test]
    fn r10_ble_esp32c3_probe_backend_incomplete_gatt_enters_backoff() {
        let mut task = R10BleEsp32c3ProbeBackendTask::<8>::new();

        let gatt = task.handle_event(R10BleEsp32c3ProbeBackendEvent::GattHandles {
            handles: R10BleGattHandles::unresolved(),
        });

        assert_eq!(gatt.state, R10BleEsp32c3ProbeBackendState::Backoff);
        assert_eq!(
            gatt.bridge_event.unwrap().stage,
            R10BleDeviceTaskX4ProbeOnlyBridgeStage::GattIncomplete
        );
    }

    #[test]
    fn r10_ble_esp32c3_probe_backend_write_failure_enters_backoff() {
        let mut task = R10BleEsp32c3ProbeBackendTask::<8>::new();

        let failed = task.handle_event(R10BleEsp32c3ProbeBackendEvent::SubscribeWritten {
            success: false,
            now_ms: 30_000,
        });

        assert_eq!(failed.state, R10BleEsp32c3ProbeBackendState::Backoff);
        assert_eq!(
            failed.record.unwrap().value_for_key("outcome"),
            Some("failed")
        );
    }

    #[test]
    fn r10_ble_esp32c3_probe_backend_bad_notify_is_logged_not_injected() {
        let mut task = R10BleEsp32c3ProbeBackendTask::<8>::new();
        let _ = task.handle_event(R10BleEsp32c3ProbeBackendEvent::LiveGattContract);

        let bad = [0x02, 0x02, 0, 0];
        let notify = task.handle_event(R10BleEsp32c3ProbeBackendEvent::Notify {
            handle: R10_BLE_LIVE_NOTIFY_VALUE_HANDLE,
            payload: &bad,
            now_ms: 40_000,
        });

        assert_eq!(notify.state, R10BleEsp32c3ProbeBackendState::Notified);
        assert_eq!(
            notify.bridge_event.unwrap().stage,
            R10BleDeviceTaskX4ProbeOnlyBridgeStage::NotifyIgnored
        );
        assert_eq!(
            notify.record.unwrap().value_for_key("outcome"),
            Some("ignored")
        );
        assert_eq!(
            notify.record.unwrap().value_for_key("reader"),
            Some("reader_off")
        );
        assert!(!notify.reader_input_enabled);
    }

    #[test]
    fn r10_ble_esp32c3_probe_backend_complete_is_terminal_without_reader_input() {
        let mut task = R10BleEsp32c3ProbeBackendTask::<8>::new();

        let complete = task.handle_event(R10BleEsp32c3ProbeBackendEvent::Complete);

        assert_eq!(complete.state, R10BleEsp32c3ProbeBackendState::Completed);
        assert!(complete.state.is_terminal());
        assert!(!complete.reader_input_enabled);
    }
}
