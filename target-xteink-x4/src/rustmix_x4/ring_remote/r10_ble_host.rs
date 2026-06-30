// Feature-gated COLMI R10 BLE host integration skeleton.
//
// r3d proves that esp-radio's BLE HCI connector and Trouble's BLE host stack
// can live behind the same feature gate with the same bt-hci version.
// Runtime scan/connect/GATT work comes later.

use super::r10_ble_transport::{
    R10_BLE_NOTIFY_UUID, R10_BLE_SERVICE_UUID, R10_BLE_WRITE_UUID, R10BleCommand,
};

#[cfg(feature = "r10-ble-host")]
use bt_hci as _;

#[cfg(feature = "r10-ble-host")]
use esp_radio::ble::controller::BleConnector;

#[cfg(feature = "r10-ble-host")]
use trouble_host as _;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R10BleHostBuildStatus {
    DisabledInDefaultBuild,
    FeatureGateReady,
}

pub const R10_BLE_HOST_STATUS: R10BleHostBuildStatus = R10BleHostBuildStatus::FeatureGateReady;

pub fn r10_ble_host_contract_summary()
-> (&'static str, &'static str, &'static str, &'static [u8; 16]) {
    (
        R10_BLE_SERVICE_UUID,
        R10_BLE_WRITE_UUID,
        R10_BLE_NOTIFY_UUID,
        R10BleCommand::StartRemote.packet(),
    )
}

#[cfg(feature = "r10-ble-host")]
pub fn r10_ble_host_stack_probe_type_names() -> (&'static str, &'static str, &'static str) {
    (
        core::any::type_name::<BleConnector<'static>>(),
        "bt_hci",
        "trouble_host",
    )
}

#[cfg(feature = "r10-ble-host")]
fn assert_ble_connector_transport() {}

#[cfg(feature = "r10-ble-host")]
pub fn r10_ble_host_trait_boundary_probe() -> (&'static str, &'static str, &'static str) {
    assert_ble_connector_transport();

    (
        core::any::type_name::<BleConnector<'static>>(),
        "bt_hci::transport::Transport",
        "trouble_host::central::Central",
    )
}

#[cfg(feature = "r10-ble-host")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R10BleController;

#[cfg(feature = "r10-ble-host")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R10BlePacketPool;

#[cfg(feature = "r10-ble-host")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R10BleHostResources;

#[cfg(feature = "r10-ble-host")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R10BleStack<'stack> {
    _marker: core::marker::PhantomData<&'stack ()>,
}

#[cfg(feature = "r10-ble-host")]
pub fn r10_ble_host_stack_shape_probe() -> (&'static str, &'static str, &'static str) {
    (
        core::any::type_name::<R10BleController>(),
        core::any::type_name::<R10BleHostResources>(),
        core::any::type_name::<R10BleStack<'static>>(),
    )
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R10BleRuntimeStep {
    BuildController,
    BuildStack,
    SplitStack,
    ScanForRing,
    Connect,
    DiscoverService,
    DiscoverWriteCharacteristic,
    DiscoverNotifyCharacteristic,
    SubscribeNotify,
    StartRemoteMode,
    PollRemoteMode,
    HandleNotify,
    StopRemoteMode,
    BackoffReconnect,
}

pub const R10_BLE_RUNTIME_PLAN: &[R10BleRuntimeStep] = &[
    R10BleRuntimeStep::BuildController,
    R10BleRuntimeStep::BuildStack,
    R10BleRuntimeStep::SplitStack,
    R10BleRuntimeStep::ScanForRing,
    R10BleRuntimeStep::Connect,
    R10BleRuntimeStep::DiscoverService,
    R10BleRuntimeStep::DiscoverWriteCharacteristic,
    R10BleRuntimeStep::DiscoverNotifyCharacteristic,
    R10BleRuntimeStep::SubscribeNotify,
    R10BleRuntimeStep::StartRemoteMode,
    R10BleRuntimeStep::PollRemoteMode,
    R10BleRuntimeStep::HandleNotify,
    R10BleRuntimeStep::StopRemoteMode,
    R10BleRuntimeStep::BackoffReconnect,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R10BleGattRuntimePlan {
    pub service_uuid: &'static str,
    pub write_uuid: &'static str,
    pub notify_uuid: &'static str,
    pub start_command: R10BleCommand,
    pub poll_command: R10BleCommand,
    pub stop_command: R10BleCommand,
    pub poll_interval_ms: u64,
}

pub const R10_BLE_GATT_RUNTIME_PLAN: R10BleGattRuntimePlan = R10BleGattRuntimePlan {
    service_uuid: R10_BLE_SERVICE_UUID,
    write_uuid: R10_BLE_WRITE_UUID,
    notify_uuid: R10_BLE_NOTIFY_UUID,
    start_command: R10BleCommand::StartRemote,
    poll_command: R10BleCommand::PollRemote,
    stop_command: R10BleCommand::StopRemote,
    poll_interval_ms: super::r10_ble_transport::R10_BLE_POLL_INTERVAL_MS,
};

pub const fn r10_ble_runtime_plan() -> &'static [R10BleRuntimeStep] {
    R10_BLE_RUNTIME_PLAN
}

pub const fn r10_ble_gatt_runtime_plan() -> R10BleGattRuntimePlan {
    R10_BLE_GATT_RUNTIME_PLAN
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn r10_ble_runtime_plan_is_ordered_for_safe_ring_remote_startup() {
        let plan = r10_ble_runtime_plan();

        assert_eq!(plan.first(), Some(&R10BleRuntimeStep::BuildController));
        assert_eq!(plan.get(1), Some(&R10BleRuntimeStep::BuildStack));
        assert_eq!(plan.get(2), Some(&R10BleRuntimeStep::SplitStack));

        let subscribe_pos = plan
            .iter()
            .position(|step| *step == R10BleRuntimeStep::SubscribeNotify)
            .unwrap();
        let start_pos = plan
            .iter()
            .position(|step| *step == R10BleRuntimeStep::StartRemoteMode)
            .unwrap();
        let poll_pos = plan
            .iter()
            .position(|step| *step == R10BleRuntimeStep::PollRemoteMode)
            .unwrap();
        let handle_pos = plan
            .iter()
            .position(|step| *step == R10BleRuntimeStep::HandleNotify)
            .unwrap();

        assert!(subscribe_pos < start_pos);
        assert!(start_pos < poll_pos);
        assert!(poll_pos < handle_pos);
        assert_eq!(plan.last(), Some(&R10BleRuntimeStep::BackoffReconnect));
    }

    #[test]
    fn r10_ble_gatt_runtime_plan_uses_colmi_uart_contract() {
        let plan = r10_ble_gatt_runtime_plan();

        assert_eq!(plan.service_uuid, R10_BLE_SERVICE_UUID);
        assert_eq!(plan.write_uuid, R10_BLE_WRITE_UUID);
        assert_eq!(plan.notify_uuid, R10_BLE_NOTIFY_UUID);
        assert_eq!(
            plan.start_command.packet(),
            &[0x02, 0x04, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x06]
        );
        assert_eq!(
            plan.poll_command.packet(),
            &[0x02, 0x05, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x07]
        );
        assert_eq!(
            plan.stop_command.packet(),
            &[0x02, 0x06, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x08]
        );
        assert_eq!(plan.poll_interval_ms, 1_000);
    }

    #[test]
    fn r10_ble_host_contract_summary_uses_transport_contract() {
        let (service, write, notify, start) = r10_ble_host_contract_summary();

        assert_eq!(service, "6e40fff0-b5a3-f393-e0a9-e50e24dcca9e");
        assert_eq!(write, "6e400002-b5a3-f393-e0a9-e50e24dcca9e");
        assert_eq!(notify, "6e400003-b5a3-f393-e0a9-e50e24dcca9e");
        assert_eq!(
            start,
            &[0x02, 0x04, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x06]
        );
    }
}
