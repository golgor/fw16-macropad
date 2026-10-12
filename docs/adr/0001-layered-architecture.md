# 0001: Layered architecture with our own VIA layer

Status: accepted (2026-10-12)

## Context

The Framework 16 RGB Macropad runs Framework's QMK fork (`fl16-2025`) with VIA enabled, so the host can talk to it over Raw HID (usage page `0xFF60`, 32-byte reports) without new firmware. Stock VIA gives global RGB matrix control and keymap access but no per-key colour. Per-key colour needs a small custom firmware handler on VIA custom channel 0. Framework repurposes VIA command `0x0B` for factory functions and a bootloader jump.

The end goal is dynamic: host programs claim keys, light them, and react to presses. That needs a clean split between bytes, transport, input and policy so each part can be tested on its own.

## Decision

1. Four crates with one direction of dependency: `padctl` (CLI) depends on `macropad-hid` and later `macropad-input`, which both depend on `macropad-proto`. Nothing depends on `padctl`.
2. `macropad-proto` is pure and owns every byte layout. Commands are an enum, so `0x0B` is unrepresentable rather than forbidden by convention.
3. Hardware sits behind `Backend` (discovery) and `Transport` (one request, one reply). An in-memory fake answers with the same encoders, so CLI behaviour is tested without a device.
4. We write the VIA layer ourselves. It is about fifteen small commands. `srwi/qmk-via-api` is GPL-3.0-only and would make the binary GPL-3.0. `FrameworkComputer/qmk_hid` (BSD-3-Clause) is the reference for byte layouts and Framework quirks.
5. Delivery is layered: skeleton, raw LED control, simple API, dynamic use cases. Each layer lands complete before the next begins.
6. `hidapi` with `linux-native-basic-udev`: pure Rust, no libudev or C hidapi needed on the build host.

## Consequences

- Adding a command means one enum variant plus its encode and decode in one file, and a fake response.
- We carry protocol tests ourselves, with golden bytes from QMK `via.c` and `qmk_hid`.
- Custom firmware, when it comes, is a separate GPL-2.0-or-later part under `firmware/` and only extends channel 0.

## Rejected

- Depending on `qmk-via-api`: license.
- Vial: stale Framework port, drops Framework BIOS-mode logic.
- keyd or compositor binds as the core: static bindings, no feedback channel.
