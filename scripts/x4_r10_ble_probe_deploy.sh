#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

MODE="${R10_BLE_X4_MODE:-probe_only}"
PORT="${ESPFLASH_PORT:-${1:-}}"
NO_MONITOR="${R10_BLE_X4_NO_MONITOR:-0}"
FEATURE="r10-ble-host"
PKG="target-xteink-x4"
CHIP="esp32c3"

case "$MODE" in
  probe_only|probe-only|probe)
    MODE="probe_only"
    ;;
  reader_remote|reader-remote|reader)
    MODE="reader_remote"
    if [[ "${R10_BLE_X4_ALLOW_READER_REMOTE:-0}" != "1" ]]; then
      echo "Refusing ReaderRemote deploy without R10_BLE_X4_ALLOW_READER_REMOTE=1" >&2
      echo "Use ProbeOnly first: R10_BLE_X4_MODE=probe_only $0 [PORT]" >&2
      exit 2
    fi
    ;;
  disabled)
    echo "Mode disabled selected; nothing to flash." >&2
    exit 0
    ;;
  *)
    echo "Unsupported R10_BLE_X4_MODE='$MODE'." >&2
    echo "Use: probe_only, reader_remote, or disabled." >&2
    exit 2
    ;;
esac

if ! command -v cargo >/dev/null 2>&1; then
  echo "cargo not found in PATH" >&2
  exit 127
fi

if ! command -v espflash >/dev/null 2>&1; then
  echo "espflash not found in PATH" >&2
  echo "Install it with: cargo install espflash" >&2
  exit 127
fi

echo "==> R10 BLE X4 deploy mode: $MODE"
echo "==> R10 BLE X4 runtime trigger: deploy_script"
echo "==> R10 BLE X4 serial records: enabled"
echo "==> R10 BLE X4 ProbeOnly bridge: enabled"
echo "==> R10 BLE X4 ESP32-C3 backend task: enabled"
echo "==> R10 BLE X4 ESP32-C3 operation queue: enabled"
echo "==> Validating host-side deployment contract"
cargo test -p "$PKG" r10_ble_device_task_x4_deploy -- --nocapture
cargo test -p "$PKG" r10_ble_device_task_x4_runtime -- --nocapture
cargo test -p "$PKG" r10_ble_device_task_x4_serial -- --nocapture
cargo test -p "$PKG" r10_ble_device_task_x4_probe_bridge -- --nocapture
cargo test -p "$PKG" r10_ble_esp32c3_probe_backend -- --nocapture
cargo test -p "$PKG" r10_ble_esp32c3_probe -- --nocapture
cargo test -p "$PKG" r10_ble_device_task_hardware_mock -- --nocapture

export R10_BLE_X4_PROBE_BRIDGE="${R10_BLE_X4_PROBE_BRIDGE:-1}"
export R10_BLE_X4_BACKEND_TASK="${R10_BLE_X4_BACKEND_TASK:-1}"
export R10_BLE_X4_OPERATION_QUEUE="${R10_BLE_X4_OPERATION_QUEUE:-1}"
echo "==> Checking firmware with feature: $FEATURE"
cargo check -p "$PKG" --features "$FEATURE"

echo "==> Building release firmware"
cargo build --release -p "$PKG" --features "$FEATURE"

ELF="${R10_BLE_X4_ELF:-}"
if [[ -z "$ELF" ]]; then
  ELF="$(find target -type f -path "*/release/$PKG" -perm -111 2>/dev/null | sort | tail -n 1 || true)"
fi

if [[ -z "$ELF" || ! -f "$ELF" ]]; then
  echo "Could not locate release ELF for $PKG under target/*/release/$PKG" >&2
  echo "Set R10_BLE_X4_ELF=/path/to/target-xteink-x4 and rerun." >&2
  exit 1
fi

if [[ -z "$PORT" ]]; then
  for candidate in /dev/ttyACM0 /dev/ttyUSB0 /dev/cu.usbmodem* /dev/cu.usbserial*; do
    if [[ -e "$candidate" ]]; then
      PORT="$candidate"
      break
    fi
  done
fi

if [[ -z "$PORT" ]]; then
  echo "No serial port provided or auto-detected." >&2
  echo "Usage: $0 /dev/ttyACM0" >&2
  echo "Or set ESPFLASH_PORT=/dev/ttyACM0" >&2
  exit 1
fi

echo "==> Flashing $ELF to $PORT as $CHIP"

if [[ "$NO_MONITOR" == "1" ]]; then
  espflash flash --chip "$CHIP" --port "$PORT" "$ELF"
else
  espflash flash --chip "$CHIP" --port "$PORT" --monitor "$ELF"
fi
