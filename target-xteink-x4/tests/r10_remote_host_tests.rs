#![allow(dead_code)]

mod rustmix_x4 {
    pub mod contracts {
        pub mod input_semantics {
            #[derive(Clone, Copy, Debug, Eq, PartialEq)]
            pub enum RustmixReaderAction {
                BackToLibrary,
                OpenOrSelect,
                NextPage,
                PreviousPage,
                BookmarkOrMenu,
                Unsupported,
            }
        }
    }

    pub mod x4_kernel {
        pub mod board {
            pub mod button {
                #[derive(Clone, Copy, Debug, Eq, PartialEq)]
                pub enum Button {
                    VolDown,
                    VolUp,
                    Right,
                    Left,
                    Confirm,
                    Back,
                    Power,
                }
            }
        }

        pub mod drivers {
            pub mod input {
                use crate::rustmix_x4::x4_kernel::board::button::Button;

                #[derive(Clone, Copy, Debug, Eq, PartialEq)]
                pub enum Event {
                    Press(Button),
                    Release(Button),
                    LongPress(Button),
                    Repeat(Button),
                }
            }
        }

        pub mod kernel {
            pub mod tasks {
                use crate::rustmix_x4::x4_kernel::drivers::input::Event;

                pub struct DummyInputEvents;

                pub static INPUT_EVENTS: DummyInputEvents = DummyInputEvents;

                impl DummyInputEvents {
                    pub fn try_send(&self, _event: Event) -> Result<(), ()> {
                        Ok(())
                    }
                }
            }
        }
    }

    pub mod ring_remote {
        pub mod r10_ble_probe {
            include!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/src/rustmix_x4/ring_remote/r10_ble_probe.rs"
            ));
        }

        pub mod r10_protocol {
            include!("../src/rustmix_x4/ring_remote/r10_protocol.rs");
        }

        pub mod r10_remote_policy {
            include!("../src/rustmix_x4/ring_remote/r10_remote_policy.rs");
        }

        pub mod r10_input_bridge {
            include!("../src/rustmix_x4/ring_remote/r10_input_bridge.rs");
        }

        pub mod r10_ble_transport {
            include!("../src/rustmix_x4/ring_remote/r10_ble_transport.rs");
        }
        pub mod r10_ble_device_task {
            include!("../src/rustmix_x4/ring_remote/r10_ble_device_task.rs");
        }
        pub mod r10_ble_esp32c3_backend {
            include!("../src/rustmix_x4/ring_remote/r10_ble_esp32c3_backend.rs");
        }
        pub mod r10_ble_esp32c3_operations {
            include!("../src/rustmix_x4/ring_remote/r10_ble_esp32c3_operations.rs");
        }
    }
}

use rustmix_x4::contracts::input_semantics::RustmixReaderAction;
use rustmix_x4::ring_remote::r10_protocol::{
    R10_REMOTE_POLL, R10_REMOTE_START, R10_REMOTE_STOP, R10RemotePacket, R10RemotePacketKind,
};

use rustmix_x4::ring_remote::r10_input_bridge::R10InputInjection;

use rustmix_x4::ring_remote::r10_ble_transport::{
    R10_BLE_NOTIFY_UUID, R10_BLE_SERVICE_UUID, R10_BLE_WRITE_UUID, R10BleCommand,
    R10BleRemoteSession,
};
use rustmix_x4::ring_remote::r10_remote_policy::{
    R10RemoteAction, R10RemotePolicy, R10RemoteScreen,
};
use rustmix_x4::x4_kernel::board::button::Button;
use rustmix_x4::x4_kernel::drivers::input::Event;

#[test]
fn r10_host_known_packets_match_stock_protocol() {
    assert_eq!(
        R10_REMOTE_START.bytes(),
        [0x02, 0x04, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x06]
    );

    assert_eq!(
        R10_REMOTE_POLL.bytes(),
        [0x02, 0x05, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x07]
    );

    assert_eq!(
        R10_REMOTE_STOP.bytes(),
        [0x02, 0x06, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x08]
    );
}

#[test]
fn r10_host_motion_packet_becomes_next_page() {
    let motion = [0x02, 0x02, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x04];

    assert_eq!(
        R10RemotePacket::classify(&motion),
        R10RemotePacketKind::RemoteMotion
    );

    let mut policy = R10RemotePolicy::enabled_for_reader(3500);
    assert_eq!(
        policy.handle_packet(10_000, &motion),
        R10RemoteAction::Reader(RustmixReaderAction::NextPage)
    );
}

#[test]
fn r10_host_motion_is_reader_only_and_debounced() {
    let motion = [0x02, 0x02, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x04];

    let mut policy = R10RemotePolicy::enabled_for_reader(3500);
    assert_eq!(
        policy.handle_packet(10_000, &motion),
        R10RemoteAction::Reader(RustmixReaderAction::NextPage)
    );
    assert_eq!(policy.handle_packet(11_000, &motion), R10RemoteAction::None);
    assert_eq!(
        policy.handle_packet(14_000, &motion),
        R10RemoteAction::Reader(RustmixReaderAction::NextPage)
    );

    let mut settings_policy = R10RemotePolicy {
        enabled: true,
        screen: R10RemoteScreen::Settings,
        debounce: policy.debounce,
    };
    assert_eq!(
        settings_policy.handle_packet(20_000, &motion),
        R10RemoteAction::None
    );
}

#[test]
fn r10_host_next_page_maps_to_existing_x4_next_button() {
    assert_eq!(
        R10InputInjection::from_remote_action(R10RemoteAction::Reader(
            RustmixReaderAction::NextPage
        )),
        R10InputInjection::Press(Event::Press(Button::VolDown))
    );
}

#[test]
fn r10_host_previous_page_maps_to_existing_x4_previous_button() {
    assert_eq!(
        R10InputInjection::from_remote_action(R10RemoteAction::Reader(
            RustmixReaderAction::PreviousPage
        )),
        R10InputInjection::Press(Event::Press(Button::VolUp))
    );
}

#[test]
fn r10_host_ble_uuid_contract_matches_stock_gatt() {
    assert_eq!(R10_BLE_SERVICE_UUID, "6e40fff0-b5a3-f393-e0a9-e50e24dcca9e");
    assert_eq!(R10_BLE_WRITE_UUID, "6e400002-b5a3-f393-e0a9-e50e24dcca9e");
    assert_eq!(R10_BLE_NOTIFY_UUID, "6e400003-b5a3-f393-e0a9-e50e24dcca9e");
}

#[test]
fn r10_host_ble_commands_match_stock_packets() {
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
fn r10_host_ble_notify_motion_uses_existing_policy() {
    let motion = [0x02, 0x02, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x04];

    let mut session = R10BleRemoteSession::reader_remote(3500);
    assert_eq!(
        session.on_notify(10_000, &motion),
        R10RemoteAction::Reader(RustmixReaderAction::NextPage)
    );
    assert_eq!(session.on_notify(11_000, &motion), R10RemoteAction::None);
}

mod r10_ble_on_device_probe_host_tests {
    use super::rustmix_x4::ring_remote::r10_ble_probe::{
        R10_BLE_PROBE_STEPS, R10BleOnDeviceProbe, R10BleProbeConfig, R10BleProbeNotifyKind,
        R10BleProbeReport, R10BleProbeStep, classify_accepted_packet, packet_checksum_ok,
    };
    use super::rustmix_x4::ring_remote::r10_ble_transport::{
        R10_BLE_DEFAULT_ADVERTISED_NAME, R10_BLE_DEFAULT_TARGET_ADDRESS,
        R10_BLE_LIVE_NOTIFY_CCCD_HANDLE, R10_BLE_LIVE_NOTIFY_VALUE_HANDLE,
        R10_BLE_LIVE_SERVICE_END_HANDLE, R10_BLE_LIVE_SERVICE_START_HANDLE,
        R10_BLE_LIVE_WRITE_VALUE_HANDLE, R10_VENDOR_STATUS_7301_PACKET, R10BleAdvertisedDevice,
        R10BleConnectionResult, R10BleGattDiscoveryEvent, R10BleGattDiscoveryStatus,
        R10BleGattHandles, R10BleRuntimeEffect, R10BleScanDecision, R10BleTransportState,
        R10BleWritePhase, R10BleWriteResult,
    };

    const R10_NO_EVENT_PACKET: [u8; 16] = [0x02, 0x00, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x02];
    const R10_MOTION_PACKET: [u8; 16] = [0x02, 0x02, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x04];

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
    fn r10_ble_probe_host_step_order_is_full_connectivity_script() {
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
    fn r10_ble_probe_host_live_config_targets_known_ring() {
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
    fn r10_ble_probe_host_classifies_no_event_motion_vendor_and_bad_checksum() {
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
        assert!(packet_checksum_ok(&R10_VENDOR_STATUS_7301_PACKET));

        let mut bad = R10_MOTION_PACKET;
        bad[15] = 0x00;
        assert_eq!(
            classify_accepted_packet(bad),
            R10BleProbeNotifyKind::BadChecksum
        );
    }

    #[test]
    fn r10_ble_probe_host_report_validates_after_connect_gatt_subscribe_start_notify() {
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
    fn r10_ble_probe_host_scan_match_moves_runtime_to_connecting() {
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
            R10BleTransportState::Connecting
        );
    }

    #[test]
    fn r10_ble_probe_host_connected_and_discovery_ready_records_gatt() {
        let mut probe = R10BleOnDeviceProbe::live_r10();

        assert_eq!(
            probe.on_connected(),
            R10BleRuntimeEffect::Connection(R10BleConnectionResult::Connected)
        );
        assert!(probe.report.connected);

        apply_live_discovery(&mut probe);

        assert!(matches!(
            probe.on_discovery_complete(),
            R10BleRuntimeEffect::Discovery(R10BleGattDiscoveryStatus::Ready(_))
        ));
        assert!(probe.report.gatt_ready);
    }

    #[test]
    fn r10_ble_probe_host_write_results_track_subscribe_start_poll_shutdown() {
        let mut probe = R10BleOnDeviceProbe::live_r10();

        assert_eq!(
            probe.on_write_result(R10BleWritePhase::Subscribe, true, 10_000),
            R10BleRuntimeEffect::WriteResult(R10BleWriteResult::Success(
                R10BleWritePhase::Subscribe
            ))
        );
        assert!(probe.report.notify_subscribed);

        probe.on_write_result(R10BleWritePhase::RemoteStart, true, 10_000);
        probe.on_write_result(R10BleWritePhase::Poll, true, 11_000);
        probe.on_write_result(R10BleWritePhase::Shutdown, true, 20_000);

        assert!(!probe.report.remote_started);
        assert!(probe.report.remote_stopped);
        assert_eq!(probe.report.polls_written, 1);
    }

    #[test]
    fn r10_ble_probe_host_notify_records_motion_and_vendor_ignored() {
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
