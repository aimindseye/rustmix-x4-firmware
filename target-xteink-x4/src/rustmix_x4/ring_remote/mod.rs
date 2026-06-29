//! Ring remote support for Rustmix X4.
//!
//! r1 intentionally contains only transport-independent protocol and policy.
//! BLE central transport is added later so the existing Reader/Wi-Fi/Sleep
//! baseline stays protected while the R10 protocol is validated.

#[cfg(feature = "r10-ble-host")]
pub mod r10_ble_host;
pub mod r10_ble_transport;
pub mod r10_input_bridge;
pub mod r10_protocol;
pub mod r10_remote_policy;

pub use r10_protocol::{
    R10_PACKET_LEN, R10_REMOTE_POLL, R10_REMOTE_START, R10_REMOTE_STOP, R10RemotePacket,
    R10RemotePacketKind,
};
pub use r10_remote_policy::{R10RemoteAction, R10RemoteDebounce, R10RemotePolicy, R10RemoteScreen};

pub use r10_input_bridge::{
    R10_NEXT_PAGE_BUTTON, R10_PREVIOUS_PAGE_BUTTON, R10InputInjection, try_enqueue_remote_action,
};

pub use r10_ble_transport::{
    R10_BLE_DEFAULT_ADVERTISED_NAME, R10_BLE_DEFAULT_NAME_PREFIX, R10_BLE_DEFAULT_TARGET_ADDRESS,
    R10_BLE_NOTIFY_CCCD_DISABLE, R10_BLE_NOTIFY_CCCD_ENABLE, R10_BLE_NOTIFY_UUID,
    R10_BLE_POLL_INTERVAL_MS, R10_BLE_RECONNECT_BACKOFF_MS, R10_BLE_SERVICE_UUID,
    R10_BLE_WRITE_UUID, R10BleAdvertisedDevice, R10BleCommand, R10BleConnectionResult,
    R10BleGattDiscoveryEvent, R10BleGattDiscoveryStatus, R10BleGattHandles, R10BleGattNotifyGate,
    R10BleGattNotifyPolicyResult, R10BleGattOperation, R10BleGattWrite, R10BleGattWriteMode,
    R10BleGattWritePayload, R10BlePendingWrite, R10BleRemoteRuntime, R10BleRemoteSession,
    R10BleRuntimeEffect, R10BleScanDecision, R10BleScanTarget, R10BleTransportState,
    R10BleWritePhase, R10BleWriteResult,
};

#[cfg(feature = "r10-ble-host")]
pub use r10_ble_host::{
    R10_BLE_GATT_RUNTIME_PLAN, R10_BLE_HOST_STATUS, R10_BLE_RUNTIME_PLAN, R10BleController,
    R10BleGattRuntimePlan, R10BleHostBuildStatus, R10BleHostResources, R10BlePacketPool,
    R10BleRuntimeStep, R10BleStack, r10_ble_gatt_runtime_plan, r10_ble_host_contract_summary,
    r10_ble_host_stack_probe_type_names, r10_ble_host_stack_shape_probe,
    r10_ble_host_trait_boundary_probe, r10_ble_runtime_plan,
};
