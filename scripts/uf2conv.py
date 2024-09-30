#!/usr/bin/env python3

import sys
import struct
import subprocess
import re
import os
import os.path
import argparse
import json
from time import sleep

chip_families = [
    {
        "id": "0x16573617",
        "short_name": "ATMEGA32",
        "description": "Microchip (Atmel) ATmega32"
    },
    {
        "id": "0x1851780a",
        "short_name": "SAML21",
        "description": "Microchip (Atmel) SAML21"
    },
    {
        "id": "0x1b57745f",
        "short_name": "NRF52",
        "description": "Nordic NRF52"
    },
    {
        "id": "0x1c5f21b0",
        "short_name": "ESP32",
        "description": "ESP32"
    },
    {
        "id": "0x1e1f432d",
        "short_name": "STM32L1",
        "description": "ST STM32L1xx"
    },
    {
        "id": "0x202e3a91",
        "short_name": "STM32L0",
        "description": "ST STM32L0xx"
    },
    {
        "id": "0x21460ff0",
        "short_name": "STM32WL",
        "description": "ST STM32WLxx"
    },
    {
        "id": "0x22e0d6fc",
        "short_name": "RTL8710B",
        "description": "Realtek AmebaZ RTL8710B"
    },
    {
        "id": "0x2abc77ec",
        "short_name": "LPC55",
        "description": "NXP LPC55xx"
    },
    {
        "id": "0x300f5633",
        "short_name": "STM32G0",
        "description": "ST STM32G0xx"
    },
    {
        "id": "0x31d228c6",
        "short_name": "GD32F350",
        "description": "GD32F350"
    },
    {
        "id": "0x3379CFE2",
        "short_name": "RTL8720D",
        "description": "Realtek AmebaD RTL8720D"
    },
    {
        "id": "0x04240bdf",
        "short_name": "STM32L5",
        "description": "ST STM32L5xx"
    },
    {
        "id": "0x4c71240a",
        "short_name": "STM32G4",
        "description": "ST STM32G4xx"
    },
    {
        "id": "0x4fb2d5bd",
        "short_name": "MIMXRT10XX",
        "description": "NXP i.MX RT10XX"
    },
    {
        "id": "0x51e903a8",
        "short_name": "XR809",
        "description": "Xradiotech 809"
    },
    {
        "id": "0x53b80f00",
        "short_name": "STM32F7",
        "description": "ST STM32F7xx"
    },
    {
        "id": "0x55114460",
        "short_name": "SAMD51",
        "description": "Microchip (Atmel) SAMD51"
    },
    {
        "id": "0x57755a57",
        "short_name": "STM32F4",
        "description": "ST STM32F4xx"
    },
    {
        "id": "0x5a18069b",
        "short_name": "FX2",
        "description": "Cypress FX2"
    },
    {
        "id": "0x5d1a0a2e",
        "short_name": "STM32F2",
        "description": "ST STM32F2xx"
    },
    {
        "id": "0x5ee21072",
        "short_name": "STM32F1",
        "description": "ST STM32F103"
    },
    {
        "id": "0x621e937a",
        "short_name": "NRF52833",
        "description": "Nordic NRF52833"
    },
    {
        "id": "0x647824b6",
        "short_name": "STM32F0",
        "description": "ST STM32F0xx"
    },
    {
        "id": "0x675a40b0",
        "short_name": "BK7231U",
        "description": "Beken 7231U/7231T"
    },
    {
        "id": "0x68ed2b88",
        "short_name": "SAMD21",
        "description": "Microchip (Atmel) SAMD21"
    },
    {
        "id": "0x6a82cc42",
        "short_name": "BK7251",
        "description": "Beken 7251/7252"
    },
    {
        "id": "0x6b846188",
        "short_name": "STM32F3",
        "description": "ST STM32F3xx"
    },
    {
        "id": "0x6d0922fa",
        "short_name": "STM32F407",
        "description": "ST STM32F407"
    },
    {
        "id": "0x6db66082",
        "short_name": "STM32H7",
        "description": "ST STM32H7xx"
    },
    {
        "id": "0x70d16653",
        "short_name": "STM32WB",
        "description": "ST STM32WBxx"
    },
    {
        "id": "0x7b3ef230",
        "short_name": "BK7231N",
        "description": "Beken 7231N"
    },
    {
        "id": "0x7eab61ed",
        "short_name": "ESP8266",
        "description": "ESP8266"
    },
    {
        "id": "0x7f83e793",
        "short_name": "KL32L2",
        "description": "NXP KL32L2x"
    },
    {
        "id": "0x8fb060fe",
        "short_name": "STM32F407VG",
        "description": "ST STM32F407VG"
    },
    {
        "id": "0x9fffd543",
        "short_name": "RTL8710A",
        "description": "Realtek Ameba1 RTL8710A"
    },
    {
        "id": "0xada52840",
        "short_name": "NRF52840",
        "description": "Nordic NRF52840"
    },
    {
        "id": "0xbfdd4eee",
        "short_name": "ESP32S2",
        "description": "ESP32-S2"
    },
    {
        "id": "0xc47e5767",
        "short_name": "ESP32S3",
        "description": "ESP32-S3"
    },
    {
        "id": "0xd42ba06c",
        "short_name": "ESP32C3",
        "description": "ESP32-C3"
    },
    {
        "id": "0x2b88d29c",
        "short_name": "ESP32C2",
        "description": "ESP32-C2"
    },
    {
        "id": "0x332726f6",
        "short_name": "ESP32H2",
        "description": "ESP32-H2"
    },
    {
        "id": "0x540ddf62",
        "short_name": "ESP32C6",
        "description": "ESP32-C6"
    },
    {
        "id": "0x3d308e94",
        "short_name": "ESP32P4",
        "description": "ESP32-P4"
    },
    {
        "id": "0xf71c0343",
        "short_name": "ESP32C5",
        "description": "ESP32-C5"
    },
    {
        "id": "0x77d850c4",
        "short_name": "ESP32C61",
        "description": "ESP32-C61"
    },
    {
        "id": "0xde1270b7",
        "short_name": "BL602",
        "description": "Boufallo 602"
    },
    {
        "id": "0xe08f7564",
        "short_name": "RTL8720C",
        "description": "Realtek AmebaZ2 RTL8720C"
    },
    {
        "id": "0xe48bff56",
        "short_name": "RP2040",
        "description": "Raspberry Pi RP2040"
    },
    {
        "id": "0xe48bff57",
        "short_name": "RP2XXX_ABSOLUTE",
        "description": "Raspberry Pi Microcontrollers: Absolute (unpartitioned) download"
    },
    {
        "id": "0xe48bff58",
        "short_name": "RP2XXX_DATA",
        "description": "Raspberry Pi Microcontrollers: Data partition download"
    },
    {
        "id": "0xe48bff59",
        "short_name": "RP2350_ARM_S",
        "description": "Raspberry Pi RP2350, Secure Arm image"
    },
    {
        "id": "0xe48bff5a",
        "short_name": "RP2350_RISCV",
        "description": "Raspberry Pi RP2350, RISC-V image"
    },
    {
        "id": "0xe48bff5b",
        "short_name": "RP2350_ARM_NS",
        "description": "Raspberry Pi RP2350, Non-secure Arm image"
    },
    {
        "id": "0x00ff6919",
        "short_name": "STM32L4",
        "description": "ST STM32L4xx"
    },
    {
        "id": "0x9af03e33",
        "short_name": "GD32VF103",
        "description": "GigaDevice GD32VF103"
    },
    {
        "id": "0x4f6ace52",
        "short_name": "CSK4",
        "description": "LISTENAI CSK300x/400x"
    },
    {
        "id": "0x6e7348a8",
        "short_name": "CSK6",
        "description": "LISTENAI CSK60xx"
    },
    {
        "id": "0x11de784a",
        "short_name": "M0SENSE",
        "description": "M0SENSE BL702"
    },
    {
        "id": "0x4b684d71",
        "short_name": "MaixPlay-U4",
        "description": "Sipeed MaixPlay-U4(BL618)"
    },
    {
        "id": "0x9517422f",
        "short_name": "RZA1LU",
        "description": "Renesas RZ/A1LU (R7S7210xx)"
    },
    {
        "id": "0x2dc309c5",
        "short_name": "STM32F411xE",
        "description": "ST STM32F411xE"
    },
    {
        "id": "0x06d1097b",
        "short_name": "STM32F411xC",
        "description": "ST STM32F411xC"
    },
    {
        "id": "0x72721d4e",
        "short_name": "NRF52832xxAA",
        "description": "Nordic NRF52832xxAA"
    },
    {
        "id": "0x6f752678",
        "short_name": "NRF52832xxAB",
        "description": "Nordic NRF52832xxAB"
    },
    {
        "id": "0xa0c97b8e",
        "short_name": "AT32F415",
        "description": "ArteryTek AT32F415"
    },
    {
        "id": "0x699b62ec",
        "short_name": "CH32V",
        "description": "WCH CH32V2xx and CH32V3xx"
    },
    {
        "id": "0x7be8976d",
        "short_name": "RA4M1",
        "description": "Renesas RA4M1"
    }
]

UF2_MAGIC_START0 = 0x0A324655 # "UF2\n"
UF2_MAGIC_START1 = 0x9E5D5157 # Randomly selected
UF2_MAGIC_END    = 0x0AB16F30 # Ditto

INFO_FILE = "/INFO_UF2.TXT"

appstartaddr = 0x2000
familyid = 0x0


def is_uf2(buf):
    w = struct.unpack("<II", buf[0:8])
    return w[0] == UF2_MAGIC_START0 and w[1] == UF2_MAGIC_START1

def is_hex(buf):
    try:
        w = buf[0:30].decode("utf-8")
    except UnicodeDecodeError:
        return False
    if w[0] == ':' and re.match(rb"^[:0-9a-fA-F\r\n]+$", buf):
        return True
    return False

def convert_from_uf2(buf):
    global appstartaddr
    global familyid
    numblocks = len(buf) // 512
    curraddr = None
    currfamilyid = None
    families_found = {}
    prev_flag = None
    all_flags_same = True
    outp = []
    for blockno in range(numblocks):
        ptr = blockno * 512
        block = buf[ptr:ptr + 512]
        hd = struct.unpack(b"<IIIIIIII", block[0:32])
        if hd[0] != UF2_MAGIC_START0 or hd[1] != UF2_MAGIC_START1:
            print("Skipping block at " + ptr + "; bad magic")
            continue
        if hd[2] & 1:
            # NO-flash flag set; skip block
            continue
        datalen = hd[4]
        if datalen > 476:
            assert False, "Invalid UF2 data size at " + ptr
        newaddr = hd[3]
        if (hd[2] & 0x2000) and (currfamilyid == None):
            currfamilyid = hd[7]
        if curraddr == None or ((hd[2] & 0x2000) and hd[7] != currfamilyid):
            currfamilyid = hd[7]
            curraddr = newaddr
            if familyid == 0x0 or familyid == hd[7]:
                appstartaddr = newaddr
        padding = newaddr - curraddr
        if padding < 0:
            assert False, "Block out of order at " + ptr
        if padding > 10*1024*1024:
            assert False, "More than 10M of padding needed at " + ptr
        if padding % 4 != 0:
            assert False, "Non-word padding size at " + ptr
        while padding > 0:
            padding -= 4
            outp.append(b"\x00\x00\x00\x00")
        if familyid == 0x0 or ((hd[2] & 0x2000) and familyid == hd[7]):
            outp.append(block[32 : 32 + datalen])
        curraddr = newaddr + datalen
        if hd[2] & 0x2000:
            if hd[7] in families_found.keys():
                if families_found[hd[7]] > newaddr:
                    families_found[hd[7]] = newaddr
            else:
                families_found[hd[7]] = newaddr
        if prev_flag == None:
            prev_flag = hd[2]
        if prev_flag != hd[2]:
            all_flags_same = False
        if blockno == (numblocks - 1):
            print("--- UF2 File Header Info ---")
            families = load_families()
            for family_hex in families_found.keys():
                family_short_name = ""
                for name, value in families.items():
                    if value == family_hex:
                        family_short_name = name
                print("Family ID is {:s}, hex value is 0x{:08x}".format(family_short_name,family_hex))
                print("Target Address is 0x{:08x}".format(families_found[family_hex]))
            if all_flags_same:
                print("All block flag values consistent, 0x{:04x}".format(hd[2]))
            else:
                print("Flags were not all the same")
            print("----------------------------")
            if len(families_found) > 1 and familyid == 0x0:
                outp = []
                appstartaddr = 0x0
    return b"".join(outp)

def convert_to_carray(file_content):
    outp = "const unsigned long bindata_len = %d;\n" % len(file_content)
    outp += "const unsigned char bindata[] __attribute__((aligned(16))) = {"
    for i in range(len(file_content)):
        if i % 16 == 0:
            outp += "\n"
        outp += "0x%02x, " % file_content[i]
    outp += "\n};\n"
    return bytes(outp, "utf-8")

def convert_to_uf2(file_content):
    global familyid
    datapadding = b""
    while len(datapadding) < 512 - 256 - 32 - 4:
        datapadding += b"\x00\x00\x00\x00"
    numblocks = (len(file_content) + 255) // 256
    outp = []
    for blockno in range(numblocks):
        ptr = 256 * blockno
        chunk = file_content[ptr:ptr + 256]
        flags = 0x0
        if familyid:
            flags |= 0x2000
        hd = struct.pack(b"<IIIIIIII",
            UF2_MAGIC_START0, UF2_MAGIC_START1,
            flags, ptr + appstartaddr, 256, blockno, numblocks, familyid)
        while len(chunk) < 256:
            chunk += b"\x00"
        block = hd + chunk + datapadding + struct.pack(b"<I", UF2_MAGIC_END)
        assert len(block) == 512
        outp.append(block)
    return b"".join(outp)

class Block:
    def __init__(self, addr):
        self.addr = addr
        self.bytes = bytearray(256)

    def encode(self, blockno, numblocks):
        global familyid
        flags = 0x0
        if familyid:
            flags |= 0x2000
        hd = struct.pack("<IIIIIIII",
            UF2_MAGIC_START0, UF2_MAGIC_START1,
            flags, self.addr, 256, blockno, numblocks, familyid)
        hd += self.bytes[0:256]
        while len(hd) < 512 - 4:
            hd += b"\x00"
        hd += struct.pack("<I", UF2_MAGIC_END)
        return hd

def convert_from_hex_to_uf2(buf):
    global appstartaddr
    appstartaddr = None
    upper = 0
    currblock = None
    blocks = []
    for line in buf.split('\n'):
        if line[0] != ":":
            continue
        i = 1
        rec = []
        while i < len(line) - 1:
            rec.append(int(line[i:i+2], 16))
            i += 2
        tp = rec[3]
        if tp == 4:
            upper = ((rec[4] << 8) | rec[5]) << 16
        elif tp == 2:
            upper = ((rec[4] << 8) | rec[5]) << 4
        elif tp == 1:
            break
        elif tp == 0:
            addr = upper + ((rec[1] << 8) | rec[2])
            if appstartaddr == None:
                appstartaddr = addr
            i = 4
            while i < len(rec) - 1:
                if not currblock or currblock.addr & ~0xff != addr & ~0xff:
                    currblock = Block(addr & ~0xff)
                    blocks.append(currblock)
                currblock.bytes[addr & 0xff] = rec[i]
                addr += 1
                i += 1
    numblocks = len(blocks)
    resfile = b""
    for i in range(0, numblocks):
        resfile += blocks[i].encode(i, numblocks)
    return resfile

def to_str(b):
    return b.decode("utf-8")

def get_drives():
    drives = []
    if sys.platform == "win32":
        r = subprocess.check_output(["wmic", "PATH", "Win32_LogicalDisk",
                                     "get", "DeviceID,", "VolumeName,",
                                     "FileSystem,", "DriveType"])
        for line in to_str(r).split('\n'):
            words = re.split(r'\s+', line)
            if len(words) >= 3 and words[1] == "2" and words[2] == "FAT":
                drives.append(words[0])
    else:
        searchpaths = ["/media"]
        if sys.platform == "darwin":
            searchpaths = ["/Volumes"]
        elif sys.platform == "linux":
            searchpaths += ["/media/" + os.environ["USER"], '/run/media/' + os.environ["USER"]]

        for rootpath in searchpaths:
            if os.path.isdir(rootpath):
                for d in os.listdir(rootpath):
                    if os.path.isdir(rootpath):
                        drives.append(os.path.join(rootpath, d))


    def has_info(d):
        try:
            return os.path.isfile(d + INFO_FILE)
        except:
            return False

    return list(filter(has_info, drives))


def board_id(path):
    with open(path + INFO_FILE, mode='r') as file:
        file_content = file.read()
    return re.search(r"Board-ID: ([^\r\n]*)", file_content).group(1)


def list_drives():
    for d in get_drives():
        print(d, board_id(d))


def write_file(name, buf):
    with open(name, "wb") as f:
        f.write(buf)
    print("Wrote %d bytes to %s" % (len(buf), name))


def load_families():
    # The expectation is that the `uf2families.json` file is in the same
    # directory as this script. Make a path that works using `__file__`
    # which contains the full path to this script.
    # filename = "uf2families.json"
    # pathname = os.path.join(os.path.dirname(os.path.abspath(__file__)), filename)
    # with open(pathname) as f:
    #     chip_families = json.load(f)

    families = {}
    for family in chip_families:
        families[family["short_name"]] = int(family["id"], 0)

    return families


def main():
    global appstartaddr, familyid
    def error(msg):
        print(msg, file=sys.stderr)
        sys.exit(1)
    parser = argparse.ArgumentParser(description='Convert to UF2 or flash directly.')
    parser.add_argument('input', metavar='INPUT', type=str, nargs='?',
                        help='input file (HEX, BIN or UF2)')
    parser.add_argument('-b', '--base', dest='base', type=str,
                        default="0x2000",
                        help='set base address of application for BIN format (default: 0x2000)')
    parser.add_argument('-f', '--family', dest='family', type=str,
                        default="0x0",
                        help='specify familyID - number or name (default: 0x0)')
    parser.add_argument('-o', '--output', metavar="FILE", dest='output', type=str,
                        help='write output to named file; defaults to "flash.uf2" or "flash.bin" where sensible')
    parser.add_argument('-d', '--device', dest="device_path",
                        help='select a device path to flash')
    parser.add_argument('-l', '--list', action='store_true',
                        help='list connected devices')
    parser.add_argument('-c', '--convert', action='store_true',
                        help='do not flash, just convert')
    parser.add_argument('-D', '--deploy', action='store_true',
                        help='just flash, do not convert')
    parser.add_argument('-w', '--wait', action='store_true',
                        help='wait for device to flash')
    parser.add_argument('-C', '--carray', action='store_true',
                        help='convert binary file to a C array, not UF2')
    parser.add_argument('-i', '--info', action='store_true',
                        help='display header information from UF2, do not convert')
    args = parser.parse_args()
    appstartaddr = int(args.base, 0)

    families = load_families()

    if args.family.upper() in families:
        familyid = families[args.family.upper()]
    else:
        try:
            familyid = int(args.family, 0)
        except ValueError:
            error("Family ID needs to be a number or one of: " + ", ".join(families.keys()))

    if args.list:
        list_drives()
    else:
        if not args.input:
            error("Need input file")
        with open(args.input, mode='rb') as f:
            inpbuf = f.read()
        from_uf2 = is_uf2(inpbuf)
        ext = "uf2"
        if args.deploy:
            outbuf = inpbuf
        elif from_uf2 and not args.info:
            outbuf = convert_from_uf2(inpbuf)
            ext = "bin"
        elif from_uf2 and args.info:
            outbuf = ""
            convert_from_uf2(inpbuf)
        elif is_hex(inpbuf):
            outbuf = convert_from_hex_to_uf2(inpbuf.decode("utf-8"))
        elif args.carray:
            outbuf = convert_to_carray(inpbuf)
            ext = "h"
        else:
            outbuf = convert_to_uf2(inpbuf)
        if not args.deploy and not args.info:
            print("Converted to %s, output size: %d, start address: 0x%x" %
                  (ext, len(outbuf), appstartaddr))
        if args.convert or ext != "uf2":
            if args.output == None:
                args.output = "flash." + ext
        if args.output:
            write_file(args.output, outbuf)
        if ext == "uf2" and not args.convert and not args.info:
            drives = get_drives()
            if len(drives) == 0:
                if args.wait:
                    print("Waiting for drive to deploy...")
                    while len(drives) == 0:
                        sleep(0.1)
                        drives = get_drives()
                elif not args.output:
                    error("No drive to deploy.")
            for d in drives:
                print("Flashing %s (%s)" % (d, board_id(d)))
                write_file(d + "/NEW.UF2", outbuf)


if __name__ == "__main__":
    main()