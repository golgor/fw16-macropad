#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-2.0-or-later
# Build the padctl keymap against Framework's QMK fork at the pinned commit.
# Output: firmware/build/framework_macropad_padctl.uf2. Flashing is manual.
#
# Needs: git, make, python3 + pip (the qmk CLI and QMK requirements are installed;
# set PIP_FLAGS=--break-system-packages on distro Pythons),
# arm-none-eabi-gcc + newlib. QMK checkout is cached in $QMK_DIR.
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
# shellcheck source=/dev/null
source <(grep -E '^(REPO|BRANCH|COMMIT)=' "$here/QMK_PIN")
QMK_DIR="${QMK_DIR:-${XDG_CACHE_HOME:-$HOME/.cache}/fw16-macropad/qmk_firmware}"

if [[ ! -d "$QMK_DIR/.git" ]]; then
    mkdir -p "$(dirname "$QMK_DIR")"
    git init -q "$QMK_DIR"
    git -C "$QMK_DIR" remote add origin "$REPO"
fi
if [[ "$(git -C "$QMK_DIR" rev-parse -q --verify HEAD || true)" != "$COMMIT" ]]; then
    git -C "$QMK_DIR" fetch -q --depth 1 origin "$COMMIT"
    git -C "$QMK_DIR" checkout -q --force FETCH_HEAD
fi
git -C "$QMK_DIR" submodule update -q --init --recursive --depth 1

python3 -m pip install -q qmk -r "$QMK_DIR/requirements.txt" ${PIP_FLAGS:-}

keymap="$QMK_DIR/keyboards/framework/macropad/keymaps/padctl"
rm -rf "$keymap"
cp -r "$here/keymaps/padctl" "$keymap"

make -C "$QMK_DIR" -j"$(nproc)" framework/macropad:padctl SKIP_GIT=yes

mkdir -p "$here/build"
cp "$QMK_DIR/.build/framework_macropad_padctl.uf2" "$here/build/"
sha256sum "$here/build/framework_macropad_padctl.uf2"
