//! In-memory fake device, so layers above can be tested without hardware.
//!
//! It answers through [`macropad_proto`]'s own encoders, so a fake reply has
//! exactly the bytes real firmware sends for the commands we support.

use macropad_proto::{Command, ProtocolVersion, Report, Response};

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
        }
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
        if request.as_bytes()[0] == Command::GetProtocolVersion.encode().as_bytes()[0] {
            Ok(Response::ProtocolVersion(self.protocol_version).encode())
        } else {
            let mut unhandled = *request;
            unhandled.0[0] = 0xFF;
            Ok(unhandled)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Macropad;

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
}
