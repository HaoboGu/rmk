#!/usr/bin/env bash
set -euo pipefail

if [ $# -lt 1 ]; then
    echo "Usage: $0 <ELF> [probe-rs extra args...]" >&2
    exit 64
fi

elf_path="$1"
shift

if [ ! -f "$elf_path" ]; then
    echo "ELF file not found: $elf_path" >&2
    exit 66
fi

sftool_port="${SIFLI_SFTOOL_PORT:-}"

# Auto-detect serial port if not set or using placeholder
if [ -z "$sftool_port" ] || echo "$sftool_port" | grep -q "YOUR_PORT_HERE"; then
    # Look for common USB-serial devices (CH340, CP210x, FTDI, etc.)
    # Use sort -u to deduplicate (e.g. wchusbserial matches both patterns)
    candidates=( $(ls /dev/cu.*wchusbserial* /dev/cu.*usbserial* /dev/cu.*usbmodem* 2>/dev/null | sort -u || true) )
    if [ ${#candidates[@]} -eq 0 ]; then
        echo "[runner] No USB serial device found. Please connect the board and retry," >&2
        echo "         or set SIFLI_SFTOOL_PORT manually." >&2
        exit 65
    elif [ ${#candidates[@]} -eq 1 ]; then
        sftool_port="${candidates[0]}"
        echo "[runner] Auto-detected serial port: $sftool_port"
    else
        echo "[runner] Multiple serial devices found:" >&2
        for dev in "${candidates[@]}"; do echo "  $dev" >&2; done
        echo "[runner] Please set SIFLI_SFTOOL_PORT to the correct one." >&2
        exit 65
    fi
fi

echo "[runner] Flashing with sftool: $elf_path"
if [ -n "${SIFLI_SFTOOL_EXTRA_ARGS:-}" ]; then
    # shellcheck disable=SC2086
    sftool -c SF32LB52 --port "$sftool_port" \
        ${SIFLI_SFTOOL_EXTRA_ARGS} write_flash --verify "$elf_path"
else
    sftool -c SF32LB52 --port "$sftool_port" \
        write_flash --verify "$elf_path"
fi

echo "[runner] Starting probe-rs attach"
if [ -n "${SIFLI_PROBE_EXTRA_ARGS:-}" ]; then
    # shellcheck disable=SC2086
    probe-rs attach --chip SF32LB52 \
        ${SIFLI_PROBE_EXTRA_ARGS} "$elf_path" "$@"
else
    probe-rs attach --chip SF32LB52 "$elf_path" "$@"
fi
