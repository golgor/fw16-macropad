//! In-memory fake device, so layers above can be tested without hardware.
//!
//! It parses requests and answers through [`macropad_proto`]'s own codecs, so
//! a fake reply has exactly the bytes real firmware sends. It can emulate stock
//! firmware or the padctl keymap (per-key channel 0).

use macropad_proto::{
    Command, Effect, KEY_COUNT, KeyId, PER_KEY_PROTOCOL_VERSION, PerKeyInfo, ProtocolVersion,
    Report, Response, Rgb, unhandled,
};

use crate::{Backend, DeviceInfo, DevicePath, HidError, Transport};

/// A fake backend exposing zero or more fake macropads.
#[derive(Debug, Clone, Default)]
pub struct FakeBackend {
    devices: Vec<FakeDevice>,
}

/// State of one fake macropad.
#[derive(Debug, Clone)]
pub struct FakeDevice {
    /// Reported in discovery.
    pub info: DeviceInfo,
    /// What `GetProtocolVersion` answers.
    pub protocol_version: ProtocolVersion,
    /// Global RGB matrix state.
    pub effect: Effect,
    /// Global brightness.
    pub brightness: u8,
    /// Effect speed.
    pub speed: u8,
    /// Global hue.
    pub hue: u8,
    /// Global saturation.
    pub sat: u8,
    /// How often lighting was saved to EEPROM.
    pub saves: u32,
    /// `Some` when emulating the padctl firmware: its per-key overrides,
    /// indexed by [`KeyId::index`].
    pub overrides: Option<[Option<Rgb>; KEY_COUNT as usize]>,
    /// Every request seen, for byte-level assertions.
    pub requests: Vec<Report>,
}

impl FakeDevice {
    /// A device that looks like stock Framework firmware (`fl16-2025`, VIA 12).
    #[must_use]
    pub fn stock(path: &str) -> Self {
        Self {
            info: DeviceInfo {
                path: DevicePath(path.to_owned()),
                product: Some("Laptop 16 RGB Macropad".to_owned()),
                serial: Some("FAKE0001".to_owned()),
            },
            protocol_version: ProtocolVersion(12),
            effect: Effect::SOLID_COLOR,
            brightness: 255,
            speed: 127,
            hue: 0,
            sat: 0,
            saves: 0,
            overrides: None,
            requests: Vec::new(),
        }
    }

    /// A device running the padctl keymap: stock plus per-key channel 0.
    #[must_use]
    pub fn padctl(path: &str) -> Self {
        Self {
            overrides: Some([None; KEY_COUNT as usize]),
            ..Self::stock(path)
        }
    }

    /// The override on `key`, if the fake emulates padctl firmware.
    #[must_use]
    pub fn override_of(&self, key: KeyId) -> Option<Rgb> {
        self.overrides?[usize::from(key.index())]
    }

    fn handle(&mut self, command: Command) -> Option<Response> {
        use Command as C;
        Some(match command {
            C::GetProtocolVersion => Response::ProtocolVersion(self.protocol_version),
            C::SetBrightness(v) => {
                self.brightness = v;
                Response::Ack
            }
            C::GetBrightness => Response::Brightness(self.brightness),
            C::SetEffect(e) => {
                self.effect = e;
                Response::Ack
            }
            C::GetEffect => Response::Effect(self.effect),
            C::SetEffectSpeed(v) => {
                self.speed = v;
                Response::Ack
            }
            C::GetEffectSpeed => Response::EffectSpeed(self.speed),
            C::SetColor { hue, sat } => {
                (self.hue, self.sat) = (hue, sat);
                Response::Ack
            }
            C::GetColor => Response::Color {
                hue: self.hue,
                sat: self.sat,
            },
            C::SaveLighting => {
                self.saves += 1;
                Response::Ack
            }
            C::ProbePerKey => {
                self.overrides?;
                Response::PerKey(PerKeyInfo {
                    version: PER_KEY_PROTOCOL_VERSION,
                    key_count: KEY_COUNT,
                })
            }
            C::SetKey(key, rgb) => {
                self.overrides.as_mut()?[usize::from(key.index())] = Some(rgb);
                Response::Ack
            }
            C::SetMany(batch) => {
                let overrides = self.overrides.as_mut()?;
                for &(key, rgb) in batch.entries() {
                    overrides[usize::from(key.index())] = Some(rgb);
                }
                Response::Ack
            }
            C::ClearKey(key) => {
                self.overrides.as_mut()?[usize::from(key.index())] = None;
                Response::Ack
            }
            C::ClearAllKeys => {
                *self.overrides.as_mut()? = [None; KEY_COUNT as usize];
                Response::Ack
            }
        })
    }
}

impl FakeBackend {
    /// A backend with the given devices attached.
    #[must_use]
    pub fn with(devices: Vec<FakeDevice>) -> Self {
        Self { devices }
    }
}

impl Backend for FakeBackend {
    type Transport = FakeDevice;

    fn devices(&self) -> Result<Vec<DeviceInfo>, HidError> {
        Ok(self.devices.iter().map(|d| d.info.clone()).collect())
    }

    fn open(&self, path: &DevicePath) -> Result<FakeDevice, HidError> {
        self.devices
            .iter()
            .find(|d| &d.info.path == path)
            .cloned()
            .ok_or(HidError::NotFound)
    }
}

impl Transport for FakeDevice {
    fn exchange(&mut self, request: &Report) -> Result<Report, HidError> {
        self.requests.push(*request);
        let reply = Command::parse(request)
            .and_then(|command| Some(command.reply(self.handle(command)?)))
            .unwrap_or_else(|| unhandled(request));
        Ok(reply)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Macropad;

    fn k(i: u8) -> KeyId {
        KeyId::new(i).unwrap()
    }

    const RED: Rgb = Rgb { r: 255, g: 0, b: 0 };

    #[test]
    fn macropad_reads_version_through_fake() {
        let backend = FakeBackend::with(vec![FakeDevice::stock("fake0")]);
        let mut pad = Macropad::new(backend.open_first().unwrap());
        assert_eq!(pad.protocol_version().unwrap(), ProtocolVersion(12));
    }

    #[test]
    fn empty_backend_reports_not_found() {
        assert!(matches!(
            FakeBackend::default().open_first(),
            Err(HidError::NotFound)
        ));
    }

    #[test]
    fn set_solid_maps_rgb_to_effect_hue_sat_brightness() {
        let mut pad = Macropad::new(FakeDevice::stock("f"));
        pad.set_solid(Rgb { r: 0, g: 128, b: 0 }).unwrap();
        let l = pad.lighting().unwrap();
        assert_eq!(
            (l.effect, l.hue, l.sat, l.brightness),
            (Effect::SOLID_COLOR, 85, 255, 128)
        );
        pad.set_solid(Rgb { r: 0, g: 0, b: 0 }).unwrap();
        assert_eq!(pad.lighting().unwrap().effect, Effect::OFF);
        assert_eq!(pad.into_transport().saves, 0, "set_solid must not persist");
    }

    #[test]
    fn stock_firmware_refuses_per_key() {
        let mut pad = Macropad::new(FakeDevice::stock("f"));
        assert!(matches!(pad.per_key(), Err(HidError::NoPerKey(_))));
        // The pad stays usable for global lighting after a failed probe.
        pad.set_brightness(10).unwrap();
    }

    #[test]
    fn per_key_set_many_and_clear() {
        let mut pad = Macropad::new(FakeDevice::padctl("f"));
        let mut leds = pad.per_key().unwrap();
        assert_eq!(leds.info().key_count, 24);
        let all: Vec<_> = KeyId::all().map(|key| (key, RED)).collect();
        leds.set_many(&all).unwrap();
        leds.clear(k(3)).unwrap();
        leds.set(k(4), Rgb { r: 1, g: 2, b: 3 }).unwrap();
        let dev = pad.into_transport();
        assert_eq!(dev.override_of(k(0)), Some(RED));
        assert_eq!(dev.override_of(k(3)), None);
        assert_eq!(dev.override_of(k(4)), Some(Rgb { r: 1, g: 2, b: 3 }));
        // probe + 4 batches (7+7+7+3) + clear + set
        assert_eq!(dev.requests.len(), 7);

        let mut pad = Macropad::new(dev);
        pad.per_key().unwrap().clear_all().unwrap();
        assert!(KeyId::all().all(|key| pad.transport().override_of(key).is_none()));
    }
}
