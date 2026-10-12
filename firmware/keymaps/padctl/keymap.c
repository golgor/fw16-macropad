// Copyright 2022 Framework Computer
// Copyright 2026 Robert Nyström (padctl per-key additions)
// SPDX-License-Identifier: GPL-2.0-or-later

#include QMK_KEYBOARD_H
#include "factory.h"
#include "via.h"

const uint16_t PROGMEM keymaps[][MATRIX_ROWS][MATRIX_COLS] = {
     /*
     *         ┌────┬────┬────┬────┐
     *  4 keys │Esc │Calc│ =  │ <- │
     *         ├────┼────┼────┼────┤
     *  4 keys │ Num│ /  │ *  │ -  │
     *         ├────┼────┼────┼────┤
     *  3 keys │ 7  │ 8  │ 9  │ +  │
     *         ├────┼────┼────┼────┤
     *  4 keys │ 4  │ 5  │ 6  │ +  │
     *         ├────┼────┼────┼────┤
     *  3 keys │ 1  │ 2  │ 3  │Entr│
     *         ├────┼────┼────┼────┤
     *  3 keys │ 0  │ 0  │ .  │Entr│
     *         └────┴────┴────┴────┴
     * 21 total
     */
    [_NUMLOCK] = LAYOUT(
        KC_ESC,  KC_CALC, KC_EQL,  KC_BSPC,
        KC_NUM,  KC_PSLS, KC_PAST, KC_PMNS,
        KC_P7,   KC_P8,   KC_P9,   KC_PPLS,
        KC_P4,   KC_P5,   KC_P6,   KC_PPLS,
        KC_P1,   KC_P2,   KC_P3,   KC_PENT,
        KC_P0,   KC_P0,   KC_PDOT, KC_PENT
    ),
     /*
     *         ┌───────┬───────┬───────┬───────┐
     *  4 keys │ RGB   │ RGB + │ RGB + │ RGB + │
     *         │Toggle │ Speed │ Hue   │ Sat   │
     *         ├───────┼───────┼───────┼───────┤
     *  4 keys │Numlock│ RGB - │ RGB - │ RGB - │
     *         │       │ Speed │ Hue   │ Sat   │
     *         ├───────┼───────┼───────┼───────┤
     *  4 keys │ Home  │   ↑   │ Page  │RGB Nxt│
     *         │       │       │ Up    │Effect │
     *         ├───────┼───────┼───────┼───────┤
     *  4 keys │  ←    │       │   →   │RGB Prv│
     *         │       │       │       │Effect │
     *         ├───────┼───────┼───────┼───────┤
     *  4 keys │ End   │  ↓    │ Page  │ BL    │
     *         │       │       │ Down  │ Step  │
     *         ├───────┼───────┼───────┼───────┤
     *  4 keys │ Insert│ Insert│ Delete│ BL    │
     *         │       │       │       │ Step  │
     *         └───────┴───────┴───────┴───────┘
     * 24 total
     */
    [_FN] = LAYOUT(
        RGB_TOG, RGB_SPI, RGB_HUI, RGB_SAI,
        _______, RGB_SPD, RGB_HUD, RGB_SAD,
        _______, _______, _______, RGB_MOD,
        _______, _______, _______, RGB_RMOD,
        _______, _______, _______, BL_STEP,
        _______, _______, _______, BL_STEP
    ),
     /* Alphabet
     *         ┌────┬────┬────┬────┐
     *  4 keys │ A  │ B  │ C  │ D  │
     *         ├────┼────┼────┼────┤
     *  4 keys │ E  │ F  │ G  │ H  │
     *         ├────┼────┼────┼────┤
     *  4 keys │ I  │ J  │ K  │ L  │
     *         ├────┼────┼────┼────┤
     *  4 keys │ M  │ N  │ O  │ P  │
     *         ├────┼────┼────┼────┤
     *  4 keys │ Q  │ R  │ S  │ T  │
     *         ├────┼────┼────┼────┤
     *  4 keys │ U  │ V  │ W  │ X  │
     *         └────┴────┴────┴────┘
     * 24 total
     */
    [_FACTORY] = LAYOUT(
        KC_A,    KC_B,    KC_C,    KC_D,
        KC_E,    KC_F,    KC_G,    KC_H,
        KC_I,    KC_J,    KC_K,    KC_L,
        KC_M,    KC_N,    KC_O,    KC_P,
        KC_Q,    KC_R,    KC_S,    KC_T,
        KC_U,    KC_V,    KC_W,    KC_X
    ),
};

bool led_update_user(led_t led_state) {
    // Change layer if numlock state changes, either triggered by OS or
    // by numlock key on this keyboard
    if (led_state.num_lock) {
        layer_off(_FN);
    } else {
        layer_on(_FN);
    }
    return true;
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
 * Everything above this comment is Framework's default keymap, unchanged.
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
