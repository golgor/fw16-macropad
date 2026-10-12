//! Real [`LedSink`]: per-key overrides through `macropad-hid`.

use macropad_hid::{Backend, Macropad};
use macropad_proto::{Effect, KeyId, PER_KEY_PROTOCOL_VERSION, Rgb};

use crate::{Frame, LedSink};

/// Paints frames on the first attached macropad, reopening it after it
/// disappears. Requires padctl firmware v3 (checked by the per-key probe).
pub struct HidLeds<B: Backend> {
    backend: B,
    pad: Option<Macropad<B::Transport>>,
    /// What the device shows now, for diffing; `None` after (re)open.
    shown: Option<Frame>,
}

impl<B: Backend> HidLeds<B> {
    /// Nothing is opened until the first [`LedSink::show`].
    pub fn new(backend: B) -> Self {
        Self {
            backend,
            pad: None,
            shown: None,
        }
    }

    fn open(&mut self) -> Result<(), String> {
        let mut pad = Macropad::new(self.backend.open_first().map_err(|e| e.to_string())?);
        if pad.per_key().map_err(|e| e.to_string())?.info().version != PER_KEY_PROTOCOL_VERSION {
            return Err("padctl firmware v3 required".into());
        }
        self.pad = Some(pad);
        self.shown = None;
        Ok(())
    }

    /// Re-assert everything padd relies on, without trusting the device's
    /// current state: exclusive mode, no stale overrides, global effect off
    /// (dark with no claims; overrides switch the matrix back on, firmware
    /// ADR 0003), then every key of `frame`.
    fn full_paint(pad: &mut Macropad<B::Transport>, frame: &Frame) -> Result<(), String> {
        {
            let mut leds = pad.per_key().map_err(|e| e.to_string())?;
            leds.set_exclusive(true).map_err(|e| e.to_string())?;
            leds.clear_all().map_err(|e| e.to_string())?;
        }
        pad.set_effect(Effect::OFF).map_err(|e| e.to_string())?;
        let all: Vec<(KeyId, Rgb)> = KeyId::all()
            .filter_map(|k| frame.get(k).map(|c| (k, c)))
            .collect();
        if !all.is_empty() {
            pad.per_key()
                .map_err(|e| e.to_string())?
                .set_many(&all)
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    fn paint(&mut self, frame: &Frame) -> Result<(), String> {
        let Some(pad) = self.pad.as_mut() else {
            return Err("not open".into());
        };
        let Some(shown) = self.shown else {
            Self::full_paint(pad, frame)?;
            self.shown = Some(*frame);
            return Ok(());
        };
        let mut leds = pad.per_key().map_err(|e| e.to_string())?;
        if frame.is_dark() {
            if !shown.is_dark() {
                leds.clear_all().map_err(|e| e.to_string())?;
            }
        } else {
            let changed: Vec<(KeyId, Rgb)> = KeyId::all()
                .filter_map(|k| {
                    frame
                        .get(k)
                        .filter(|c| shown.get(k) != Some(*c))
                        .map(|c| (k, c))
                })
                .collect();
            leds.set_many(&changed).map_err(|e| e.to_string())?;
            for k in KeyId::all().filter(|&k| frame.get(k).is_none() && shown.get(k).is_some()) {
                leds.clear(k).map_err(|e| e.to_string())?;
            }
        }
        self.shown = Some(*frame);
        Ok(())
    }
}

impl<B: Backend + Send> LedSink for HidLeds<B>
where
    B::Transport: Send,
{
    fn show(&mut self, frame: &Frame) -> Result<(), String> {
        if self.pad.is_none() {
            self.open()?;
        }
        self.paint(frame).inspect_err(|_| {
            self.pad = None;
            self.shown = None;
        })
    }

    fn invalidate(&mut self) {
        self.shown = None;
    }
}

#[cfg(test)]
mod tests {
    use macropad_hid::fake::{FakeBackend, FakeDevice};

    use super::*;

    const RED: Rgb = Rgb { r: 255, g: 0, b: 0 };

    #[test]
    fn stock_firmware_is_refused() {
        let mut leds = HidLeds::new(FakeBackend::with(vec![FakeDevice::stock("f")]));
        let err = leds.show(&Frame::default()).unwrap_err();
        assert!(err.contains("padctl firmware"), "{err}");
    }

    #[test]
    fn paints_and_clears_with_diffs() {
        let mut leds = HidLeds::new(FakeBackend::with(vec![FakeDevice::padctl("f")]));
        let mut frame = Frame::default();
        frame.0[0] = Some(RED);
        frame.0[1] = Some(RED);
        leds.show(&frame).unwrap();
        frame.0[1] = None;
        leds.show(&frame).unwrap();
        let dev = leds.pad.as_ref().unwrap().transport();
        assert_eq!(dev.override_of(KeyId::new(0).unwrap()), Some(RED));
        assert_eq!(dev.override_of(KeyId::new(1).unwrap()), None);
        assert_eq!(
            dev.effect,
            Effect::SOLID_COLOR,
            "matrix on while overrides exist"
        );
        leds.show(&Frame::default()).unwrap();
        assert_eq!(leds.pad.as_ref().unwrap().transport().effect, Effect::OFF);
    }

    /// Device state changed behind padd's back (another writer turned the
    /// matrix off and dropped overrides). After `invalidate` the next show
    /// re-asserts everything instead of diffing against the stale cache.
    #[test]
    fn invalidate_forces_a_full_repaint() {
        let mut leds = HidLeds::new(FakeBackend::with(vec![FakeDevice::padctl("f")]));
        let mut frame = Frame::default();
        frame.0[3] = Some(RED);
        leds.show(&frame).unwrap();
        {
            let pad = leds.pad.as_mut().unwrap();
            pad.set_effect(Effect::OFF).unwrap();
            let mut per_key = pad.per_key().unwrap();
            per_key.set_exclusive(false).unwrap();
            per_key.clear_all().unwrap();
        }
        leds.show(&frame).unwrap();
        let k3 = KeyId::new(3).unwrap();
        assert_eq!(
            leds.pad.as_ref().unwrap().transport().override_of(k3),
            None,
            "diff alone sends nothing"
        );
        leds.invalidate();
        leds.show(&frame).unwrap();
        let dev = leds.pad.as_ref().unwrap().transport();
        assert_eq!(dev.shown(k3), Some(RED));
        assert!(dev.exclusive);
        assert_eq!(
            dev.shown(KeyId::new(4).unwrap()),
            Some(Rgb { r: 0, g: 0, b: 0 })
        );
    }

    #[test]
    fn missing_device_is_an_error_not_a_panic() {
        assert!(
            HidLeds::new(FakeBackend::default())
                .show(&Frame::default())
                .is_err()
        );
    }
}
