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
