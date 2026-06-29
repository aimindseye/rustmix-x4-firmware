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

