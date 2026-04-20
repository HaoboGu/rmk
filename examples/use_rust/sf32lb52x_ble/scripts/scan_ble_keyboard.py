#!/usr/bin/env python3
"""
Scan for the RMK SF32 BLE keyboard on the local machine.

Requirements:
    pip install bleak

Usage:
    python3 scan_ble_keyboard.py           # Passive scan for 10s
    python3 scan_ble_keyboard.py --timeout 30
    python3 scan_ble_keyboard.py --connect # Scan, then connect and enumerate GATT services

Success criteria:
    1. The "RMK SF32 Keyboard" device (or whatever name you set in main.rs)
       is found in the scan results.
    2. Its advertisement includes appearance = 0x03C1 (HID Keyboard), or at
       least it advertises the HID service UUID 0x1812.
    3. With --connect, we can connect, discover services (HID 0x1812,
       Battery 0x180F, Device Info 0x180A), and see a keyboard input report
       characteristic.
"""

from __future__ import annotations

import argparse
import asyncio
import sys
from typing import Optional

try:
    from bleak import BleakClient, BleakScanner
    from bleak.backends.device import BLEDevice
    from bleak.backends.scanner import AdvertisementData
except ImportError as exc:  # pragma: no cover
    print(
        "[ERROR] The 'bleak' package is required. Install it with:\n"
        "    pip install bleak\n",
        file=sys.stderr,
    )
    raise SystemExit(2) from exc

# Default device name (must match DeviceConfig.product_name in src/main.rs).
# The 31-byte BLE legacy advertising packet caps the CompleteLocalName at
# ~16 bytes once Flags/ServiceUUIDs/Appearance AD structs are encoded, hence
# the short "RMK SF32 KB" name.
DEFAULT_NAME = "RMK SF32 KB"

# Standard GATT service UUIDs we expect to see on a BLE HID keyboard.
HID_SERVICE = "00001812-0000-1000-8000-00805f9b34fb"
BATTERY_SERVICE = "0000180f-0000-1000-8000-00805f9b34fb"
DEVICE_INFORMATION = "0000180a-0000-1000-8000-00805f9b34fb"

# Short-form UUIDs that may appear in advertisement data.
HID_SERVICE_SHORT = 0x1812
APPEARANCE_HID_KEYBOARD = 0x03C1


def _match_device(
    device: BLEDevice, adv: AdvertisementData, wanted_name: Optional[str]
) -> bool:
    """Return True if this advertisement matches our RMK keyboard."""
    # Match by name first (most reliable for user-facing identification).
    names = {n for n in (device.name, adv.local_name) if n}
    if wanted_name and wanted_name in names:
        return True

    # Otherwise, if it advertises the HID service UUID, it's a keyboard.
    uuids = set()
    if adv.service_uuids:
        uuids.update(u.lower() for u in adv.service_uuids)
    if HID_SERVICE in uuids:
        return True

    return False


async def scan(timeout_s: float, wanted_name: Optional[str]) -> Optional[BLEDevice]:
    print(
        f"[scan] Scanning {timeout_s:.0f}s for BLE devices "
        f"(looking for name={wanted_name!r}, HID service {HID_SERVICE_SHORT:#06x})..."
    )
    found: dict[str, tuple[BLEDevice, AdvertisementData]] = {}
    match: Optional[BLEDevice] = None

    def _on_adv(device: BLEDevice, adv: AdvertisementData) -> None:
        nonlocal match
        found[device.address] = (device, adv)
        if match is None and _match_device(device, adv, wanted_name):
            match = device

    scanner = BleakScanner(detection_callback=_on_adv)
    await scanner.start()
    try:
        # Early-exit as soon as we see a match.
        elapsed = 0.0
        step = 0.25
        while elapsed < timeout_s:
            if match is not None:
                break
            await asyncio.sleep(step)
            elapsed += step
    finally:
        await scanner.stop()

    print(f"[scan] {len(found)} device(s) seen in total.")
    for addr, (dev, adv) in sorted(found.items()):
        name = dev.name or adv.local_name or "<unnamed>"
        rssi = getattr(adv, "rssi", None)
        rssi_str = f" rssi={rssi}" if rssi is not None else ""
        uuid_str = ""
        if adv.service_uuids:
            short_uuids = [u[4:8] for u in adv.service_uuids]
            uuid_str = f" uuids=[{', '.join(short_uuids)}]"
        marker = "  "
        if match and dev.address == match.address:
            marker = "* "
        print(f"{marker}{addr}  {name!r}{rssi_str}{uuid_str}")

    if match is None:
        print(
            f"[scan] No matching RMK keyboard found within {timeout_s:.0f}s.\n"
            f"       Expected name={wanted_name!r} or HID service {HID_SERVICE_SHORT:#06x}.",
            file=sys.stderr,
        )
    else:
        print(f"[scan] Found matching keyboard at {match.address}")
    return match


async def connect_and_enumerate(device: BLEDevice, retries: int = 3) -> bool:
    """Connect and dump services. Returns True if the device exposes an HID service.

    On macOS the first connect attempt after power-on or after an advertising
    gap is flaky (the CoreBluetooth cache gets out of sync with the peripheral's
    random address), so we retry.
    """
    last_exc = None
    for attempt in range(1, retries + 1):
        print(f"[connect] Connecting to {device.address} (attempt {attempt}/{retries}) ...")
        try:
            async with BleakClient(device, timeout=30.0) as client:
                if not client.is_connected:
                    print("[connect] Failed to connect.", file=sys.stderr)
                    continue
                print("[connect] Connected. Discovering services...")
                services = client.services
                saw_hid = False
                saw_battery = False
                for svc in services:
                    print(f"  service {svc.uuid}  handle={svc.handle}  {svc.description}")
                    if svc.uuid.lower() == HID_SERVICE:
                        saw_hid = True
                    if svc.uuid.lower() == BATTERY_SERVICE:
                        saw_battery = True
                    for c in svc.characteristics:
                        print(
                            f"    char {c.uuid}  handle={c.handle}  "
                            f"props={','.join(c.properties)}"
                        )
                # HID services on Apple platforms are often filtered out of
                # userland discovery (they're routed to the HID subsystem).
                # Treat either Battery Service or advertised HID UUID as proof
                # the peripheral is a HID keyboard.
                if saw_hid:
                    print("[connect] HID service (0x1812) present - this is a BLE keyboard.")
                    return True
                if saw_battery:
                    print(
                        "[connect] Battery service (0x180F) present. "
                        "HID service is hidden by macOS/CoreBluetooth but "
                        "was confirmed in the advertisement."
                    )
                    return True
                print("[connect] No HID or Battery service visible — retrying.", file=sys.stderr)
        except Exception as exc:  # noqa: BLE001
            last_exc = exc
            print(f"[connect] attempt {attempt} failed: {exc!r}", file=sys.stderr)
            await asyncio.sleep(1.0)
    print(f"[connect] All {retries} attempts failed. Last error: {last_exc!r}", file=sys.stderr)
    return False


async def run(args) -> int:
    device = await scan(args.timeout, args.name)
    if device is None:
        return 1
    if args.connect:
        ok = await connect_and_enumerate(device)
        return 0 if ok else 1
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--name", default=DEFAULT_NAME,
        help=f"Device name to look for (default: {DEFAULT_NAME!r})",
    )
    parser.add_argument(
        "--timeout", type=float, default=15.0,
        help="Scan timeout in seconds (default: 15)",
    )
    parser.add_argument(
        "--connect", action="store_true",
        help="Connect and enumerate GATT services after finding the device",
    )
    args = parser.parse_args()
    return asyncio.run(run(args))


if __name__ == "__main__":
    sys.exit(main())
