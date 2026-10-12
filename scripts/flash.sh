#!/usr/bin/env bash
# Flash a .uf2 onto the Framework 16 RGB Macropad.
#   scripts/flash.sh [path/to.uf2]   (default: firmware/build/framework_macropad_padctl.uf2)
# Steps: padctl bootloader -> wait for RPI-RP2 (mount via udisksctl if needed)
# -> copy -> sync -> wait for the macropad to come back -> padctl info.
# Set PADCTL to override the padctl command (default: cargo run -q -p padctl --).
set -euo pipefail

uf2="${1:-firmware/build/framework_macropad_padctl.uf2}"
[[ -f "$uf2" ]] || { echo "no such file: $uf2 (run: mise run firmware)" >&2; exit 1; }
read -r -a padctl <<<"${PADCTL:-cargo run -q -p padctl --}"

wait_for() { # seconds, description, command...
    local secs=$1 what=$2; shift 2
    for ((i = 0; i < secs * 2; i++)); do
        if "$@" >/dev/null 2>&1; then return 0; fi
        sleep 0.5
    done
    echo "timed out after ${secs}s waiting for $what" >&2
    return 1
}

mountpoint_of_rp2() {
    local dev=/dev/disk/by-label/RPI-RP2
    [[ -e "$dev" ]] || return 1
    local mp
    mp="$(findmnt -n -o TARGET --source "$(readlink -f "$dev")" || true)"
    if [[ -z "$mp" ]]; then
        udisksctl mount -b "$dev" >/dev/null 2>&1 || true
        mp="$(findmnt -n -o TARGET --source "$(readlink -f "$dev")" || true)"
    fi
    [[ -n "$mp" ]] && echo "$mp"
}

echo "==> rebooting macropad into the bootloader"
"${padctl[@]}" bootloader

echo "==> waiting for the RPI-RP2 drive"
wait_for 30 "RPI-RP2 (hold 2+6 while inserting the module if it never appears)" mountpoint_of_rp2
mp="$(mountpoint_of_rp2)"
echo "    mounted at $mp"

echo "==> copying $uf2"
cp "$uf2" "$mp/"
sync

echo "==> waiting for the macropad to come back"
wait_for 30 "the macropad to re-enumerate" bash -c "lsusb -d 32ac:0013 || ls /dev/hidraw* >/dev/null && ${padctl[*]} list | grep -q ."
sleep 1
"${padctl[@]}" info
