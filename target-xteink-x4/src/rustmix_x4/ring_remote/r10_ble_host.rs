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
