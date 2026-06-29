use super::r10_ble_transport::{
    R10_VENDOR_STATUS_7301_PACKET, R10BleAdvertisedDevice, R10BleConnectionResult,
    R10BleGattDiscoveryEvent, R10BleGattDiscoveryStatus, R10BleGattHandles, R10BleGattNotifyGate,
    R10BleRemoteRuntime, R10BleRuntimeEffect, R10BleScanDecision, R10BleScanTarget,
    R10BleWritePhase, R10BleWriteResult,
};

use core::fmt;

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
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::NoEvent => "no_event",
            Self::Motion => "motion",
            Self::VendorStatus7301 => "vendor_status_7301",
            Self::UnknownValid => "unknown_valid",
            Self::BadChecksum => "bad_checksum",
            Self::WrongHandle => "wrong_handle",
            Self::WrongLength => "wrong_length",
        }
    }

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

    pub fn on_link_lost(&mut self, now_ms: u64) -> R10BleRuntimeEffect {
        let effect = self.runtime.link_lost_effect(now_ms);
        self.report
            .record_connection_result(R10BleConnectionResult::LinkLost);
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R10BleProbeEvent<'a> {
    AdvertisedDevice(R10BleAdvertisedDevice<'a>),
    Connected,
    ConnectFailed {
        now_ms: u64,
    },
    LinkLost {
        now_ms: u64,
    },
    Discovery(R10BleGattDiscoveryEvent),
    DiscoveryComplete,
    WriteResult {
        phase: R10BleWritePhase,
        success: bool,
        now_ms: u64,
    },
    Notify {
        handle: u16,
        payload: &'a [u8],
        now_ms: u64,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R10BleProbeOutcome {
    Running,
    Validated,
    TimedOut,
    ConnectFailed,
    LinkLost,
    WriteFailed(R10BleWritePhase),
}

impl R10BleProbeOutcome {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Validated => "validated",
            Self::TimedOut => "timed_out",
            Self::ConnectFailed => "connect_failed",
            Self::LinkLost => "link_lost",
            Self::WriteFailed(_) => "write_failed",
        }
    }

    pub const fn is_terminal(&self) -> bool {
        !matches!(self, Self::Running)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R10BleProbeRunResult {
    pub effect: R10BleRuntimeEffect,
    pub outcome: R10BleProbeOutcome,
    pub report: R10BleProbeReport,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R10BleProbeRunner {
    pub probe: R10BleOnDeviceProbe,
    pub started_at_ms: u64,
    pub deadline_ms: u64,
    pub outcome: R10BleProbeOutcome,
}

impl R10BleProbeRunner {
    pub fn live_r10(started_at_ms: u64) -> Self {
        Self::start(started_at_ms, R10BleProbeConfig::live_r10())
    }

    pub fn start(started_at_ms: u64, config: R10BleProbeConfig) -> Self {
        let mut probe = R10BleOnDeviceProbe::new(config);
        probe.begin_scan();

        Self {
            probe,
            started_at_ms,
            deadline_ms: started_at_ms.saturating_add(config.duration_ms),
            outcome: R10BleProbeOutcome::Running,
        }
    }

    pub fn on_event(&mut self, event: R10BleProbeEvent<'_>) -> R10BleProbeRunResult {
        let mut timed_now = None;

        let effect = match event {
            R10BleProbeEvent::AdvertisedDevice(device) => self.probe.on_advertised_device(device),
            R10BleProbeEvent::Connected => self.probe.on_connected(),
            R10BleProbeEvent::ConnectFailed { now_ms } => {
                timed_now = Some(now_ms);
                self.outcome = R10BleProbeOutcome::ConnectFailed;
                self.probe.on_connect_failed(now_ms)
            }
            R10BleProbeEvent::LinkLost { now_ms } => {
                timed_now = Some(now_ms);
                self.outcome = R10BleProbeOutcome::LinkLost;
                self.probe.on_link_lost(now_ms)
            }
            R10BleProbeEvent::Discovery(event) => self.probe.on_discovery_event(event),
            R10BleProbeEvent::DiscoveryComplete => self.probe.on_discovery_complete(),
            R10BleProbeEvent::WriteResult {
                phase,
                success,
                now_ms,
            } => {
                timed_now = Some(now_ms);

                if !success {
                    self.outcome = R10BleProbeOutcome::WriteFailed(phase);
                }

                self.probe.on_write_result(phase, success, now_ms)
            }
            R10BleProbeEvent::Notify {
                handle,
                payload,
                now_ms,
            } => {
                timed_now = Some(now_ms);
                self.probe.on_notify(handle, payload, now_ms)
            }
        };

        self.update_outcome(timed_now);
        self.result(effect)
    }

    pub fn on_tick(&mut self, now_ms: u64) -> R10BleProbeRunResult {
        self.update_outcome(Some(now_ms));
        self.result(R10BleRuntimeEffect::None)
    }

    fn update_outcome(&mut self, now_ms: Option<u64>) {
        if self.outcome.is_terminal() {
            return;
        }

        if self.probe.report.connectivity_validated() {
            self.outcome = R10BleProbeOutcome::Validated;
            return;
        }

        if let Some(now_ms) = now_ms {
            if now_ms >= self.deadline_ms {
                self.outcome = R10BleProbeOutcome::TimedOut;
            }
        }
    }

    fn result(&self, effect: R10BleRuntimeEffect) -> R10BleProbeRunResult {
        R10BleProbeRunResult {
            effect,
            outcome: self.outcome,
            report: self.probe.report,
        }
    }
}

pub const fn r10_ble_write_phase_label(phase: R10BleWritePhase) -> &'static str {
    match phase {
        R10BleWritePhase::Subscribe => "subscribe",
        R10BleWritePhase::RemoteStart => "remote_start",
        R10BleWritePhase::Poll => "poll",
        R10BleWritePhase::Shutdown => "shutdown",
    }
}

const fn r10_ble_flag(value: bool) -> u8 {
    if value { 1 } else { 0 }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct R10BleProbeLogFormatter;

impl R10BleProbeLogFormatter {
    pub fn write_report_line<W: fmt::Write>(
        out: &mut W,
        outcome: R10BleProbeOutcome,
        report: &R10BleProbeReport,
    ) -> fmt::Result {
        write!(out, "[r10-ble-probe] outcome={}", outcome.as_str())?;

        if let R10BleProbeOutcome::WriteFailed(phase) = outcome {
            write!(out, " write_phase={}", r10_ble_write_phase_label(phase))?;
        }

        write!(
            out,
            " scan={} connected={} gatt={} notify_sub={} remote={} stopped={} polls={} notify={} no_event={} motion={} vendor7301={} unknown={} bad_checksum={} wrong_handle={} wrong_length={}",
            r10_ble_flag(report.scan_matched),
            r10_ble_flag(report.connected),
            r10_ble_flag(report.gatt_ready),
            r10_ble_flag(report.notify_subscribed),
            r10_ble_flag(report.remote_started),
            r10_ble_flag(report.remote_stopped),
            report.polls_written,
            report.notifications_total,
            report.no_event_count,
            report.motion_count,
            report.vendor_status_7301_count,
            report.unknown_valid_count,
            report.bad_checksum_count,
            report.wrong_handle_count,
            report.wrong_length_count,
        )
    }

    pub fn write_run_result_line<W: fmt::Write>(
        out: &mut W,
        result: &R10BleProbeRunResult,
    ) -> fmt::Result {
        Self::write_report_line(out, result.outcome, &result.report)
    }

    pub fn write_notify_line<W: fmt::Write>(
        out: &mut W,
        kind: R10BleProbeNotifyKind,
        report: &R10BleProbeReport,
    ) -> fmt::Result {
        write!(
            out,
            "[r10-ble-probe] notify kind={} total={} motion={} vendor7301={} ignored_valid={}",
            kind.as_str(),
            report.notifications_total,
            report.motion_count,
            report.vendor_status_7301_count,
            r10_ble_flag(kind.is_ignored_valid_packet()),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::super::r10_ble_transport::{
        R10_BLE_DEFAULT_ADVERTISED_NAME, R10_BLE_DEFAULT_TARGET_ADDRESS,
        R10_BLE_LIVE_NOTIFY_CCCD_HANDLE, R10_BLE_LIVE_NOTIFY_VALUE_HANDLE,
        R10_BLE_LIVE_SERVICE_END_HANDLE, R10_BLE_LIVE_SERVICE_START_HANDLE,
        R10_BLE_LIVE_WRITE_VALUE_HANDLE,
    };
    use super::*;

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

#[cfg(test)]
mod runner_tests {
    use super::super::r10_ble_transport::{
        R10_BLE_DEFAULT_ADVERTISED_NAME, R10_BLE_DEFAULT_TARGET_ADDRESS,
        R10_BLE_LIVE_NOTIFY_CCCD_HANDLE, R10_BLE_LIVE_NOTIFY_VALUE_HANDLE,
        R10_BLE_LIVE_SERVICE_END_HANDLE, R10_BLE_LIVE_SERVICE_START_HANDLE,
        R10_BLE_LIVE_WRITE_VALUE_HANDLE, R10BleTransportState,
    };
    use super::*;

    fn live_device<'a>() -> R10BleAdvertisedDevice<'a> {
        R10BleAdvertisedDevice {
            address: Some(R10_BLE_DEFAULT_TARGET_ADDRESS),
            name: Some(R10_BLE_DEFAULT_ADVERTISED_NAME),
        }
    }

    fn feed_live_discovery(runner: &mut R10BleProbeRunner) {
        runner.on_event(R10BleProbeEvent::Discovery(
            R10BleGattDiscoveryEvent::Service {
                start_handle: R10_BLE_LIVE_SERVICE_START_HANDLE,
                end_handle: R10_BLE_LIVE_SERVICE_END_HANDLE,
            },
        ));
        runner.on_event(R10BleProbeEvent::Discovery(
            R10BleGattDiscoveryEvent::WriteCharacteristic {
                value_handle: R10_BLE_LIVE_WRITE_VALUE_HANDLE,
            },
        ));
        runner.on_event(R10BleProbeEvent::Discovery(
            R10BleGattDiscoveryEvent::NotifyCharacteristic {
                value_handle: R10_BLE_LIVE_NOTIFY_VALUE_HANDLE,
            },
        ));
        runner.on_event(R10BleProbeEvent::Discovery(
            R10BleGattDiscoveryEvent::NotifyCccd {
                handle: R10_BLE_LIVE_NOTIFY_CCCD_HANDLE,
            },
        ));
    }

    #[test]
    fn r10_ble_probe_runner_starts_scanning_with_deadline() {
        let runner = R10BleProbeRunner::live_r10(1_000);

        assert_eq!(runner.started_at_ms, 1_000);
        assert_eq!(runner.deadline_ms, 31_000);
        assert_eq!(runner.outcome, R10BleProbeOutcome::Running);
        assert_eq!(
            runner.probe.runtime.session.state,
            R10BleTransportState::Scanning
        );
    }

    #[test]
    fn r10_ble_probe_runner_scan_and_connect_events_update_report() {
        let mut runner = R10BleProbeRunner::live_r10(1_000);

        assert_eq!(
            runner
                .on_event(R10BleProbeEvent::AdvertisedDevice(live_device()))
                .effect,
            R10BleRuntimeEffect::Scan(R10BleScanDecision::MatchByAddress)
        );
        assert!(runner.probe.report.scan_matched);

        assert_eq!(
            runner.on_event(R10BleProbeEvent::Connected).effect,
            R10BleRuntimeEffect::Connection(R10BleConnectionResult::Connected)
        );
        assert!(runner.probe.report.connected);
    }

    #[test]
    fn r10_ble_probe_runner_validates_after_full_probe_flow() {
        let mut runner = R10BleProbeRunner::live_r10(1_000);

        runner.on_event(R10BleProbeEvent::AdvertisedDevice(live_device()));
        runner.on_event(R10BleProbeEvent::Connected);
        feed_live_discovery(&mut runner);
        runner.on_event(R10BleProbeEvent::DiscoveryComplete);
        runner.on_event(R10BleProbeEvent::WriteResult {
            phase: R10BleWritePhase::Subscribe,
            success: true,
            now_ms: 2_000,
        });
        runner.on_event(R10BleProbeEvent::WriteResult {
            phase: R10BleWritePhase::RemoteStart,
            success: true,
            now_ms: 2_100,
        });

        let result = runner.on_event(R10BleProbeEvent::Notify {
            handle: R10_BLE_LIVE_NOTIFY_VALUE_HANDLE,
            payload: &R10_NO_EVENT_PACKET,
            now_ms: 2_200,
        });

        assert_eq!(result.outcome, R10BleProbeOutcome::Validated);
        assert!(result.report.connectivity_validated());
    }

    #[test]
    fn r10_ble_probe_runner_times_out_without_notifications() {
        let mut runner = R10BleProbeRunner::live_r10(1_000);

        let result = runner.on_tick(31_000);

        assert_eq!(result.effect, R10BleRuntimeEffect::None);
        assert_eq!(result.outcome, R10BleProbeOutcome::TimedOut);
    }

    #[test]
    fn r10_ble_probe_runner_tracks_connect_failed() {
        let mut runner = R10BleProbeRunner::live_r10(1_000);

        let result = runner.on_event(R10BleProbeEvent::ConnectFailed { now_ms: 2_000 });

        assert_eq!(result.outcome, R10BleProbeOutcome::ConnectFailed);
        assert!(!result.report.connected);
    }

    #[test]
    fn r10_ble_probe_runner_tracks_write_failure() {
        let mut runner = R10BleProbeRunner::live_r10(1_000);

        let result = runner.on_event(R10BleProbeEvent::WriteResult {
            phase: R10BleWritePhase::RemoteStart,
            success: false,
            now_ms: 2_000,
        });

        assert_eq!(
            result.outcome,
            R10BleProbeOutcome::WriteFailed(R10BleWritePhase::RemoteStart)
        );
        assert!(!result.report.remote_started);
    }
}

#[cfg(test)]
mod log_formatter_tests {
    use super::*;
    use std::string::String;

    fn sample_valid_report() -> R10BleProbeReport {
        R10BleProbeReport {
            scan_matched: true,
            connected: true,
            gatt_ready: true,
            notify_subscribed: true,
            remote_started: true,
            remote_stopped: false,
            polls_written: 2,
            notifications_total: 3,
            no_event_count: 1,
            motion_count: 1,
            vendor_status_7301_count: 1,
            unknown_valid_count: 0,
            bad_checksum_count: 0,
            wrong_handle_count: 0,
            wrong_length_count: 0,
        }
    }

    #[test]
    fn r10_ble_probe_log_labels_outcomes_and_write_phases() {
        assert_eq!(R10BleProbeOutcome::Running.as_str(), "running");
        assert_eq!(R10BleProbeOutcome::Validated.as_str(), "validated");
        assert_eq!(R10BleProbeOutcome::TimedOut.as_str(), "timed_out");
        assert_eq!(R10BleProbeOutcome::ConnectFailed.as_str(), "connect_failed");
        assert_eq!(R10BleProbeOutcome::LinkLost.as_str(), "link_lost");
        assert_eq!(
            R10BleProbeOutcome::WriteFailed(R10BleWritePhase::RemoteStart).as_str(),
            "write_failed"
        );

        assert_eq!(
            r10_ble_write_phase_label(R10BleWritePhase::Subscribe),
            "subscribe"
        );
        assert_eq!(
            r10_ble_write_phase_label(R10BleWritePhase::RemoteStart),
            "remote_start"
        );
        assert_eq!(r10_ble_write_phase_label(R10BleWritePhase::Poll), "poll");
        assert_eq!(
            r10_ble_write_phase_label(R10BleWritePhase::Shutdown),
            "shutdown"
        );
    }

    #[test]
    fn r10_ble_probe_log_labels_notify_kinds() {
        assert_eq!(R10BleProbeNotifyKind::NoEvent.as_str(), "no_event");
        assert_eq!(R10BleProbeNotifyKind::Motion.as_str(), "motion");
        assert_eq!(
            R10BleProbeNotifyKind::VendorStatus7301.as_str(),
            "vendor_status_7301"
        );
        assert_eq!(
            R10BleProbeNotifyKind::UnknownValid.as_str(),
            "unknown_valid"
        );
        assert_eq!(R10BleProbeNotifyKind::BadChecksum.as_str(), "bad_checksum");
        assert_eq!(R10BleProbeNotifyKind::WrongHandle.as_str(), "wrong_handle");
        assert_eq!(R10BleProbeNotifyKind::WrongLength.as_str(), "wrong_length");
    }

    #[test]
    fn r10_ble_probe_log_report_line_is_compact_and_monitor_safe() {
        let mut line = String::new();

        R10BleProbeLogFormatter::write_report_line(
            &mut line,
            R10BleProbeOutcome::Validated,
            &sample_valid_report(),
        )
        .unwrap();

        assert_eq!(
            line,
            "[r10-ble-probe] outcome=validated scan=1 connected=1 gatt=1 notify_sub=1 remote=1 stopped=0 polls=2 notify=3 no_event=1 motion=1 vendor7301=1 unknown=0 bad_checksum=0 wrong_handle=0 wrong_length=0"
        );
    }

    #[test]
    fn r10_ble_probe_log_report_line_includes_failed_write_phase() {
        let mut line = String::new();

        R10BleProbeLogFormatter::write_report_line(
            &mut line,
            R10BleProbeOutcome::WriteFailed(R10BleWritePhase::RemoteStart),
            &R10BleProbeReport::default(),
        )
        .unwrap();

        assert!(line.starts_with("[r10-ble-probe] outcome=write_failed write_phase=remote_start"));
        assert!(line.contains("scan=0 connected=0 gatt=0"));
    }

    #[test]
    fn r10_ble_probe_log_run_result_line_reuses_report_formatter() {
        let result = R10BleProbeRunResult {
            effect: R10BleRuntimeEffect::None,
            outcome: R10BleProbeOutcome::Validated,
            report: sample_valid_report(),
        };
        let mut line = String::new();

        R10BleProbeLogFormatter::write_run_result_line(&mut line, &result).unwrap();

        assert!(line.contains("outcome=validated"));
        assert!(line.contains("notify=3"));
        assert!(line.contains("motion=1"));
        assert!(line.contains("vendor7301=1"));
    }

    #[test]
    fn r10_ble_probe_log_notify_line_marks_ignored_vendor_status() {
        let mut line = String::new();

        R10BleProbeLogFormatter::write_notify_line(
            &mut line,
            R10BleProbeNotifyKind::VendorStatus7301,
            &sample_valid_report(),
        )
        .unwrap();

        assert_eq!(
            line,
            "[r10-ble-probe] notify kind=vendor_status_7301 total=3 motion=1 vendor7301=1 ignored_valid=1"
        );
    }
}
