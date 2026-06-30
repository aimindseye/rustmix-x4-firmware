# R10 BLE X4 Deploy

BLE remains disabled by default in normal firmware behavior. This deploy path is
explicit and builds with the `r10-ble-host` feature.

## ProbeOnly deploy

```bash
cd /home/mindseye73/Documents/projects/rustmix-x4-firmware

R10_BLE_X4_MODE=probe_only ESPFLASH_PORT=/dev/ttyACM0 ./scripts/x4_r10_ble_probe_deploy.sh
```

## ProbeOnly safety

`probe_only` uses the live COLMI R10 target configuration but keeps Reader input
emission disabled.

## ReaderRemote opt-in

```bash
R10_BLE_X4_MODE=reader_remote R10_BLE_X4_ALLOW_READER_REMOTE=1 ESPFLASH_PORT=/dev/ttyACM0 ./scripts/x4_r10_ble_probe_deploy.sh
```

Use ReaderRemote only after ProbeOnly scan/connect/notify behavior is validated.

## Useful overrides

```bash
R10_BLE_X4_NO_MONITOR=1
R10_BLE_X4_ELF=/path/to/target-xteink-x4
ESPFLASH_PORT=/dev/ttyACM0
```


## Runtime ProbeOnly trigger and serial log formatter

`R10BleDeviceTaskX4RuntimeTrigger::probe_only_deploy_script()` models the X4
runtime path used by the deploy script. It keeps Reader events disabled and
formats monitor-safe serial log metadata for profile, trigger, command,
transcript, and report lines.

The deploy script validates this runtime contract before flashing:

    cargo test -p target-xteink-x4 r10_ble_device_task_x4_runtime -- --nocapture

`ReaderRemote` remains explicit opt-in and is represented by
`reader_remote_manual()` for later hardware validation.


## Runtime serial record renderer

`R10BleDeviceTaskX4RuntimeSerialRecord` converts runtime log-line metadata into
monitor-safe key/value fields without allocation. The record contains:

    event
    kind
    mode
    source
    lifecycle
    command
    outcome
    reader
    status

The deploy script validates this renderer before flashing:

    cargo test -p target-xteink-x4 r10_ble_device_task_x4_serial -- --nocapture

ProbeOnly records remain `reader_off`; ReaderRemote records are `reader_on` only
for the explicit opt-in mode.


## ProbeOnly BLE bridge from scan through notify logs

`R10BleDeviceTaskX4ProbeOnlyBleBridge` is the firmware-side bridge that real BLE
adapter callbacks feed. It records each ProbeOnly stage as no-alloc transcript
entries and r4s serial records:

    start_scan
    target_seen
    connect
    discover
    subscribe
    remote_start
    poll
    notify

ProbeOnly remains `reader_off`; notification packets are classified and logged
but never injected into Reader navigation.

The deploy script validates this bridge before flashing:

    cargo test -p target-xteink-x4 r10_ble_device_task_x4_probe_bridge -- --nocapture

For ProbeOnly deploy builds, `R10_BLE_X4_PROBE_BRIDGE=1` emits the bridge plan
records on boot so the serial monitor shows the active bridge boundary without
enabling ReaderRemote.


## ESP32-C3 ProbeOnly backend task

`R10BleEsp32c3ProbeBackendTask` is the ESP32-C3 ProbeOnly backend controller.
It is callback-fed by the BLE host loop and routes scan, advertisement, connect,
GATT discovery, write result, and notify events into the r4t ProbeOnly bridge.

The backend task remains `reader_off`; it does not inject Reader navigation.
ReaderRemote stays compile-present and explicitly blocked from runtime navigation.

The deploy script validates the backend task before flashing:

    cargo test -p target-xteink-x4 r10_ble_esp32c3_probe_backend -- --nocapture

For ProbeOnly deploy builds, `R10_BLE_X4_BACKEND_TASK=1` emits backend startup
and plan records on boot. This makes the ESP32-C3 backend boundary visible in
the serial monitor without enabling ReaderRemote.


## ESP32-C3 ProbeOnly operation queue

`R10BleEsp32c3ProbeOperationQueue` maps the r4u ProbeOnly backend states to
concrete BLE host operations:

    init_radio
    build_controller
    build_host_stack
    start_scan
    connect_target
    discover_gatt
    subscribe_cccd
    write_remote_start
    write_poll
    await_notify
    backoff

The queue is fixed-capacity and monitor-safe. It remains `reader_off`; it is
used only to prepare the future target-only async BLE runner. ReaderRemote
navigation is still not enabled.

The deploy script validates the queue before flashing:

    cargo test -p target-xteink-x4 r10_ble_esp32c3_probe_operations -- --nocapture

For ProbeOnly deploy builds, `R10_BLE_X4_OPERATION_QUEUE=1` emits the operation
queue plan on boot.


## ESP32-C3 ProbeOnly runner boundary

`R10BleEsp32c3ProbeRunner` consumes the ProbeOnly operation queue and maps
operation success/failure into the backend callback API. It also accepts
advertisement and notify callbacks and routes them into the r4t ProbeOnly bridge.

The runner remains `reader_off`; ReaderRemote navigation is not enabled. The
future target-only async BLE loop should execute the current operation, then
report success, failure, advertisement, or notify events into this runner.

The deploy script validates the runner through the broad ESP32-C3 ProbeOnly
filter:

    cargo test -p target-xteink-x4 r10_ble_esp32c3_probe -- --nocapture

For ProbeOnly deploy builds, `R10_BLE_X4_RUNNER=1` emits runner startup records
on boot.


## ReaderRemote guarded navigation

`R10BleReaderRemoteGuard` enables guarded ReaderRemote navigation only when all
of these conditions pass:

    mode == reader_remote
    explicit ReaderRemote unlock is present
    current screen is Reader
    notify handle matches the R10 notify value handle
    payload is a valid R10 motion packet
    existing R10 debounce accepts the event

ProbeOnly remains log-only and returns `reader_off`. Disabled remains the
default. Accepted ReaderRemote motion maps through the existing R10 input bridge
to the same X4 Reader next-page button event used by the physical page key.

The deploy script validates the guard before flashing:

    cargo test -p target-xteink-x4 r10_ble_reader_remote_guard -- --nocapture

ReaderRemote deployment remains explicitly gated:

    R10_BLE_X4_MODE=reader_remote \
    R10_BLE_X4_ALLOW_READER_REMOTE=1 \
    ESPFLASH_PORT=/dev/ttyACM0 \
    ./scripts/x4_r10_ble_probe_deploy.sh

