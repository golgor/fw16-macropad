# padctl firmware

Framework's stock macropad firmware plus one handler: per-key colour overrides on VIA custom channel 0. Everything else (keymap, VIA, Framework's `0x0B` factory commands, BIOS-mode logic) is unchanged.

License: GPL-2.0-or-later, like QMK. This folder is separate from the Rust crates (MIT OR Apache-2.0) and nothing in the Rust build links it.

## Layout

- `QMK_PIN`: the exact `FrameworkComputer/qmk_firmware` commit (`fl16-2025`) we build against.
- `keymaps/padctl/keymap.c`: Framework's `default` macropad keymap, verbatim, plus the channel-0 handler (`via_custom_value_command_kb`) and the override painter (`rgb_matrix_indicators_advanced_user`).
- `build.sh`: fetches the pinned fork into `~/.cache/fw16-macropad/qmk_firmware`, copies the keymap in, runs `make framework/macropad:padctl`, writes `build/framework_macropad_padctl.uf2`.

The wire format is documented once, in `crates/macropad-proto/src/perkey.rs`, and in ADR 0003.

## Build

```sh
# Arch / Omarchy
sudo pacman -S --needed arm-none-eabi-gcc arm-none-eabi-newlib python-pip
PIP_FLAGS=--break-system-packages firmware/build.sh   # or run inside a venv
```

CI builds the same `.uf2` and uploads it as the `framework_macropad_padctl` artifact.

## Flash (manual, not done by any tool here)

1. Note your current lighting (`padctl light show`); whether VIA settings survive a reflash is untested.
2. Put the macropad in bootloader mode. Framework documents holding two keys while sliding the module in; which two is **unconfirmed** for the macropad (`1`+`6` vs `2`+`6` in the numpad layout). Check Framework's knowledge base for "Laptop 16 RGB Macropad firmware" before trying.
3. An `RPI-RP2` drive appears. Copy `framework_macropad_padctl.uf2` onto it. The pad reboots.
4. `padctl info` should print `per_key: v2 (24 keys)`.

To go back to stock, flash Framework's released macropad `.uf2` (v0.3.1) the same way.
