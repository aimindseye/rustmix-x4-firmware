#!/usr/bin/env python3
"""
COLMI R10 BLE connectivity probe.

Purpose:
- Scan for COLMI R10 by address/name.
- Connect to the proprietary R10 UART-like GATT service.
- Verify service/write/notify UUIDs.
- Subscribe to notifications.
- Send remote-mode START/POLL/STOP packets.
- Decode 16-byte R10 notifications and checksum.

Requires:
  python3 -m pip install bleak
"""

from __future__ import annotations

import argparse
import asyncio
import contextlib
import sys
import time
from dataclasses import dataclass
from typing import Iterable, Optional

from bleak import BleakClient, BleakScanner


R10_DEFAULT_ADDRESS = "31:39:46:36:E5:05"
R10_DEFAULT_NAME_PREFIX = "COLMI R10"
R10_DEFAULT_ADVERTISED_NAME = "COLMI R10_E505"

R10_SERVICE_UUID = "6e40fff0-b5a3-f393-e0a9-e50e24dcca9e"
R10_WRITE_UUID = "6e400002-b5a3-f393-e0a9-e50e24dcca9e"
R10_NOTIFY_UUID = "6e400003-b5a3-f393-e0a9-e50e24dcca9e"

R10_REMOTE_START = bytes([0x02, 0x04, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x06])
R10_REMOTE_POLL = bytes([0x02, 0x05, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x07])
R10_REMOTE_STOP = bytes([0x02, 0x06, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x08])


@dataclass(frozen=True)
class R10Notify:
    raw_hex: str
    valid_checksum: bool
    kind: str


@dataclass(frozen=True)
class ProbeResult:
    connected: bool
    notifications: int
    motion: int


def canonical_uuid(value: object) -> str:
    return str(value).lower()


def hex_bytes(data: bytes) -> str:
    return " ".join(f"{b:02X}" for b in data)


def checksum_ok(data: bytes) -> bool:
    return len(data) == 16 and (sum(data[:15]) & 0xFF) == data[15]


def decode_notify(data: bytes) -> R10Notify:
    raw = hex_bytes(data)

    if len(data) != 16:
        return R10Notify(raw_hex=raw, valid_checksum=False, kind=f"wrong_len:{len(data)}")

    valid = checksum_ok(data)
    if not valid:
        return R10Notify(raw_hex=raw, valid_checksum=False, kind="bad_checksum")

    if data[0] == 0x02 and data[1] == 0x00:
        return R10Notify(raw_hex=raw, valid_checksum=True, kind="no_event")

    if data[0] == 0x02 and data[1] == 0x02:
        return R10Notify(raw_hex=raw, valid_checksum=True, kind="motion")

    return R10Notify(raw_hex=raw, valid_checksum=True, kind=f"unknown:{data[0]:02X}:{data[1]:02X}")


def address_matches(expected: Optional[str], actual: Optional[str]) -> bool:
    if not expected or not actual:
        return False
    return expected.lower() == actual.lower()


def name_matches(prefix: str, name: Optional[str]) -> bool:
    return bool(name and name.startswith(prefix))


def iter_services(services) -> Iterable:
    # Bleak service collection is iterable across versions, but this helper keeps
    # the call sites readable.
    return services


def characteristic_uuid_set(services) -> set[str]:
    uuids: set[str] = set()
    for service in iter_services(services):
        for char in service.characteristics:
            uuids.add(canonical_uuid(char.uuid))
    return uuids


def print_services(services) -> None:
    for service in iter_services(services):
        print(f"[gatt] service uuid={canonical_uuid(service.uuid)} handle={getattr(service, 'handle', '-')}")
        for char in service.characteristics:
            props = ",".join(getattr(char, "properties", []))
            print(
                f"[gatt]   char uuid={canonical_uuid(char.uuid)} "
                f"handle={getattr(char, 'handle', '-')} props={props}"
            )
            for desc in getattr(char, "descriptors", []):
                print(
                    f"[gatt]     desc uuid={canonical_uuid(desc.uuid)} "
                    f"handle={getattr(desc, 'handle', '-')}"
                )


def validate_gatt_contract(services) -> bool:
    service_uuids = {canonical_uuid(service.uuid) for service in iter_services(services)}
    char_uuids = characteristic_uuid_set(services)

    service_ok = R10_SERVICE_UUID in service_uuids
    write_ok = R10_WRITE_UUID in char_uuids
    notify_ok = R10_NOTIFY_UUID in char_uuids

    print(f"[gatt] service {R10_SERVICE_UUID}: {'ok' if service_ok else 'missing'}")
    print(f"[gatt] write   {R10_WRITE_UUID}: {'ok' if write_ok else 'missing'}")
    print(f"[gatt] notify  {R10_NOTIFY_UUID}: {'ok' if notify_ok else 'missing'}")

    return service_ok and write_ok and notify_ok


async def find_r10(address: Optional[str], name_prefix: str, timeout: float):
    print(f"[scan] timeout={timeout:.1f}s address={address or '-'} name_prefix={name_prefix!r}")

    devices = await BleakScanner.discover(timeout=timeout, return_adv=True)

    for device, adv in devices.values():
        name = device.name or adv.local_name
        rssi = getattr(adv, "rssi", None)
        print(f"[scan] seen address={device.address} name={name!r} rssi={rssi}")

    for device, adv in devices.values():
        name = device.name or adv.local_name
        if address_matches(address, device.address):
            print(f"[scan] match by address: {device.address} name={name!r}")
            return device
        if name_matches(name_prefix, name):
            print(f"[scan] match by name: {device.address} name={name!r}")
            return device

    return None


async def write_packet(client: BleakClient, label: str, packet: bytes, response: bool) -> None:
    print(f"[write] {label:<5} response={response} data={hex_bytes(packet)}")
    await client.write_gatt_char(R10_WRITE_UUID, packet, response=response)


async def run_probe(args: argparse.Namespace) -> int:
    target_address = args.address.strip() or None
    device = await find_r10(target_address, args.name_prefix, args.scan_timeout)
    if device is None:
        print("[result] R10 not found")
        return 2

    if args.scan_only:
        print("[result] scan ok")
        return 0

    notifications: list[R10Notify] = []
    motion_count = 0

    def on_notify(sender, data: bytearray):
        nonlocal motion_count

        packet = decode_notify(bytes(data))
        notifications.append(packet)

        if packet.kind == "motion":
            motion_count += 1

        print(
            f"[notify] sender={sender} kind={packet.kind} "
            f"checksum={'ok' if packet.valid_checksum else 'bad'} data={packet.raw_hex}"
        )

    print(f"[connect] connecting to {device.address}")
    async with BleakClient(device, timeout=args.connect_timeout) as client:
        if not client.is_connected:
            print("[result] connect failed")
            return 3

        print("[connect] connected")

        services = client.services
        if args.print_services:
            print_services(services)

        if not validate_gatt_contract(services):
            print("[result] GATT contract missing expected R10 UUIDs")
            return 4

        if args.connect_only:
            print("[result] connect ok")
            return 0

        print(f"[gatt] subscribing notify={R10_NOTIFY_UUID}")
        await client.start_notify(R10_NOTIFY_UUID, on_notify)
        notify_started = True

        try:
            if not args.no_remote:
                await write_packet(client, "start", R10_REMOTE_START, args.write_response)

            deadline = time.monotonic() + args.duration
            next_poll = time.monotonic()

            while time.monotonic() < deadline:
                now = time.monotonic()

                if not args.no_remote and now >= next_poll:
                    await write_packet(client, "poll", R10_REMOTE_POLL, args.write_response)
                    next_poll = now + args.poll_interval

                await asyncio.sleep(0.05)

            if not args.no_remote:
                await write_packet(client, "stop", R10_REMOTE_STOP, args.write_response)
                await asyncio.sleep(args.post_stop_delay)

        finally:
            if notify_started:
                print("[gatt] unsubscribing")
                with contextlib.suppress(Exception):
                    await client.stop_notify(R10_NOTIFY_UUID)

    result = ProbeResult(
        connected=True,
        notifications=len(notifications),
        motion=motion_count,
    )
    print(
        f"[result] connected={result.connected} notifications={result.notifications} "
        f"motion={result.motion}"
    )

    if args.require_notification and result.notifications == 0:
        print("[result] no notifications received")
        return 5

    if args.require_motion and result.motion == 0:
        print("[result] no motion notifications received")
        return 6

    return 0


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="COLMI R10 BLE connectivity probe")
    parser.add_argument("--address", default=R10_DEFAULT_ADDRESS, help="Target BLE address; pass empty string to disable address matching")
    parser.add_argument("--name-prefix", default=R10_DEFAULT_NAME_PREFIX, help="Target advertised name prefix")
    parser.add_argument("--scan-timeout", type=float, default=10.0, help="BLE scan timeout seconds")
    parser.add_argument("--connect-timeout", type=float, default=20.0, help="BLE connect timeout seconds")
    parser.add_argument("--duration", type=float, default=30.0, help="Remote-mode test duration seconds")
    parser.add_argument("--poll-interval", type=float, default=1.0, help="Remote poll interval seconds")
    parser.add_argument("--post-stop-delay", type=float, default=0.2, help="Delay after stop write before unsubscribe")
    parser.add_argument("--write-response", action="store_true", help="Use GATT write-with-response instead of write-without-response")
    parser.add_argument("--scan-only", action="store_true", help="Only scan and match the R10")
    parser.add_argument("--connect-only", action="store_true", help="Scan, connect, and validate GATT UUIDs without subscribing")
    parser.add_argument("--no-remote", action="store_true", help="Subscribe only; do not send start/poll/stop")
    parser.add_argument("--print-services", action="store_true", help="Print all discovered GATT services/characteristics")
    parser.add_argument("--require-notification", action="store_true", help="Return non-zero if no notification arrives")
    parser.add_argument("--require-motion", action="store_true", help="Return non-zero if no motion notification arrives")
    return parser.parse_args()


def main() -> int:
    try:
        return asyncio.run(run_probe(parse_args()))
    except KeyboardInterrupt:
        print("\n[result] interrupted")
        return 130
    except Exception as exc:
        print(f"[error] {type(exc).__name__}: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
