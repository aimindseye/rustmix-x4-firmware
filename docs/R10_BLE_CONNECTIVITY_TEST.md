# COLMI R10 BLE Connectivity Test

This probe validates the real COLMI R10 ring before firmware-side ESP32-C3 BLE runtime wiring.

It tests:

- BLE scan by known address or `COLMI R10` name prefix.
- Connection to the R10 BLE peripheral.
- GATT service and characteristic presence:
  - Service: `6e40fff0-b5a3-f393-e0a9-e50e24dcca9e`
  - Write: `6e400002-b5a3-f393-e0a9-e50e24dcca9e`
  - Notify: `6e400003-b5a3-f393-e0a9-e50e24dcca9e`
- Notification subscription.
- Remote-mode start/poll/stop writes.
- 16-byte notification checksum validation.
- Motion notification decoding.

Known R10 values:

```text
Address: 31:39:46:36:E5:05
Advertised name: COLMI R10_E505
Service UUID: 6e40fff0-b5a3-f393-e0a9-e50e24dcca9e
Write UUID:   6e400002-b5a3-f393-e0a9-e50e24dcca9e
Notify UUID:  6e400003-b5a3-f393-e0a9-e50e24dcca9e
```

Known remote-mode packets:

```text
START: 02 04 00 00 00 00 00 00 00 00 00 00 00 00 00 06
POLL:  02 05 00 00 00 00 00 00 00 00 00 00 00 00 00 07
STOP:  02 06 00 00 00 00 00 00 00 00 00 00 00 00 00 08
```

Known notification packets:

```text
NO EVENT: 02 00 00 00 00 00 00 00 00 00 00 00 00 00 00 02
MOTION:   02 02 00 00 00 00 00 00 00 00 00 00 00 00 00 04
```

The last byte is checksum: sum of bytes 0 through 14 masked to `0xff`.

## Install

From the repo root:

```bash
cd /home/mindseye73/Documents/projects/rustmix-x4-firmware

python3 -m venv .venv-r10
source .venv-r10/bin/activate
python3 -m pip install --upgrade pip
python3 -m pip install -r requirements-r10.txt
```

## 1. Scan-only test

Wake the R10 and keep it near the host Bluetooth adapter.

```bash
source .venv-r10/bin/activate

python3 tools/r10_ble_probe.py \
  --address 31:39:46:36:E5:05 \
  --scan-only
```

Expected success signs:

```text
[scan] match by address: 31:39:46:36:E5:05 name='COLMI R10_E505'
[result] scan ok
```

Name-prefix fallback:

```bash
python3 tools/r10_ble_probe.py \
  --address "" \
  --name-prefix "COLMI R10" \
  --scan-only
```

## 2. Connect-only + GATT validation

```bash
python3 tools/r10_ble_probe.py \
  --address 31:39:46:36:E5:05 \
  --connect-only \
  --print-services
```

Expected success signs:

```text
[connect] connected
[gatt] service 6e40fff0-b5a3-f393-e0a9-e50e24dcca9e: ok
[gatt] write   6e400002-b5a3-f393-e0a9-e50e24dcca9e: ok
[gatt] notify  6e400003-b5a3-f393-e0a9-e50e24dcca9e: ok
[result] connect ok
```

## 3. Subscribe-only test

This validates notification subscription without sending remote-mode packets.

```bash
python3 tools/r10_ble_probe.py \
  --address 31:39:46:36:E5:05 \
  --duration 15 \
  --no-remote \
  --print-services
```

Expected success signs:

```text
[connect] connected
[gatt] subscribing notify=6e400003-b5a3-f393-e0a9-e50e24dcca9e
[gatt] unsubscribing
[result] connected=True notifications=0 motion=0
```

Notifications may remain at zero in subscribe-only mode because the ring usually needs remote-mode start/poll writes.

## 4. Full remote-mode connectivity test

```bash
python3 tools/r10_ble_probe.py \
  --address 31:39:46:36:E5:05 \
  --duration 30 \
  --poll-interval 1.0 \
  --print-services
```

Expected success signs:

```text
[connect] connected
[gatt] service 6e40fff0-b5a3-f393-e0a9-e50e24dcca9e: ok
[gatt] write   6e400002-b5a3-f393-e0a9-e50e24dcca9e: ok
[gatt] notify  6e400003-b5a3-f393-e0a9-e50e24dcca9e: ok
[gatt] subscribing notify=6e400003-b5a3-f393-e0a9-e50e24dcca9e
[write] start response=False data=02 04 00 00 00 00 00 00 00 00 00 00 00 00 00 06
[write] poll  response=False data=02 05 00 00 00 00 00 00 00 00 00 00 00 00 00 07
```

Move or tap the ring while the probe is running. Expected motion packet:

```text
[notify] sender=... kind=motion checksum=ok data=02 02 00 00 00 00 00 00 00 00 00 00 00 00 00 04
```

A normal idle/no-event packet looks like:

```text
[notify] sender=... kind=no_event checksum=ok data=02 00 00 00 00 00 00 00 00 00 00 00 00 00 00 02
```

## 5. Strict notification test

Return non-zero if no notification arrives:

```bash
python3 tools/r10_ble_probe.py \
  --address 31:39:46:36:E5:05 \
  --duration 30 \
  --require-notification
```

Return non-zero if no motion notification arrives:

```bash
python3 tools/r10_ble_probe.py \
  --address 31:39:46:36:E5:05 \
  --duration 30 \
  --require-motion
```

For `--require-motion`, move or tap the ring during the test window.

## 6. Write-with-response fallback

The firmware plan uses write-without-response for remote commands. If the host BLE stack or ring behaves better with write-with-response during testing, run:

```bash
python3 tools/r10_ble_probe.py \
  --address 31:39:46:36:E5:05 \
  --duration 30 \
  --write-response
```

## Linux troubleshooting

Verify Bluetooth is powered:

```bash
bluetoothctl show
rfkill list bluetooth
```

Restart Bluetooth if scanning fails:

```bash
sudo systemctl restart bluetooth
```

Watch adapter logs while probing:

```bash
journalctl -u bluetooth -f
```

BlueZ permissions vary by distro. If non-root scanning fails, try from a normal desktop session first, then retry with Bluetooth restarted.

## macOS notes

The probe should work from macOS with Python + Bleak. macOS may show a randomized BLE address. Use name-prefix matching when the exact address does not match:

```bash
python3 tools/r10_ble_probe.py \
  --address "" \
  --name-prefix "COLMI R10" \
  --duration 30
```

## Exit codes

```text
0   success
1   unexpected error
2   R10 not found
3   connect failed
4   expected R10 GATT UUID missing
5   --require-notification was set, but no notification arrived
6   --require-motion was set, but no motion notification arrived
130 interrupted
```

## Firmware implication

A successful full remote-mode test confirms the values needed by the ESP32-C3 firmware path:

- scan matcher can identify the ring,
- connection can be established,
- expected service/write/notify characteristics are present,
- notify subscription works,
- start/poll/stop packets are accepted,
- motion notifications decode into the existing reader-next-page policy.


## Live validation: COLMI R10_E505

Validated with the known ring:

    Address: 31:39:46:36:E5:05
    Name: COLMI R10_E505

Observed GATT layout:

    Service 6e40fff0-b5a3-f393-e0a9-e50e24dcca9e handle=14
      Notify 6e400003-b5a3-f393-e0a9-e50e24dcca9e handle=17 props=notify
        CCCD 00002902-0000-1000-8000-00805f9b34fb handle=19
      Write  6e400002-b5a3-f393-e0a9-e50e24dcca9e handle=15 props=write-without-response,write

Validated runtime behavior:

    Remote start write accepted: 02 04 ... 06
    Remote poll write accepted:  02 05 ... 07
    Remote stop write accepted:  02 06 ... 08
    No-event notify:             02 00 ... 02
    Motion notify:               02 02 ... 04

A 30-second run produced:

    notifications=37
    motion=4

Known valid-checksum vendor/status packet seen during the run:

    73 01 00 00 00 00 00 00 00 00 00 00 00 00 00 74

This packet is intentionally treated as accepted-but-ignored, not as a page-turn motion event.

