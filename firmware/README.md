# padctl firmware

Framework's stock macropad firmware with two changes: per-key colour overrides on VIA custom channel 0, and since **v3** a host-only keymap. Everything else is unchanged: VIA, Framework's `0x0B` factory commands and the BIOS-mode logic.

## v3 keymap

There is one layer of 24 distinct key codes, so padd can tell every key apart: keys 0-11 send F13-F24, keys 12-23 send Execute, Help, Menu, Select, Stop, Again, Undo, Cut, Copy, Paste, Find and International1 (table in ADR 0004). The on-pad RGB and Num Lock functions are gone; lighting is controlled from the host. Without padd running these codes reach the desktop, where they usually do nothing.

VIA keeps the keymap in EEPROM. Its reset trigger depends only on the build date, so the keymap stamps the padctl version into the user EEPROM word (`via_init_kb`). On the first boot after flashing a different version, the stored VIA keymap is reset to this one. This is the only EEPROM write the firmware makes on its own, and it happens once per version change. `padctl info` reports `per_key: v3`.

License: GPL-2.0-or-later, like QMK. This folder is separate from the Rust crates (MIT OR Apache-2.0) and nothing in the Rust build links it.

## Layout

- `QMK_PIN`: the exact `FrameworkComputer/qmk_firmware` commit (`fl16-2025`) we build against.
- `keymaps/padctl/keymap.c`: Framework's `default` macropad keymap, with the v3 host layer, plus the version stamp, the channel-0 handler (`via_custom_value_command_kb`) and the override painter (`rgb_matrix_indicators_advanced_user`).
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
