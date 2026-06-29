use super::r10_ble_transport::{
    R10_VENDOR_STATUS_7301_PACKET, R10BleAdvertisedDevice, R10BleConnectionResult,
    R10BleGattDiscoveryEvent, R10BleGattDiscoveryStatus, R10BleGattHandles, R10BleGattNotifyGate,
    R10BleRemoteRuntime, R10BleRuntimeEffect, R10BleScanDecision, R10BleScanTarget,
    R10BleWritePhase, R10BleWriteResult,
};

pub const R10_BLE_PROBE_DEFAULT_DURATION_MS: u64 = 30_000;
pub const R10_BLE_PROBE_DEFAULT_DEBOUNCE_MS: u64 = 3_500;

const R10_NO_EVENT_PACKET: [u8; 16] = [0x02, 0x00, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x02];
const R10_MOTION_PACKET: [u8; 16] = [0x02, 0x02, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x04];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R10BleProbeStep {
    Scan,
    Connect,
    DiscoverGatt,
    SubscribeNotify,
    StartRemote,
    PollRemote,
    ObserveNotify,
    StopRemote,
    Disconnect,
}

pub const R10_BLE_PROBE_STEPS: [R10BleProbeStep; 9] = [
    R10BleProbeStep::Scan,
    R10BleProbeStep::Connect,
    R10BleProbeStep::DiscoverGatt,
    R10BleProbeStep::SubscribeNotify,
    R10BleProbeStep::StartRemote,
    R10BleProbeStep::PollRemote,
    R10BleProbeStep::ObserveNotify,
    R10BleProbeStep::StopRemote,
    R10BleProbeStep::Disconnect,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R10BleProbeConfig {
    pub target: R10BleScanTarget,
    pub duration_ms: u64,
    pub debounce_ms: u64,
}

impl R10BleProbeConfig {
    pub const fn live_r10() -> Self {
        Self {
            target: R10BleScanTarget::default_r10(),
            duration_ms: R10_BLE_PROBE_DEFAULT_DURATION_MS,
            debounce_ms: R10_BLE_PROBE_DEFAULT_DEBOUNCE_MS,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R10BleProbeNotifyKind {
    NoEvent,
    Motion,
    VendorStatus7301,
    UnknownValid,
    BadChecksum,
    WrongHandle,
    WrongLength,
}

impl R10BleProbeNotifyKind {
    pub const fn is_motion(&self) -> bool {
        matches!(self, Self::Motion)
    }

    pub const fn is_ignored_valid_packet(&self) -> bool {
        matches!(
            self,
            Self::NoEvent | Self::VendorStatus7301 | Self::UnknownValid
        )
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct R10BleProbeReport {
    pub scan_matched: bool,
    pub connected: bool,
    pub gatt_ready: bool,
    pub notify_subscribed: bool,
    pub remote_started: bool,
    pub remote_stopped: bool,
    pub polls_written: u32,
    pub notifications_total: u32,
    pub no_event_count: u32,
    pub motion_count: u32,
    pub vendor_status_7301_count: u32,
    pub unknown_valid_count: u32,
    pub bad_checksum_count: u32,
    pub wrong_handle_count: u32,
    pub wrong_length_count: u32,
}

impl R10BleProbeReport {
    pub fn record_scan_decision(&mut self, decision: R10BleScanDecision) {
        if decision.is_match() {
            self.scan_matched = true;
        }
    }

    pub fn record_connection_result(&mut self, result: R10BleConnectionResult) {
        match result {
            R10BleConnectionResult::Connected => self.connected = true,
            R10BleConnectionResult::ConnectFailed | R10BleConnectionResult::LinkLost => {
                self.connected = false;
                self.notify_subscribed = false;
                self.remote_started = false;
            }
        }
    }

    pub fn record_discovery_status(&mut self, status: R10BleGattDiscoveryStatus) {
        self.gatt_ready = status.is_ready();
    }

    pub fn record_write_result(&mut self, result: R10BleWriteResult) {
        match result {
            R10BleWriteResult::Success(R10BleWritePhase::Subscribe) => {
                self.notify_subscribed = true;
            }
            R10BleWriteResult::Success(R10BleWritePhase::RemoteStart) => {
                self.remote_started = true;
            }
            R10BleWriteResult::Success(R10BleWritePhase::Poll) => {
                self.polls_written = self.polls_written.saturating_add(1);
            }
            R10BleWriteResult::Success(R10BleWritePhase::Shutdown) => {
                self.remote_stopped = true;
                self.remote_started = false;
            }
            R10BleWriteResult::Failed(_) => {
                self.connected = false;
                self.notify_subscribed = false;
                self.remote_started = false;
            }
        }
    }

    pub fn record_notify_kind(&mut self, kind: R10BleProbeNotifyKind) {
        self.notifications_total = self.notifications_total.saturating_add(1);

        match kind {
            R10BleProbeNotifyKind::NoEvent => {
                self.no_event_count = self.no_event_count.saturating_add(1);
            }
            R10BleProbeNotifyKind::Motion => {
                self.motion_count = self.motion_count.saturating_add(1);
            }
            R10BleProbeNotifyKind::VendorStatus7301 => {
                self.vendor_status_7301_count = self.vendor_status_7301_count.saturating_add(1);
            }
            R10BleProbeNotifyKind::UnknownValid => {
                self.unknown_valid_count = self.unknown_valid_count.saturating_add(1);
            }
            R10BleProbeNotifyKind::BadChecksum => {
                self.bad_checksum_count = self.bad_checksum_count.saturating_add(1);
            }
            R10BleProbeNotifyKind::WrongHandle => {
                self.wrong_handle_count = self.wrong_handle_count.saturating_add(1);
            }
            R10BleProbeNotifyKind::WrongLength => {
                self.wrong_length_count = self.wrong_length_count.saturating_add(1);
            }
        }
    }

    pub const fn connectivity_validated(&self) -> bool {
        self.scan_matched
            && self.connected
            && self.gatt_ready
            && self.notify_subscribed
            && self.remote_started
            && self.notifications_total > 0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R10BleOnDeviceProbe {
    pub config: R10BleProbeConfig,
    pub runtime: R10BleRemoteRuntime,
    pub report: R10BleProbeReport,
}

impl R10BleOnDeviceProbe {
    pub fn live_r10() -> Self {
        Self::new(R10BleProbeConfig::live_r10())
    }

    pub fn new(config: R10BleProbeConfig) -> Self {
        Self {
            runtime: R10BleRemoteRuntime::reader_remote(config.debounce_ms),
            config,
            report: R10BleProbeReport::default(),
        }
    }

    pub fn begin_scan(&mut self) {
        self.runtime.session.begin_scan();
    }

    pub fn on_advertised_device(
        &mut self,
        device: R10BleAdvertisedDevice<'_>,
    ) -> R10BleRuntimeEffect {
        let effect = self.runtime.scan_device_effect(self.config.target, device);

        if let R10BleRuntimeEffect::Scan(decision) = effect {
            self.report.record_scan_decision(decision);
        }

        effect
    }

    pub fn on_connected(&mut self) -> R10BleRuntimeEffect {
        let effect = self.runtime.connect_success_effect();
        self.report
            .record_connection_result(R10BleConnectionResult::Connected);
        effect
    }

    pub fn on_connect_failed(&mut self, now_ms: u64) -> R10BleRuntimeEffect {
        let effect = self.runtime.connect_failed_effect(now_ms);
        self.report
            .record_connection_result(R10BleConnectionResult::ConnectFailed);
        effect
    }

    pub fn on_discovery_event(&mut self, event: R10BleGattDiscoveryEvent) -> R10BleRuntimeEffect {
        self.runtime.discovery_event_effect(event)
    }

    pub fn on_discovery_complete(&mut self) -> R10BleRuntimeEffect {
        let effect = self.runtime.discovery_complete_effect();

        if let R10BleRuntimeEffect::Discovery(status) = effect {
            self.report.record_discovery_status(status);
        }

        effect
    }

    pub fn on_write_result(
        &mut self,
        phase: R10BleWritePhase,
        success: bool,
        now_ms: u64,
    ) -> R10BleRuntimeEffect {
        let effect = if success {
            self.runtime.write_success_effect(phase, now_ms)
        } else {
            self.runtime.write_failed_effect(phase, now_ms)
        };

        if let R10BleRuntimeEffect::WriteResult(result) = effect {
            self.report.record_write_result(result);
        }

        effect
    }

    pub fn on_notify(&mut self, handle: u16, payload: &[u8], now_ms: u64) -> R10BleRuntimeEffect {
        let kind = classify_notify(&self.runtime.handles, handle, payload);
        self.report.record_notify_kind(kind);
        self.runtime.notify_effect(handle, payload, now_ms)
    }
}

pub fn classify_notify(
    handles: &R10BleGattHandles,
    handle: u16,
    payload: &[u8],
) -> R10BleProbeNotifyKind {
    match handles.gate_notify(handle, payload) {
        R10BleGattNotifyGate::Accepted(packet) => classify_accepted_packet(packet),
        R10BleGattNotifyGate::WrongHandle { .. } => R10BleProbeNotifyKind::WrongHandle,
        R10BleGattNotifyGate::WrongLength { .. } => R10BleProbeNotifyKind::WrongLength,
    }
}

pub fn classify_accepted_packet(packet: [u8; 16]) -> R10BleProbeNotifyKind {
    if !packet_checksum_ok(&packet) {
        return R10BleProbeNotifyKind::BadChecksum;
    }

    if packet == R10_NO_EVENT_PACKET {
        return R10BleProbeNotifyKind::NoEvent;
    }

    if packet == R10_MOTION_PACKET {
        return R10BleProbeNotifyKind::Motion;
    }

    if packet == R10_VENDOR_STATUS_7301_PACKET {
        return R10BleProbeNotifyKind::VendorStatus7301;
    }

    R10BleProbeNotifyKind::UnknownValid
}

pub fn packet_checksum_ok(packet: &[u8; 16]) -> bool {
    packet[15]
        == packet[..15]
            .iter()
            .fold(0u8, |acc, byte| acc.wrapping_add(*byte))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rustmix_x4::ring_remote::{
        R10_BLE_DEFAULT_ADVERTISED_NAME, R10_BLE_DEFAULT_TARGET_ADDRESS,
        R10_BLE_LIVE_NOTIFY_CCCD_HANDLE, R10_BLE_LIVE_NOTIFY_VALUE_HANDLE,
        R10_BLE_LIVE_SERVICE_END_HANDLE, R10_BLE_LIVE_SERVICE_START_HANDLE,
        R10_BLE_LIVE_WRITE_VALUE_HANDLE,
    };

    fn live_handles() -> R10BleGattHandles {
        R10BleGattHandles::new(
            R10_BLE_LIVE_SERVICE_START_HANDLE,
            R10_BLE_LIVE_SERVICE_END_HANDLE,
            R10_BLE_LIVE_WRITE_VALUE_HANDLE,
            R10_BLE_LIVE_NOTIFY_VALUE_HANDLE,
            R10_BLE_LIVE_NOTIFY_CCCD_HANDLE,
        )
    }

    fn apply_live_discovery(probe: &mut R10BleOnDeviceProbe) {
        probe.on_discovery_event(R10BleGattDiscoveryEvent::Service {
            start_handle: R10_BLE_LIVE_SERVICE_START_HANDLE,
            end_handle: R10_BLE_LIVE_SERVICE_END_HANDLE,
        });
        probe.on_discovery_event(R10BleGattDiscoveryEvent::WriteCharacteristic {
            value_handle: R10_BLE_LIVE_WRITE_VALUE_HANDLE,
        });
        probe.on_discovery_event(R10BleGattDiscoveryEvent::NotifyCharacteristic {
            value_handle: R10_BLE_LIVE_NOTIFY_VALUE_HANDLE,
        });
        probe.on_discovery_event(R10BleGattDiscoveryEvent::NotifyCccd {
            handle: R10_BLE_LIVE_NOTIFY_CCCD_HANDLE,
        });
    }

    #[test]
    fn r10_ble_probe_step_order_is_full_connectivity_script() {
        assert_eq!(
            R10_BLE_PROBE_STEPS,
            [
                R10BleProbeStep::Scan,
                R10BleProbeStep::Connect,
                R10BleProbeStep::DiscoverGatt,
                R10BleProbeStep::SubscribeNotify,
                R10BleProbeStep::StartRemote,
                R10BleProbeStep::PollRemote,
                R10BleProbeStep::ObserveNotify,
                R10BleProbeStep::StopRemote,
                R10BleProbeStep::Disconnect,
            ]
        );
    }

    #[test]
    fn r10_ble_probe_live_config_targets_known_ring() {
        let config = R10BleProbeConfig::live_r10();

        assert_eq!(config.duration_ms, 30_000);
        assert_eq!(config.debounce_ms, 3_500);
        assert_eq!(
            config.target.scan_device(R10BleAdvertisedDevice {
                address: Some(R10_BLE_DEFAULT_TARGET_ADDRESS),
                name: Some(R10_BLE_DEFAULT_ADVERTISED_NAME),
            }),
            R10BleScanDecision::MatchByAddress
        );
    }

    #[test]
    fn r10_ble_probe_classifies_no_event_motion_vendor_and_bad_checksum() {
        assert_eq!(
            classify_accepted_packet(R10_NO_EVENT_PACKET),
            R10BleProbeNotifyKind::NoEvent
        );
        assert_eq!(
            classify_accepted_packet(R10_MOTION_PACKET),
            R10BleProbeNotifyKind::Motion
        );
        assert_eq!(
            classify_accepted_packet(R10_VENDOR_STATUS_7301_PACKET),
            R10BleProbeNotifyKind::VendorStatus7301
        );

        let mut bad = R10_MOTION_PACKET;
        bad[15] = 0x00;
        assert_eq!(
            classify_accepted_packet(bad),
            R10BleProbeNotifyKind::BadChecksum
        );
    }

    #[test]
    fn r10_ble_probe_report_validates_after_connect_gatt_subscribe_start_notify() {
        let mut report = R10BleProbeReport::default();

        report.record_scan_decision(R10BleScanDecision::MatchByAddress);
        report.record_connection_result(R10BleConnectionResult::Connected);
        report.record_discovery_status(live_handles().discovery_status());
        report.record_write_result(R10BleWriteResult::Success(R10BleWritePhase::Subscribe));
        report.record_write_result(R10BleWriteResult::Success(R10BleWritePhase::RemoteStart));
        report.record_notify_kind(R10BleProbeNotifyKind::NoEvent);

        assert!(report.connectivity_validated());
    }

    #[test]
    fn r10_ble_on_device_probe_scan_match_moves_runtime_to_connecting() {
        let mut probe = R10BleOnDeviceProbe::live_r10();
        probe.begin_scan();

        assert_eq!(
            probe.on_advertised_device(R10BleAdvertisedDevice {
                address: Some(R10_BLE_DEFAULT_TARGET_ADDRESS),
                name: Some(R10_BLE_DEFAULT_ADVERTISED_NAME),
            }),
            R10BleRuntimeEffect::Scan(R10BleScanDecision::MatchByAddress)
        );

        assert!(probe.report.scan_matched);
        assert_eq!(
            probe.runtime.session.state,
            super::super::r10_ble_transport::R10BleTransportState::Connecting
        );
    }

    #[test]
    fn r10_ble_on_device_probe_connected_and_discovery_ready_records_gatt() {
        let mut probe = R10BleOnDeviceProbe::live_r10();

        assert!(matches!(
            probe.on_connected(),
            R10BleRuntimeEffect::Connection(R10BleConnectionResult::Connected)
        ));
        assert!(probe.report.connected);

        apply_live_discovery(&mut probe);

        assert!(matches!(
            probe.on_discovery_complete(),
            R10BleRuntimeEffect::Discovery(R10BleGattDiscoveryStatus::Ready(_))
        ));
        assert!(probe.report.gatt_ready);
    }

    #[test]
    fn r10_ble_on_device_probe_write_results_track_subscribe_start_poll_shutdown() {
        let mut probe = R10BleOnDeviceProbe::live_r10();

        assert!(matches!(
            probe.on_write_result(R10BleWritePhase::Subscribe, true, 10_000),
            R10BleRuntimeEffect::WriteResult(R10BleWriteResult::Success(
                R10BleWritePhase::Subscribe
            ))
        ));
        assert!(probe.report.notify_subscribed);

        probe.on_write_result(R10BleWritePhase::RemoteStart, true, 10_000);
        probe.on_write_result(R10BleWritePhase::Poll, true, 11_000);
        probe.on_write_result(R10BleWritePhase::Shutdown, true, 20_000);

        assert!(!probe.report.remote_started);
        assert!(probe.report.remote_stopped);
        assert_eq!(probe.report.polls_written, 1);
    }

    #[test]
    fn r10_ble_on_device_probe_notify_records_motion_and_vendor_ignored() {
        let mut probe = R10BleOnDeviceProbe::live_r10();
        probe.runtime.handles = live_handles();

        probe.on_notify(R10_BLE_LIVE_NOTIFY_VALUE_HANDLE, &R10_MOTION_PACKET, 10_000);
        probe.on_notify(
            R10_BLE_LIVE_NOTIFY_VALUE_HANDLE,
            &R10_VENDOR_STATUS_7301_PACKET,
            11_000,
        );

        assert_eq!(probe.report.notifications_total, 2);
        assert_eq!(probe.report.motion_count, 1);
        assert_eq!(probe.report.vendor_status_7301_count, 1);
        assert!(R10BleProbeNotifyKind::VendorStatus7301.is_ignored_valid_packet());
    }
}
