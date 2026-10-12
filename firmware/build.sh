#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-2.0-or-later
# Build the padctl keymap against Framework's QMK fork at the pinned commit.
# Output: firmware/build/framework_macropad_padctl.uf2. Flashing is manual.
#
# Needs: git, make, uv, arm-none-eabi-gcc + newlib. uv provides Python and a
# venv for the qmk CLI and QMK's requirements; nothing touches system Python.
# QMK checkout and venv are cached under $QMK_DIR and $QMK_VENV.
set -euo pipefail

# --- preflight: fail fast, before any clone or install -----------------------
missing=()
command -v git >/dev/null || missing+=("git")
command -v make >/dev/null || missing+=("make")
command -v uv >/dev/null || missing+=("uv (mise install, or pacman -S uv)")
if ! command -v arm-none-eabi-gcc >/dev/null; then
    missing+=("arm-none-eabi-gcc")
elif ! echo '#include <string.h>' | arm-none-eabi-gcc -E -x c - >/dev/null 2>&1; then
    missing+=("newlib headers for arm-none-eabi-gcc")
fi
if ((${#missing[@]})); then
    {
        echo "firmware/build.sh: missing prerequisites:"
        printf '  - %s\n' "${missing[@]}"
        echo
        echo "Arch / Omarchy:  sudo pacman -S --needed arm-none-eabi-gcc arm-none-eabi-newlib make git"
        echo "Debian / Ubuntu: sudo apt install gcc-arm-none-eabi libnewlib-arm-none-eabi make git"
        echo "uv:              mise install (from the repo root), or see https://docs.astral.sh/uv/"
    } >&2
    exit 1
fi

here="$(cd "$(dirname "$0")" && pwd)"
# shellcheck source=/dev/null
source <(grep -E '^(REPO|BRANCH|COMMIT)=' "$here/QMK_PIN")
cache="${XDG_CACHE_HOME:-$HOME/.cache}/fw16-macropad"
QMK_DIR="${QMK_DIR:-$cache/qmk_firmware}"
QMK_VENV="${QMK_VENV:-$cache/venv}"
# QMK's Python tooling emits SyntaxWarnings (newer Python) and milc deprecation
# UserWarnings; both are noise for this build.
export PYTHONWARNINGS="ignore::SyntaxWarning,ignore::UserWarning"

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

[[ -x "$QMK_VENV/bin/python" ]] || uv venv -q --python 3.12 "$QMK_VENV"
uv pip install -q --python "$QMK_VENV/bin/python" qmk -r "$QMK_DIR/requirements.txt"
export PATH="$QMK_VENV/bin:$PATH" VIRTUAL_ENV="$QMK_VENV"

keymap="$QMK_DIR/keyboards/framework/macropad/keymaps/padctl"
rm -rf "$keymap"
cp -r "$here/keymaps/padctl" "$keymap"

make -C "$QMK_DIR" -j"$(nproc)" framework/macropad:padctl SKIP_GIT=yes

mkdir -p "$here/build"
cp "$QMK_DIR/.build/framework_macropad_padctl.uf2" "$here/build/"
sha256sum "$here/build/framework_macropad_padctl.uf2"
