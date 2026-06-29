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
