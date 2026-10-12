# 0002: Mapping `Rgb` to the stock firmware's HSV

Status: proposed (2026-10-12)

## Context

The host speaks `Rgb` everywhere (`macropad-proto::Rgb`), because callers think in colours and the per-key path takes RGB directly. Stock VIA lighting (channel 3) only accepts colour as hue and saturation (`id_qmk_rgb_matrix_color`) and value as brightness (`id_qmk_rgb_matrix_brightness`), all on QMK's 0..=255 scale. The firmware then scales brightness by `RGB_MATRIX_MAXIMUM_BRIGHTNESS`.

## Decision

1. Conversion happens on the host, in `macropad-proto` (`impl From<Rgb> for Hsv`), with integer maths on QMK's scale: one sixth of the wheel is 43 steps, so red 0, yellow 43, green 85, blue 171, magenta 213, matching QMK's `HSV_*` constants.
2. `Macropad::set_solid(rgb)` sends effect `SOLID_COLOR` (1), then hue and saturation, then `v` as brightness. Three reports, none persisted.
3. Black (`v == 0`) sends effect `OFF` (0) instead, because brightness 0 alone leaves an effect running invisibly and VIA treats effect 0 as "matrix off".
4. Nothing is written to EEPROM unless the caller asks (`save_lighting`, `padctl light save`). Flash wear and surprise both argue for explicit saves.
5. Lower-level setters (`set_hue_sat`, `set_brightness`, `set_effect`, `set_speed`) stay public for callers that want HSV or effects directly.

## Consequences

- Round-tripping is lossy (8-bit HSV, and the firmware's maximum brightness caps `v`). Fine for indicators; not a colour-accurate path.
- The global colour shown is QMK's `hsv_to_rgb` of our HSV, so it can differ by a step or two from the requested RGB.
- The per-key path (ADR 0003) is unaffected: it sends raw RGB and the firmware paints it without HSV conversion or brightness scaling.

## Rejected

- Asking firmware to convert: stock firmware cannot, and it would put the same logic in two languages.
- Mapping `v` into saturation or ignoring it: dims nothing, surprises users who pass a dark colour.
