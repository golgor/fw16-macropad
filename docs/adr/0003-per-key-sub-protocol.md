# 0003: Per-key colour through a channel-0 sub-protocol

Status: proposed (2026-10-12)

## Context

Stock firmware can only light all keys alike. The notification use case needs individual keys. QMK's VIA routes `id_custom_set_value`/`id_custom_get_value` on channel 0 (`id_custom_channel`) to the weak hook `via_custom_value_command_kb`, which stock firmware leaves unimplemented (it answers `0xFF`).

## Decision

1. Firmware: a keymap, `firmware/keymaps/padctl`, built against a pinned `fl16-2025` commit. It is Framework's default keymap plus the hook and an `rgb_matrix_indicators_advanced_user` painter. No board code or QMK core is forked.
2. Wire format, all 32-byte reports, `[cmd, 0x00, value, args..]`:

   | value | cmd | args | reply |
   |-------|-----|------|-------|
   | `0x01` probe | `0x08` | - | `'p' 'd' version key_count` |
   | `0x02` set key | `0x07` | `key r g b` | echo |
   | `0x03` set many | `0x07` | `n (key r g b)*n`, `n` 1..7 | echo |
   | `0x04` clear all | `0x07` | - | echo |
   | `0x05` clear key | `0x07` | `key` | echo |

   Invalid value ids, keys or counts get `0xFF` in byte 0. `0x09` (save) is not handled: overrides are RAM-only.
3. Keys are host `KeyId`s (row-major from the top left, in `LAYOUT` order). The firmware maps them to switch-matrix positions and reads the LED index from `g_led_config`, so LED wiring stays Framework's business.
4. Host: `Macropad::per_key()` probes and returns a `PerKeyLeds` capability handle. Stock firmware yields `HidError::NoPerKey`, so per-key calls cannot be made without a positive probe. The probe checks the `pd` magic and the version.
5. Setting an override turns the RGB matrix on (not persisted) if it was off, since indicators only paint while it runs. Unoverridden keys show the current effect.

## Consequences

- One report carries up to 7 keys; all 24 take 4 reports. Batches are not atomic across reports.
- Overrides are lost on power loss or reflash, which suits transient notifications.
- Upgrading Framework firmware means bumping `firmware/QMK_PIN`, rebuilding, and reflashing.

## Rejected

- Raw HID outside VIA (`via_command_kb`): Framework already uses it for `0x0B`; staying inside VIA's custom channel keeps us out of its way.
- Persisting overrides in EEPROM: wear, and stale lights after a crash.
- Rust firmware (RMK, Embassy): rewrites the LED driver, matrix and Framework logic for no gain.
