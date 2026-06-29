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
fn assert_ble_connector_transport<T>()
where
    T: bt_hci::transport::Transport,
{
}

#[cfg(feature = "r10-ble-host")]
pub fn r10_ble_host_trait_boundary_probe() -> (&'static str, &'static str, &'static str) {
    assert_ble_connector_transport::<BleConnector<'static>>();

    (
        core::any::type_name::<BleConnector<'static>>(),
        core::any::type_name::<
            dyn bt_hci::transport::Transport<Error = esp_radio::ble::controller::BleConnectorError>,
        >(),
        core::any::type_name::<trouble_host::central::Central<'static, (), ()>>(),
    )
}

#[cfg(feature = "r10-ble-host")]
pub type R10BleController = bt_hci::controller::Controller<BleConnector<'static>>;

#[cfg(feature = "r10-ble-host")]
pub type R10BlePacketPool = trouble_host::prelude::DefaultPacketPool;

#[cfg(feature = "r10-ble-host")]
pub type R10BleHostResources = trouble_host::HostResources<R10BlePacketPool, 1, 4>;

#[cfg(feature = "r10-ble-host")]
pub type R10BleStack<'stack> = trouble_host::Stack<'stack, R10BleController, R10BlePacketPool>;

#[cfg(feature = "r10-ble-host")]
pub fn r10_ble_host_stack_shape_probe() -> (&'static str, &'static str, &'static str) {
    (
        core::any::type_name::<R10BleController>(),
        core::any::type_name::<R10BleHostResources>(),
        core::any::type_name::<R10BleStack<'static>>(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

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
