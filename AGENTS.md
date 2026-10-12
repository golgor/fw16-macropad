# AGENTS.md

Host tooling for the Framework 16 RGB Macropad. Read `README.md` (roadmap) and `docs/adr/` before changing structure.

## Rules

- Build layer by layer. Do not start a layer's work until the previous one is merged. No `todo!()` stubs or placeholder commands; a command exists only when it works.
- `macropad-proto` is pure: no I/O, no hidapi, no evdev. All byte layouts live there, encode and decode side by side.
- `0x0B` is Framework's factory command. Only `macropad_proto::BootloaderJump` (sub-command `0xFE`) may encode it, only `Macropad::jump_to_bootloader` sends it, only `padctl bootloader` (with confirmation) calls that. Never add other `0x0B` encoders or a `Command` variant for it.
- Keys are `KeyId` (physical position 0..24), never keycodes. Construct only via `KeyId::new`.
- Validate at boundaries (CLI args, HID replies). Trust typed values inside.
- Hardware access goes through the `Backend` and `Transport` traits. Every new command gets a test against `macropad_hid::fake`.
- No GPL dependencies (`qmk-via-api` is GPL-3.0). Reference `FrameworkComputer/qmk_hid` (BSD-3) instead. `cargo deny` enforces this.
- Firmware lives in `firmware/` under GPL-2.0-or-later, separate from the Rust crates. Channel-0 value ids in `keymap.c` and `macropad-proto/src/perkey.rs` must change together (ADR 0003).
- Never flash a device from automation. Flashing is Robert's manual step.
- Nothing writes EEPROM implicitly; only explicit save commands. The one exception is the firmware's once-per-version VIA keymap reset stamp (`via_init_kb`, see firmware/README).
- `padd` is the only process touching the device while it runs. Clients go through `macropad-client`/the socket. `padctl light`/`key` refuse unless `--force`. Socket wire types live only in `macropad-ipc`, and every doc example in `docs/protocol.md` must round-trip in its tests.
- Every new daemon behaviour gets a fake end-to-end test (`crates/padd/tests/e2e.rs`, fake input + fake LEDs).

## Verify

`mise run ci` and `mise run firmware`, or by hand:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo deny check licenses bans sources
firmware/build.sh   # needs arm-none-eabi-gcc; CI runs it
```
