use crate::key::{KeyId, Rgb};
use crate::perkey::{self, KeyBatch, MAGIC, PerKeyInfo};
use crate::report::Report;

/// VIA command ids this crate knows how to encode. Kept private so callers go
/// through [`Command`], which is what keeps `0x0B` unreachable.
mod id {
    pub const GET_PROTOCOL_VERSION: u8 = 0x01;
    pub const CUSTOM_SET_VALUE: u8 = 0x07;
    pub const CUSTOM_GET_VALUE: u8 = 0x08;
    pub const CUSTOM_SAVE: u8 = 0x09;
    /// VIA's reply byte for a command the firmware did not handle.
    pub const UNHANDLED: u8 = 0xFF;
}

/// Stock QMK RGB matrix channel and its value ids (`via.h`, `qmk_hid`
/// `ViaRgbMatrixValue`).
mod rgb_matrix {
    pub const CHANNEL: u8 = 0x03;
    pub const BRIGHTNESS: u8 = 0x01;
    pub const EFFECT: u8 = 0x02;
    pub const EFFECT_SPEED: u8 = 0x03;
    pub const COLOR: u8 = 0x04;
}

/// An RGB matrix effect index as the firmware numbers it.
///
/// Indices depend on which effects the firmware was built with, so only the two
/// that QMK fixes are named. `0` is special in VIA: it turns the matrix off.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Effect(pub u8);

impl Effect {
    /// Matrix off (VIA calls `rgb_matrix_disable_noeeprom`).
    pub const OFF: Self = Self(0);
    /// `RGB_MATRIX_SOLID_COLOR`, always the first QMK effect.
    pub const SOLID_COLOR: Self = Self(1);
}

/// A request the host can send. Each variant owns its encoding in
/// [`Command::encode`] and its decoding in [`Command::decode`].
///
/// There are three families:
/// - VIA core: [`Command::GetProtocolVersion`].
/// - Stock lighting on VIA channel 3 (RGB matrix): global for all keys, works
///   on unmodified Framework firmware. Changes are not persisted until
///   [`Command::SaveLighting`].
/// - Per-key on VIA channel 0: only our `padctl` keymap answers these. Never
///   persisted; overrides vanish on power loss.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    /// Ask which VIA protocol version the firmware speaks.
    GetProtocolVersion,
    /// Global brightness, `0..=255`, scaled by the firmware's maximum.
    SetBrightness(u8),
    /// Read global brightness.
    GetBrightness,
    /// Select an RGB matrix effect.
    SetEffect(Effect),
    /// Read the current effect.
    GetEffect,
    /// Effect animation speed.
    SetEffectSpeed(u8),
    /// Read effect speed.
    GetEffectSpeed,
    /// Global hue and saturation (value stays with brightness).
    SetColor {
        /// Hue, QMK scale.
        hue: u8,
        /// Saturation.
        sat: u8,
    },
    /// Read global hue and saturation.
    GetColor,
    /// Persist the current global lighting to EEPROM.
    SaveLighting,
    /// Ask whether the firmware speaks our per-key sub-protocol.
    ProbePerKey,
    /// Override one key's colour.
    SetKey(KeyId, Rgb),
    /// Override up to seven keys in one report.
    SetMany(KeyBatch),
    /// Drop one key's override so it shows the effect again.
    ClearKey(KeyId),
    /// Drop every override.
    ClearAllKeys,
    /// Exclusive mode (default on): while any override is active, every
    /// other key is dark. Off: overrides sit on top of the running effect.
    SetExclusive(bool),
}

/// A decoded reply.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Response {
    /// Reply to [`Command::GetProtocolVersion`].
    ProtocolVersion(ProtocolVersion),
    /// Any set or save command was accepted.
    Ack,
    /// Reply to [`Command::GetBrightness`].
    Brightness(u8),
    /// Reply to [`Command::GetEffect`].
    Effect(Effect),
    /// Reply to [`Command::GetEffectSpeed`].
    EffectSpeed(u8),
    /// Reply to [`Command::GetColor`].
    Color {
        /// Hue.
        hue: u8,
        /// Saturation.
        sat: u8,
    },
    /// Reply to [`Command::ProbePerKey`].
    PerKey(PerKeyInfo),
}

/// VIA protocol version as reported by the firmware (Framework's `fl16-2025`
/// branch reports 12, upstream QMK 13).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ProtocolVersion(pub u16);

/// Why a reply could not be decoded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ProtoError {
    /// The firmware answered `0xFF`: it does not implement this command, or
    /// rejected its arguments.
    #[error("firmware does not handle command {0:?}")]
    Unhandled(Command),
    /// The reply does not echo the request header.
    #[error("reply header {got:02x?} does not echo request {expected:02x?}")]
    WrongEcho {
        /// Header we sent (command, channel, value id; unused bytes zero).
        expected: [u8; 3],
        /// Header in the reply.
        got: [u8; 3],
    },
    /// Channel 0 answered but without our magic: some other custom firmware.
    #[error("channel 0 is answered by firmware that is not padctl's")]
    ForeignFirmware,
}

impl Command {
    /// `(command id, channel, value id)`, and how many header bytes are real.
    const fn header(self) -> ([u8; 3], usize) {
        use id::{CUSTOM_GET_VALUE, CUSTOM_SAVE, CUSTOM_SET_VALUE, GET_PROTOCOL_VERSION};
        use {perkey::value as pk, rgb_matrix as rm};
        let (bytes, len) = match self {
            Self::GetProtocolVersion => ([GET_PROTOCOL_VERSION, 0, 0], 1),
            Self::SetBrightness(_) => ([CUSTOM_SET_VALUE, rm::CHANNEL, rm::BRIGHTNESS], 3),
            Self::GetBrightness => ([CUSTOM_GET_VALUE, rm::CHANNEL, rm::BRIGHTNESS], 3),
            Self::SetEffect(_) => ([CUSTOM_SET_VALUE, rm::CHANNEL, rm::EFFECT], 3),
            Self::GetEffect => ([CUSTOM_GET_VALUE, rm::CHANNEL, rm::EFFECT], 3),
            Self::SetEffectSpeed(_) => ([CUSTOM_SET_VALUE, rm::CHANNEL, rm::EFFECT_SPEED], 3),
            Self::GetEffectSpeed => ([CUSTOM_GET_VALUE, rm::CHANNEL, rm::EFFECT_SPEED], 3),
            Self::SetColor { .. } => ([CUSTOM_SET_VALUE, rm::CHANNEL, rm::COLOR], 3),
            Self::GetColor => ([CUSTOM_GET_VALUE, rm::CHANNEL, rm::COLOR], 3),
            Self::SaveLighting => ([CUSTOM_SAVE, rm::CHANNEL, 0], 2),
            Self::ProbePerKey => ([CUSTOM_GET_VALUE, perkey::CHANNEL, pk::PROBE], 3),
            Self::SetKey(..) => ([CUSTOM_SET_VALUE, perkey::CHANNEL, pk::SET_KEY], 3),
            Self::SetMany(_) => ([CUSTOM_SET_VALUE, perkey::CHANNEL, pk::SET_MANY], 3),
            Self::ClearKey(_) => ([CUSTOM_SET_VALUE, perkey::CHANNEL, pk::CLEAR_KEY], 3),
            Self::ClearAllKeys => ([CUSTOM_SET_VALUE, perkey::CHANNEL, pk::CLEAR_ALL], 3),
            Self::SetExclusive(_) => ([CUSTOM_SET_VALUE, perkey::CHANNEL, pk::SET_EXCLUSIVE], 3),
        };
        (bytes, len)
    }

    /// Frame this command as one report.
    #[must_use]
    pub fn encode(self) -> Report {
        let mut report = Report::zeroed();
        let (header, len) = self.header();
        report.0[..len].copy_from_slice(&header[..len]);
        let args = &mut report.0[len..];
        match self {
            Self::SetBrightness(v) | Self::SetEffectSpeed(v) | Self::SetEffect(Effect(v)) => {
                args[0] = v;
            }
            Self::SetColor { hue, sat } => args[..2].copy_from_slice(&[hue, sat]),
            Self::SetKey(key, Rgb { r, g, b }) => {
                args[..4].copy_from_slice(&[key.index(), r, g, b]);
            }
            Self::SetMany(batch) => batch.write(args),
            Self::ClearKey(key) => args[0] = key.index(),
            Self::SetExclusive(on) => args[0] = u8::from(on),
            Self::GetProtocolVersion
            | Self::GetBrightness
            | Self::GetEffect
            | Self::GetEffectSpeed
            | Self::GetColor
            | Self::SaveLighting
            | Self::ProbePerKey
            | Self::ClearAllKeys => {}
        }
        report
    }

    /// Decode the firmware's reply to this command.
    ///
    /// # Errors
    /// [`ProtoError`] when the reply is not a valid answer to `self`.
    pub fn decode(self, reply: &Report) -> Result<Response, ProtoError> {
        let bytes = reply.as_bytes();
        if bytes[0] == id::UNHANDLED {
            return Err(ProtoError::Unhandled(self));
        }
        let (header, len) = self.header();
        if bytes[..len] != header[..len] {
            let mut got = [0; 3];
            got[..len].copy_from_slice(&bytes[..len]);
            return Err(ProtoError::WrongEcho {
                expected: header,
                got,
            });
        }
        let p = &bytes[len..];
        Ok(match self {
            Self::GetProtocolVersion => {
                Response::ProtocolVersion(ProtocolVersion(u16::from_be_bytes([p[0], p[1]])))
            }
            Self::GetBrightness => Response::Brightness(p[0]),
            Self::GetEffect => Response::Effect(Effect(p[0])),
            Self::GetEffectSpeed => Response::EffectSpeed(p[0]),
            Self::GetColor => Response::Color {
                hue: p[0],
                sat: p[1],
            },
            Self::ProbePerKey => {
                if p[..2] != MAGIC {
                    return Err(ProtoError::ForeignFirmware);
                }
                Response::PerKey(PerKeyInfo {
                    version: p[2],
                    key_count: p[3],
                })
            }
            Self::SetBrightness(_)
            | Self::SetEffect(_)
            | Self::SetEffectSpeed(_)
            | Self::SetColor { .. }
            | Self::SaveLighting
            | Self::SetKey(..)
            | Self::SetMany(_)
            | Self::ClearKey(_)
            | Self::ClearAllKeys
            | Self::SetExclusive(_) => Response::Ack,
        })
    }

    /// Parse a request the way firmware would: the inverse of
    /// [`Command::encode`]. Used by fake devices so they emulate firmware from
    /// the same layout. `None` for anything this crate cannot encode
    /// (including `0x0B`) or invalid arguments.
    #[must_use]
    pub fn parse(request: &Report) -> Option<Self> {
        use id::{CUSTOM_GET_VALUE, CUSTOM_SAVE, CUSTOM_SET_VALUE, GET_PROTOCOL_VERSION};
        use {perkey::value as pk, rgb_matrix as rm};
        let b = request.as_bytes();
        let a = &b[3..];
        let key = |i: u8| KeyId::new(i).ok();
        Some(match (b[0], b[1], b[2]) {
            (GET_PROTOCOL_VERSION, ..) => Self::GetProtocolVersion,
            (CUSTOM_SAVE, rm::CHANNEL, _) => Self::SaveLighting,
            (CUSTOM_SET_VALUE, rm::CHANNEL, rm::BRIGHTNESS) => Self::SetBrightness(a[0]),
            (CUSTOM_GET_VALUE, rm::CHANNEL, rm::BRIGHTNESS) => Self::GetBrightness,
            (CUSTOM_SET_VALUE, rm::CHANNEL, rm::EFFECT) => Self::SetEffect(Effect(a[0])),
            (CUSTOM_GET_VALUE, rm::CHANNEL, rm::EFFECT) => Self::GetEffect,
            (CUSTOM_SET_VALUE, rm::CHANNEL, rm::EFFECT_SPEED) => Self::SetEffectSpeed(a[0]),
            (CUSTOM_GET_VALUE, rm::CHANNEL, rm::EFFECT_SPEED) => Self::GetEffectSpeed,
            (CUSTOM_SET_VALUE, rm::CHANNEL, rm::COLOR) => Self::SetColor {
                hue: a[0],
                sat: a[1],
            },
            (CUSTOM_GET_VALUE, rm::CHANNEL, rm::COLOR) => Self::GetColor,
            (CUSTOM_GET_VALUE, perkey::CHANNEL, pk::PROBE) => Self::ProbePerKey,
            (CUSTOM_SET_VALUE, perkey::CHANNEL, pk::SET_KEY) => Self::SetKey(
                key(a[0])?,
                Rgb {
                    r: a[1],
                    g: a[2],
                    b: a[3],
                },
            ),
            (CUSTOM_SET_VALUE, perkey::CHANNEL, pk::SET_MANY) => Self::SetMany(KeyBatch::parse(a)?),
            (CUSTOM_SET_VALUE, perkey::CHANNEL, pk::CLEAR_KEY) => Self::ClearKey(key(a[0])?),
            (CUSTOM_SET_VALUE, perkey::CHANNEL, pk::CLEAR_ALL) => Self::ClearAllKeys,
            (CUSTOM_SET_VALUE, perkey::CHANNEL, pk::SET_EXCLUSIVE) => match a[0] {
                0 => Self::SetExclusive(false),
                1 => Self::SetExclusive(true),
                _ => return None,
            },
            _ => return None,
        })
    }

    /// Encode the reply firmware would send for `self`. Used by fake devices
    /// and tests so they share the one byte layout with [`Command::decode`].
    /// Set commands echo the request; gets fill in the payload.
    #[must_use]
    pub fn reply(self, response: Response) -> Report {
        let mut report = self.encode();
        let (_, len) = self.header();
        let p = &mut report.0[len..];
        match response {
            Response::ProtocolVersion(ProtocolVersion(v)) => {
                p[..2].copy_from_slice(&v.to_be_bytes());
            }
            Response::Brightness(v) | Response::EffectSpeed(v) | Response::Effect(Effect(v)) => {
                p[0] = v;
            }
            Response::Color { hue, sat } => p[..2].copy_from_slice(&[hue, sat]),
            Response::PerKey(info) => {
                p[..4].copy_from_slice(&[MAGIC[0], MAGIC[1], info.version, info.key_count]);
            }
            Response::Ack => {}
        }
        report
    }
}

/// The reply firmware sends for a command it does not handle: the request
/// with byte 0 replaced by `0xFF`.
#[must_use]
pub fn unhandled(request: &Report) -> Report {
    let mut reply = *request;
    reply.0[0] = id::UNHANDLED;
    reply
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Expected report: the given prefix, zero padded to 32 bytes.
    fn bytes(prefix: &[u8]) -> [u8; 32] {
        let mut out = [0u8; 32];
        out[..prefix.len()].copy_from_slice(prefix);
        out
    }

    fn k(i: u8) -> KeyId {
        KeyId::new(i).unwrap()
    }

    #[test]
    fn get_protocol_version_matches_via_bytes() {
        assert_eq!(Command::GetProtocolVersion.encode().0, bytes(&[0x01]));
    }

    #[test]
    fn protocol_version_round_trips() {
        let reply =
            Command::GetProtocolVersion.reply(Response::ProtocolVersion(ProtocolVersion(0x000C)));
        assert_eq!(&reply.0[..3], &[0x01, 0x00, 0x0C]);
        assert_eq!(
            Command::GetProtocolVersion.decode(&reply),
            Ok(Response::ProtocolVersion(ProtocolVersion(12)))
        );
    }

    /// Golden bytes from `FrameworkComputer/qmk_hid` `src/via.rs`
    /// (`set_rgb_u8`, `get_rgb_u8`, `set_rgb_color`, `get_rgb_color`,
    /// `save_rgb`): `[command, channel 3, value id, data..]`.
    #[test]
    fn stock_lighting_matches_qmk_hid() {
        let cases = [
            (
                Command::SetBrightness(0x80),
                bytes(&[0x07, 0x03, 0x01, 0x80]),
            ),
            (Command::GetBrightness, bytes(&[0x08, 0x03, 0x01])),
            (
                Command::SetEffect(Effect::SOLID_COLOR),
                bytes(&[0x07, 0x03, 0x02, 0x01]),
            ),
            (
                Command::SetEffect(Effect::OFF),
                bytes(&[0x07, 0x03, 0x02, 0x00]),
            ),
            (Command::GetEffect, bytes(&[0x08, 0x03, 0x02])),
            (
                Command::SetEffectSpeed(0x40),
                bytes(&[0x07, 0x03, 0x03, 0x40]),
            ),
            (Command::GetEffectSpeed, bytes(&[0x08, 0x03, 0x03])),
            (
                Command::SetColor { hue: 85, sat: 255 },
                bytes(&[0x07, 0x03, 0x04, 85, 255]),
            ),
            (Command::GetColor, bytes(&[0x08, 0x03, 0x04])),
            (Command::SaveLighting, bytes(&[0x09, 0x03])),
        ];
        for (command, expected) in cases {
            assert_eq!(command.encode().0, expected, "{command:?}");
        }
    }

    #[test]
    fn stock_get_replies_read_payload_after_header() {
        // qmk_hid reads output[2] (after dropping byte 0), i.e. byte 3.
        let reply = Report(bytes(&[0x08, 0x03, 0x01, 0xC8]));
        assert_eq!(
            Command::GetBrightness.decode(&reply),
            Ok(Response::Brightness(200))
        );
        let reply = Report(bytes(&[0x08, 0x03, 0x04, 10, 20]));
        assert_eq!(
            Command::GetColor.decode(&reply),
            Ok(Response::Color { hue: 10, sat: 20 })
        );
    }

    #[test]
    fn per_key_requests_use_channel_zero() {
        let red = Rgb {
            r: 0xFF,
            g: 0x10,
            b: 0x00,
        };
        let cases = [
            (Command::ProbePerKey, bytes(&[0x08, 0x00, 0x01])),
            (
                Command::SetKey(k(5), red),
                bytes(&[0x07, 0x00, 0x02, 5, 0xFF, 0x10, 0x00]),
            ),
            (Command::ClearAllKeys, bytes(&[0x07, 0x00, 0x04])),
            (Command::ClearKey(k(23)), bytes(&[0x07, 0x00, 0x05, 23])),
            (Command::SetExclusive(true), bytes(&[0x07, 0x00, 0x06, 1])),
            (Command::SetExclusive(false), bytes(&[0x07, 0x00, 0x06, 0])),
        ];
        for (command, expected) in cases {
            assert_eq!(command.encode().0, expected, "{command:?}");
        }
    }

    #[test]
    fn set_many_packs_seven_entries_into_one_report() {
        let c = |i: u8| Rgb {
            r: i,
            g: i + 1,
            b: i + 2,
        };
        let keys: Vec<_> = (0..7).map(|i| (k(i), c(i * 10))).collect();
        let batch = KeyBatch::chunks(&keys).next().unwrap();
        let mut expected = vec![0x07, 0x00, 0x03, 7];
        for i in 0..7u8 {
            expected.extend([i, i * 10, i * 10 + 1, i * 10 + 2]);
        }
        assert_eq!(expected.len(), 32);
        assert_eq!(Command::SetMany(batch).encode().0.to_vec(), expected);
    }

    #[test]
    fn probe_round_trips_and_rejects_foreign_firmware() {
        let info = PerKeyInfo {
            version: 1,
            key_count: 24,
        };
        let reply = Command::ProbePerKey.reply(Response::PerKey(info));
        assert_eq!(&reply.0[..7], &[0x08, 0x00, 0x01, b'p', b'd', 1, 24]);
        assert_eq!(
            Command::ProbePerKey.decode(&reply),
            Ok(Response::PerKey(info))
        );
        let other = Report(bytes(&[0x08, 0x00, 0x01, 0, 0, 1, 24]));
        assert_eq!(
            Command::ProbePerKey.decode(&other),
            Err(ProtoError::ForeignFirmware)
        );
    }

    #[test]
    fn stock_firmware_rejects_channel_zero() {
        let reply = unhandled(&Command::ProbePerKey.encode());
        assert_eq!(
            Command::ProbePerKey.decode(&reply),
            Err(ProtoError::Unhandled(Command::ProbePerKey))
        );
    }

    #[test]
    fn rejects_mismatched_echo() {
        let reply = Report(bytes(&[0x07, 0x03, 0x02]));
        assert!(matches!(
            Command::SetBrightness(1).decode(&reply),
            Err(ProtoError::WrongEcho {
                got: [0x07, 0x03, 0x02],
                ..
            })
        ));
        assert!(matches!(
            Command::GetProtocolVersion.decode(&Report(bytes(&[0x02]))),
            Err(ProtoError::WrongEcho {
                got: [0x02, 0, 0],
                ..
            })
        ));
    }

    #[test]
    fn parse_inverts_encode() {
        let red = Rgb { r: 255, g: 0, b: 0 };
        let keys: Vec<_> = KeyId::all().take(9).map(|key| (key, red)).collect();
        for batch in KeyBatch::chunks(&keys) {
            let c = Command::SetMany(batch);
            assert_eq!(Command::parse(&c.encode()), Some(c));
        }
        for c in [
            Command::SetColor { hue: 1, sat: 2 },
            Command::SetKey(k(7), red),
            Command::ClearKey(k(3)),
            Command::SaveLighting,
        ] {
            assert_eq!(Command::parse(&c.encode()), Some(c));
        }
        assert_eq!(Command::parse(&Report(bytes(&[0x0B]))), None);
        assert_eq!(
            Command::parse(&Report(bytes(&[0x07, 0x00, 0x02, 24]))),
            None
        );
    }

    /// `0x0B` is Framework's factory command. Only `BootloaderJump` (a
    /// separate type, see `factory.rs`) may encode it; no `Command` can.
    #[test]
    fn only_bootloader_jump_encodes_0x0b() {
        assert_eq!(crate::BootloaderJump.encode().0[0], 0x0B);
        let key = k(0);
        let rgb = Rgb { r: 0, g: 0, b: 0 };
        let batch = KeyBatch::chunks(&[(key, rgb)]).next().unwrap();
        let all = [
            Command::GetProtocolVersion,
            Command::SetBrightness(0x0B),
            Command::GetBrightness,
            Command::SetEffect(Effect(0x0B)),
            Command::GetEffect,
            Command::SetEffectSpeed(0),
            Command::GetEffectSpeed,
            Command::SetColor { hue: 0, sat: 0 },
            Command::GetColor,
            Command::SaveLighting,
            Command::ProbePerKey,
            Command::SetKey(key, rgb),
            Command::SetMany(batch),
            Command::ClearKey(key),
            Command::ClearAllKeys,
            Command::SetExclusive(true),
        ];
        for command in all {
            assert_ne!(command.encode().0[0], 0x0B, "{command:?}");
        }
    }
}
