//! Per-key colour over our channel-0 sub-protocol.
//!
//! [`PerKeyLeds`] is a capability: the only way to get one is
//! [`Macropad::per_key`], which probes the firmware first. Code holding a
//! `PerKeyLeds` therefore knows the firmware can do per-key colour, and code
//! talking to stock firmware cannot call these methods at all.

use macropad_proto::{
    Command, KeyBatch, KeyId, PER_KEY_PROTOCOL_VERSION, PerKeyInfo, ProtoError, Response, Rgb,
};

use crate::{HidError, Macropad, Transport, unexpected};

/// Per-key LED control, borrowed from a probed [`Macropad`].
pub struct PerKeyLeds<'a, T> {
    pad: &'a mut Macropad<T>,
    info: PerKeyInfo,
}

impl<T: Transport> Macropad<T> {
    /// Probe for the padctl firmware and hand out per-key control.
    ///
    /// # Errors
    /// [`HidError::NoPerKey`] on stock or foreign firmware, or an unknown
    /// sub-protocol version; other [`HidError`]s on transport failure.
    pub fn per_key(&mut self) -> Result<PerKeyLeds<'_, T>, HidError> {
        let info = match self.request(Command::ProbePerKey) {
            Ok(Response::PerKey(info)) => info,
            Ok(other) => return Err(unexpected(other)),
            Err(HidError::Protocol(ProtoError::Unhandled(_))) => {
                return Err(HidError::NoPerKey(
                    "firmware does not answer channel 0".into(),
                ));
            }
            Err(HidError::Protocol(e @ ProtoError::ForeignFirmware)) => {
                return Err(HidError::NoPerKey(e.to_string()));
            }
            Err(e) => return Err(e),
        };
        if info.version != PER_KEY_PROTOCOL_VERSION {
            return Err(HidError::NoPerKey(format!(
                "firmware speaks per-key v{}, padctl speaks v{PER_KEY_PROTOCOL_VERSION}",
                info.version
            )));
        }
        Ok(PerKeyLeds { pad: self, info })
    }
}

impl<T: Transport> PerKeyLeds<'_, T> {
    /// What the firmware reported when probed.
    #[must_use]
    pub fn info(&self) -> PerKeyInfo {
        self.info
    }

    fn ack(&mut self, command: Command) -> Result<(), HidError> {
        match self.pad.request(command)? {
            Response::Ack => Ok(()),
            other => Err(unexpected(other)),
        }
    }

    /// Override one key's colour. The rest of the pad keeps its effect.
    ///
    /// # Errors
    /// [`HidError`] on transport or protocol failure.
    pub fn set(&mut self, key: KeyId, rgb: Rgb) -> Result<(), HidError> {
        self.ack(Command::SetKey(key, rgb))
    }

    /// Override many keys, seven per report. Not atomic across reports: a
    /// failure part way leaves earlier batches applied.
    ///
    /// # Errors
    /// [`HidError`] on transport or protocol failure.
    pub fn set_many(&mut self, colours: &[(KeyId, Rgb)]) -> Result<(), HidError> {
        for batch in KeyBatch::chunks(colours) {
            self.ack(Command::SetMany(batch))?;
        }
        Ok(())
    }

    /// Drop one key's override.
    ///
    /// # Errors
    /// [`HidError`] on transport or protocol failure.
    pub fn clear(&mut self, key: KeyId) -> Result<(), HidError> {
        self.ack(Command::ClearKey(key))
    }

    /// Drop every override.
    ///
    /// # Errors
    /// [`HidError`] on transport or protocol failure.
    pub fn clear_all(&mut self) -> Result<(), HidError> {
        self.ack(Command::ClearAllKeys)
    }
}
