//! Global RGB matrix control over stock VIA (channel 3). Works on unmodified
//! Framework firmware and affects every key at once.

use macropad_proto::{Command, Effect, Hsv, Response, Rgb};

use crate::{HidError, Macropad, Transport, unexpected};

/// Snapshot of the global lighting state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Lighting {
    /// Current effect.
    pub effect: Effect,
    /// Brightness, `0..=255` before the firmware's maximum is applied.
    pub brightness: u8,
    /// Effect speed.
    pub speed: u8,
    /// Hue and saturation; together with `brightness` this is the HSV colour.
    pub hue: u8,
    /// Saturation.
    pub sat: u8,
}

impl<T: Transport> Macropad<T> {
    fn ack(&mut self, command: Command) -> Result<(), HidError> {
        match self.request(command)? {
            Response::Ack => Ok(()),
            other => Err(unexpected(other)),
        }
    }

    fn get_u8(&mut self, command: Command) -> Result<u8, HidError> {
        match self.request(command)? {
            Response::Brightness(v) | Response::EffectSpeed(v) | Response::Effect(Effect(v)) => {
                Ok(v)
            }
            other => Err(unexpected(other)),
        }
    }

    /// Read the global lighting state (four requests).
    ///
    /// # Errors
    /// [`HidError`] on transport or protocol failure.
    pub fn lighting(&mut self) -> Result<Lighting, HidError> {
        let effect = Effect(self.get_u8(Command::GetEffect)?);
        let brightness = self.get_u8(Command::GetBrightness)?;
        let speed = self.get_u8(Command::GetEffectSpeed)?;
        let (hue, sat) = match self.request(Command::GetColor)? {
            Response::Color { hue, sat } => (hue, sat),
            other => return Err(unexpected(other)),
        };
        Ok(Lighting {
            effect,
            brightness,
            speed,
            hue,
            sat,
        })
    }

    /// Set global brightness.
    ///
    /// # Errors
    /// [`HidError`] on transport or protocol failure.
    pub fn set_brightness(&mut self, brightness: u8) -> Result<(), HidError> {
        self.ack(Command::SetBrightness(brightness))
    }

    /// Select an effect; [`Effect::OFF`] turns the matrix off.
    ///
    /// # Errors
    /// [`HidError`] on transport or protocol failure.
    pub fn set_effect(&mut self, effect: Effect) -> Result<(), HidError> {
        self.ack(Command::SetEffect(effect))
    }

    /// Set effect speed.
    ///
    /// # Errors
    /// [`HidError`] on transport or protocol failure.
    pub fn set_speed(&mut self, speed: u8) -> Result<(), HidError> {
        self.ack(Command::SetEffectSpeed(speed))
    }

    /// Set global hue and saturation, leaving brightness alone.
    ///
    /// # Errors
    /// [`HidError`] on transport or protocol failure.
    pub fn set_hue_sat(&mut self, hue: u8, sat: u8) -> Result<(), HidError> {
        self.ack(Command::SetColor { hue, sat })
    }

    /// Light every key in one colour: solid effect, hue and saturation from
    /// `rgb`, brightness from its value (ADR 0002). Black turns the matrix off,
    /// because VIA brightness 0 alone would leave the effect running dark.
    ///
    /// # Errors
    /// [`HidError`] on transport or protocol failure.
    pub fn set_solid(&mut self, rgb: Rgb) -> Result<(), HidError> {
        let Hsv { h, s, v } = rgb.into();
        if v == 0 {
            return self.set_effect(Effect::OFF);
        }
        self.set_effect(Effect::SOLID_COLOR)?;
        self.set_hue_sat(h, s)?;
        self.set_brightness(v)
    }

    /// Persist the current global lighting to EEPROM. Everything else in this
    /// API is volatile; call this sparingly, flash wears.
    ///
    /// # Errors
    /// [`HidError`] on transport or protocol failure.
    pub fn save_lighting(&mut self) -> Result<(), HidError> {
        self.ack(Command::SaveLighting)
    }
}
