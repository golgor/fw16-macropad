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
sudo pacman -S --needed arm-none-eabi-gcc arm-none-eabi-newlib
# Debian / Ubuntu: sudo apt install gcc-arm-none-eabi libnewlib-arm-none-eabi
mise install          # provides uv
mise run firmware     # or firmware/build.sh
```

`build.sh` checks its prerequisites before cloning anything and prints the package names if one is missing.

CI builds the same `.uf2` and uploads it as the `framework_macropad_padctl` artifact.

## Flash

See the Flashing section of the top-level README (`mise run flash`, key-hold fallbacks, restoring stock v0.3.1).
