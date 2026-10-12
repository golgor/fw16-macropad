# AGENTS.md

Host tooling for the Framework 16 RGB Macropad. Read `README.md` (roadmap) and `docs/adr/` before changing structure.

## Rules

- Build layer by layer. Do not start a layer's work until the previous one is merged. No `todo!()` stubs or placeholder commands; a command exists only when it works.
- `macropad-proto` is pure: no I/O, no hidapi, no evdev. All byte layouts live there, encode and decode side by side.
- Never encode or send VIA command `0x0B`. Framework firmware uses it for factory mode and bootloader jump.
- Keys are `KeyId` (physical position 0..24), never keycodes. Construct only via `KeyId::new`.
- Validate at boundaries (CLI args, HID replies). Trust typed values inside.
- Hardware access goes through the `Backend` and `Transport` traits. Every new command gets a test against `macropad_hid::fake`.
- No GPL dependencies (`qmk-via-api` is GPL-3.0). Reference `FrameworkComputer/qmk_hid` (BSD-3) instead. `cargo deny` enforces this.
- Firmware, if added, lives in `firmware/` under GPL-2.0-or-later, separate from the Rust crates.

## Verify

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo deny check licenses bans sources
```
