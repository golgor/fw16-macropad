// Copyright 2022 Framework Computer
// Copyright 2026 Robert Nyström (padctl per-key additions)
// SPDX-License-Identifier: GPL-2.0-or-later

#include QMK_KEYBOARD_H
#include "factory.h"
#include "via.h"

/*
 * padctl v3: one host layer, 24 distinct key codes, no on-pad functions.
 *
 * The host (padd) grabs the evdev device and maps each code back to a key
 * position, so every key must send a code no other key sends, and nothing
 * may be consumed inside the firmware. Codes are HID usages the desktop never
 * sees from a normal keyboard; Linux names in brackets. Table mirrored in
 * crates/macropad-input/src/keymap.rs (Keymap::padctl_v3).
 *
 *   key  0- 3  F13  F14  F15  F16       [KEY_F13..KEY_F16]
 *   key  4- 7  F17  F18  F19  F20       [KEY_F17..KEY_F20]
 *   key  8-11  F21  F22  F23  F24       [KEY_F21..KEY_F24]
 *   key 12-15  EXEC HELP MENU SLCT      [KEY_OPEN KEY_HELP KEY_PROPS KEY_FRONT]
 *   key 16-19  STOP AGIN UNDO CUT       [KEY_STOP KEY_AGAIN KEY_UNDO KEY_CUT]
 *   key 20-23  COPY PSTE FIND INT1      [KEY_COPY KEY_PASTE KEY_FIND KEY_RO]
 *
 * Framework's Num Lock layer switch and the on-pad RGB/backlight keys are
 * gone on purpose (ADR 0004). _FN stays empty so a stray layer_on is harmless;
 * _FACTORY is Framework's, used by their factory tooling only.
 */
const uint16_t PROGMEM keymaps[][MATRIX_ROWS][MATRIX_COLS] = {
    [_NUMLOCK] = LAYOUT(
        KC_F13,     KC_F14,   KC_F15,   KC_F16,
        KC_F17,     KC_F18,   KC_F19,   KC_F20,
        KC_F21,     KC_F22,   KC_F23,   KC_F24,
        KC_EXECUTE, KC_HELP,  KC_MENU,  KC_SELECT,
        KC_STOP,    KC_AGAIN, KC_UNDO,  KC_CUT,
        KC_COPY,    KC_PASTE, KC_FIND,  KC_INTERNATIONAL_1
    ),
    [_FN] = LAYOUT(
        _______, _______, _______, _______,
        _______, _______, _______, _______,
        _______, _______, _______, _______,
        _______, _______, _______, _______,
        _______, _______, _______, _______,
        _______, _______, _______, _______
    ),
    [_FACTORY] = LAYOUT(
        KC_A,    KC_B,    KC_C,    KC_D,
        KC_E,    KC_F,    KC_G,    KC_H,
        KC_I,    KC_J,    KC_K,    KC_L,
        KC_M,    KC_N,    KC_O,    KC_P,
        KC_Q,    KC_R,    KC_S,    KC_T,
        KC_U,    KC_V,    KC_W,    KC_X
    ),
};

/*
 * VIA keeps a copy of the keymap in EEPROM and only re-seeds it when its magic
 * (the build *date*) changes, so a same-day reflash from an older padctl
 * keymap would keep the old codes. We stamp the user EEPROM word with our
 * keymap version and force VIA to re-seed when it differs. This runs inside
 * via_init() before the magic check.
 */
#define PADCTL_KEYMAP_STAMP (0x70640000UL | PADCTL_PROTOCOL_VERSION) // "pd" + version

void via_init_kb(void) {
    if (eeconfig_read_user() != PADCTL_KEYMAP_STAMP) {
        via_eeprom_set_valid(false);
        eeconfig_update_user(PADCTL_KEYMAP_STAMP);
    }
}

void enable_factory_mode(bool enable) {
    if (enable)
        layer_on(_FACTORY);
    else
        layer_off(_FACTORY);
}

/*
 * padctl per-key sub-protocol on VIA custom channel 0.
 *
 * Everything above this comment is the v3 host layer (Framework's factory
 * layer kept).
 * Below, the host can override the colour of individual keys. Overrides are
 * painted after the active RGB matrix effect, live only in RAM, and are never
 * written to EEPROM. Byte layout: crates/macropad-proto/src/perkey.rs.
 *
 *   data[0] id_custom_set_value (0x07) or id_custom_get_value (0x08)
 *   data[1] id_custom_channel (0x00)
 *   data[2] padctl value id
 *   data[3..] arguments
 *
 * VIA echoes the buffer back after this handler. Anything we do not accept
 * gets id_unhandled (0xFF) in data[0].
 */

enum padctl_value_id {
    padctl_probe     = 0x01, // get: -> 'p' 'd' version key_count
    padctl_set_key   = 0x02, // set: key r g b
    padctl_set_many  = 0x03, // set: count (key r g b) * count, count 1..7
    padctl_clear_all = 0x04, // set: -
    padctl_clear_key = 0x05, // set: key
    padctl_set_exclusive = 0x06, // set: 0 overlay, 1 exclusive (default)
};

#define PADCTL_KEYS 24
#define PADCTL_MAX_BATCH 7

// Host KeyId (row-major from the top left, as the LAYOUT macro lists keys)
// to switch matrix position. LED indices come from g_led_config at runtime, so
// this table only restates macropad.h's LAYOUT.
static const uint8_t padctl_key_pos[PADCTL_KEYS][2] = {
    {2, 1}, {2, 2}, {3, 4}, {2, 4}, // H1  H2   H3   H4
    {0, 0}, {0, 4}, {1, 1}, {1, 6}, // K90 K95  K100 K105
    {0, 1}, {0, 5}, {1, 2}, {2, 5}, // K91 K96  K101 K107
    {0, 2}, {0, 6}, {1, 3}, {1, 7}, // K92 K97  K102 K106
    {0, 3}, {0, 7}, {1, 4}, {2, 6}, // K93 K98  K103 K109
    {1, 0}, {2, 7}, {1, 5}, {2, 0}, // K99 K110 K104 K108
};

static uint8_t  padctl_rgb[PADCTL_KEYS][3];
static uint32_t padctl_active; // bit n set: key n is overridden
// Exclusive (default): while any override is active, every other LED is
// painted black so only overridden keys are lit, whatever the effect.
static bool padctl_exclusive = true;
// The matrix was off and we switched it on for overrides. When the last
// override clears we switch it off again, unless the user picked an effect
// in between (mode changed).
static bool    padctl_enabled_by_us;
static uint8_t padctl_enabled_mode;

static void padctl_after_change(void) {
    if (padctl_active && !rgb_matrix_is_enabled()) {
        rgb_matrix_enable_noeeprom();
        padctl_enabled_by_us = true;
        padctl_enabled_mode  = rgb_matrix_get_mode();
    } else if (!padctl_active && padctl_enabled_by_us) {
        padctl_enabled_by_us = false;
        if (rgb_matrix_is_enabled() && rgb_matrix_get_mode() == padctl_enabled_mode) {
            rgb_matrix_disable_noeeprom();
        }
    }
}

static void padctl_set(uint8_t key, const uint8_t *rgb) {
    padctl_rgb[key][0] = rgb[0];
    padctl_rgb[key][1] = rgb[1];
    padctl_rgb[key][2] = rgb[2];
    padctl_active |= (1UL << key);
}

static bool padctl_set_handle(uint8_t value, uint8_t *args);

static bool padctl_handle(uint8_t command, uint8_t value, uint8_t *args) {
    if (command == id_custom_get_value) {
        if (value != padctl_probe) return false;
        args[0] = 'p';
        args[1] = 'd';
        args[2] = PADCTL_PROTOCOL_VERSION;
        args[3] = PADCTL_KEYS;
        return true;
    }
    if (command != id_custom_set_value) return false; // no save: never persist
    // Overrides are drawn by the indicator hook, which only runs while the
    // matrix is on, so changes may toggle it (never persisted).
    bool ok = padctl_set_handle(value, args);
    if (ok) padctl_after_change();
    return ok;
}

static bool padctl_set_handle(uint8_t value, uint8_t *args) {
    switch (value) {
        case padctl_set_key:
            if (args[0] >= PADCTL_KEYS) return false;
            padctl_set(args[0], &args[1]);
            return true;
        case padctl_set_many: {
            uint8_t count = args[0];
            if (count == 0 || count > PADCTL_MAX_BATCH) return false;
            for (uint8_t i = 0; i < count; i++) {
                if (args[1 + 4 * i] >= PADCTL_KEYS) return false;
            }
            for (uint8_t i = 0; i < count; i++) {
                padctl_set(args[1 + 4 * i], &args[2 + 4 * i]);
            }
            return true;
        }
        case padctl_clear_all:
            padctl_active = 0;
            return true;
        case padctl_clear_key:
            if (args[0] >= PADCTL_KEYS) return false;
            padctl_active &= ~(1UL << args[0]);
            return true;
        case padctl_set_exclusive:
            if (args[0] > 1) return false;
            padctl_exclusive = args[0];
            return true;
        default:
            return false;
    }
}

void via_custom_value_command_kb(uint8_t *data, uint8_t length) {
    (void)length; // always the 32-byte Raw HID report
    if (data[1] != id_custom_channel || !padctl_handle(data[0], data[2], &data[3])) {
        data[0] = id_unhandled;
    }
}

bool rgb_matrix_indicators_advanced_user(uint8_t led_min, uint8_t led_max) {
    if (!padctl_active) return false;
    if (padctl_exclusive) {
        for (uint8_t led = led_min; led < led_max; led++) {
            rgb_matrix_set_color(led, 0, 0, 0);
        }
    }
    for (uint8_t key = 0; key < PADCTL_KEYS; key++) {
        if (!(padctl_active & (1UL << key))) continue;
        uint8_t led = g_led_config.matrix_co[padctl_key_pos[key][0]][padctl_key_pos[key][1]];
        if (led == NO_LED || led < led_min || led >= led_max) continue;
        rgb_matrix_set_color(led, padctl_rgb[key][0], padctl_rgb[key][1], padctl_rgb[key][2]);
    }
    return false;
}
