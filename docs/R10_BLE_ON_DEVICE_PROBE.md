# R10 BLE On-Device Probe Mode

This document describes the firmware-side probe contract for testing COLMI R10 BLE
connectivity on the X4 without enabling normal Reader page-turn behavior by default.

The probe mode is intentionally event-fed. The future ESP32-C3 BLE task performs
the real BLE operations and feeds these events into `R10BleOnDeviceProbe`:

    scan advertisement
    connect result
    GATT discovery events
    notify subscribe write result
    remote start write result
    poll write result
    notify payload
    shutdown write result
    disconnect/link-loss result

The report object tracks whether the connection was validated:

    scan_matched
    connected
    gatt_ready
    notify_subscribed
    remote_started
    notifications_total

Connectivity is considered validated when all required flags are true and at
least one notification was received.

## Live R10 values

Validated ring:

    Address: 31:39:46:36:E5:05
    Name: COLMI R10_E505

Observed handles:

    Service start: 14
    Service end:   19
    Write value:   15
    Notify value:  17
    Notify CCCD:   19

Known accepted notification packets:

    No event: 02 00 00 00 00 00 00 00 00 00 00 00 00 00 00 02
    Motion:   02 02 00 00 00 00 00 00 00 00 00 00 00 00 00 04
    Ignored vendor/status:
              73 01 00 00 00 00 00 00 00 00 00 00 00 00 00 74

The vendor/status packet has a valid checksum but is not a page-turn motion event.

## Safety

BLE remains disabled by default. This probe contract only adds the state/report
model and tests. The actual ESP32-C3 BLE task should remain behind the existing
`r10-ble-host` feature and an explicit runtime setting before it is allowed to
scan/connect on device.


## Probe event runner

`R10BleProbeRunner` wraps `R10BleOnDeviceProbe` with a deadline and terminal
outcome. The future BLE task should call:

    R10BleProbeRunner::live_r10(started_at_ms)

Then feed events:

    AdvertisedDevice(...)
    Connected
    Discovery(...)
    DiscoveryComplete
    WriteResult { phase, success, now_ms }
    Notify { handle, payload, now_ms }
    ConnectFailed { now_ms }
    LinkLost { now_ms }

The runner returns `R10BleProbeRunResult` after each event:

    effect
    outcome
    report

Terminal outcomes:

    Validated
    TimedOut
    ConnectFailed
    LinkLost
    WriteFailed(...)

`Validated` means scan, connect, GATT discovery, notify subscription, remote start,
and at least one notification all completed.


## Probe log/report formatter

`R10BleProbeLogFormatter` provides compact monitor-safe output for the future
on-device BLE task.

Summary line:

    [r10-ble-probe] outcome=validated scan=1 connected=1 gatt=1 notify_sub=1 remote=1 stopped=0 polls=2 notify=3 no_event=1 motion=1 vendor7301=1 unknown=0 bad_checksum=0 wrong_handle=0 wrong_length=0

Write failure line:

    [r10-ble-probe] outcome=write_failed write_phase=remote_start ...

Notify line:

    [r10-ble-probe] notify kind=vendor_status_7301 total=3 motion=1 vendor7301=1 ignored_valid=1

The formatter writes into any `core::fmt::Write`, so the BLE task can use a small
stack buffer, serial monitor adapter, or test `String` without allocating in the
runtime path.


## Runtime mode/config gate

`R10BleRuntimeConfig` is the explicit runtime gate for BLE use.

Modes:

    Disabled
    ProbeOnly
    ReaderRemote

Default behavior is `Disabled`, so BLE scan/connect must not start unless a
runtime setting explicitly selects `ProbeOnly` or `ReaderRemote`.

Start decisions:

    StayDisabled
    StartProbe
    StartReaderRemote

`ProbeOnly` is for the on-device connectivity probe. `ReaderRemote` is for the
future Reader page-turn mode. This keeps diagnostic BLE behavior separate from
normal Reader remote-control behavior.


## Startup queue de-duplication

The lower-level GATT helper still documents the full startup write pair:

    CCCD enable
    remote-start command

The runtime startup queue de-duplicates that path. It emits only two pending
writes:

    Subscribe      -> CCCD enable
    RemoteStart    -> remote-start command

This avoids writing CCCD enable twice during the Reader/Probe startup path while
preserving the lower-level operation-order contract.


## On-device task adapter scaffold

`R10BleDeviceTaskPlan` and `R10BleDeviceTaskState` provide the runtime adapter
contract for the future ESP BLE task.

The adapter maps runtime config to task startup actions:

    Disabled      -> StayDisabled
    ProbeOnly     -> StartProbeOnly
    ReaderRemote  -> StartReaderRemote

The default remains disabled. The task state also prevents repeated start
requests until the task is stopped or reconfigured. This is the boundary that
the next hardware-facing deliverable can attach to `esp-radio` / `trouble-host`
without changing the Reader policy or packet contracts.


## Device-task test visibility and start request

The host test harness now includes the `r10_ble_device_task` module so its
contract tests run under `cargo test`.

`R10BleDeviceTaskState::start_request()` is the one-shot boundary for the future
hardware-facing BLE task:

    Disabled      -> None
    ProbeOnly     -> Some(StartProbeOnly)
    ReaderRemote  -> Some(StartReaderRemote)

A second call returns `None` until the task is stopped or reconfigured. This
keeps BLE startup explicit and prevents accidental repeated scan/connect startup
loops.


## ProbeOnly hardware runner boundary

`R10BleDeviceTaskRunnerPlan` is the hardware-facing boundary for the future BLE
task. It consumes the one-shot `R10BleDeviceTaskStartRequest` and maps it to a
monitor-safe lifecycle script:

    start_scan
    target_seen
    connect
    discover
    subscribe
    remote_start
    poll
    notify
    timeout

`ProbeOnly` uses this lifecycle without emitting Reader input events.
`ReaderRemote` uses the same lifecycle but enables Reader event emission after
valid motion notifications pass the existing policy bridge. `Disabled` still
produces no runner plan and does not start BLE.


## ProbeOnly monitor event sink

`R10BleDeviceTaskMonitorEvent` and `R10BleDeviceTaskMonitorSink` provide a
small fixed-capacity event sink for the future hardware runner.

Each lifecycle event exposes compact monitor-safe labels:

    r10_ble_task
    start_probe_only / start_reader_remote
    start_scan / target_seen / connect / discover / subscribe / remote_start / poll / notify / timeout
    reader_off / reader_on
    running / terminal

`ProbeOnly` records lifecycle events with `reader_off`. `ReaderRemote` records
the same lifecycle with `reader_on`. `Disabled` still produces no runner plan and
therefore no monitor events.


## ProbeOnly hardware adapter command plan

`R10BleDeviceTaskHardwareCommandPlan` maps the hardware-neutral lifecycle plan
to the BLE operations the real adapter must perform:

    start_scan    -> scan_start
    target_seen   -> connect_target
    connect       -> discover_gatt
    discover      -> subscribe_cccd
    subscribe     -> write_remote_start
    remote_start  -> write_poll
    poll          -> write_poll
    notify        -> handle_notify
    timeout       -> no command

`ProbeOnly` remains log-only: notify handling is recorded but does not emit
Reader input. `ReaderRemote` uses the same command sequence, but the notify
handler is marked Reader-event capable so the next hardware adapter step can
route accepted motion notifications through the existing policy bridge.


## Hardware adapter execution transcript

`R10BleDeviceTaskHardwareTranscript` records command execution outcomes over the
hardware command plan. The transcript remains hardware-neutral and does not call
`esp-radio` or `trouble-host` directly.

Supported outcomes:

    pending
    started
    ok
    ignored
    failed
    timeout

Reader input emission remains guarded: `ReaderRemote` can emit a Reader event
only when the command is `handle_notify` and the outcome is `ok`. `ProbeOnly`
records the same notify command as log-only. Failed and timeout outcomes are
marked retry-worthy for the future adapter loop.


## Hardware adapter mock executor

`R10BleDeviceTaskHardwareMockExecutor` consumes the hardware command plan and
records execution outcomes into `R10BleDeviceTaskHardwareTranscript`.

Mock scripts:

    success
    timeout_notify
    failed_write
    ignored_notify

The executor is intentionally hardware-neutral. It does not call `esp-radio` or
`trouble-host`; it only exercises the command/transcript contract that the
future on-device adapter will implement.

Reader input emission remains guarded. `ProbeOnly` records notify outcomes as
log-only. `ReaderRemote` records a Reader-capable notify path, but the mock
report counts a Reader event only when `handle_notify` completes with `ok`.


## X4 deployable ProbeOnly package

The X4 deployment path is provided by:

    scripts/x4_r10_ble_probe_deploy.sh
    docs/R10_BLE_X4_DEPLOY.md

`probe_only` is the default deploy mode for the script. It builds with
`--features r10-ble-host`, validates the deployment contract, flashes the X4
with `espflash`, and opens the serial monitor.

`reader_remote` is guarded by `R10_BLE_X4_ALLOW_READER_REMOTE=1` so Reader input
cannot be enabled accidentally during first hardware validation.


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

