use std::ffi::CString;

use hidapi::{HidApi, HidDevice};
use macropad_proto::{
    PRODUCT_ID, RAW_HID_USAGE, RAW_HID_USAGE_PAGE, REPORT_LEN, Report, VENDOR_ID,
};

use crate::{Backend, DeviceInfo, DevicePath, HidError, Transport};

/// How long to wait for a reply before giving up.
const READ_TIMEOUT_MS: i32 = 1000;

/// Real backend on `hidapi` (pure Rust `linux-native-basic-udev`, no C library).
///
/// Filters on VID/PID and the Raw HID usage page, because the macropad also
/// exposes keyboard and console interfaces that must not receive VIA reports.
pub struct HidapiBackend {
    api: HidApi,
}

impl HidapiBackend {
    /// Initialise hidapi.
    ///
    /// # Errors
    /// [`HidError::Io`] when hidapi cannot start.
    pub fn new() -> Result<Self, HidError> {
        HidApi::new().map(|api| Self { api }).map_err(|e| io(&e))
    }
}

fn io(e: &hidapi::HidError) -> HidError {
    HidError::Io(e.to_string())
}

impl Backend for HidapiBackend {
    type Transport = HidapiTransport;

    fn devices(&self) -> Result<Vec<DeviceInfo>, HidError> {
        Ok(self
            .api
            .device_list()
            .filter(|d| {
                d.vendor_id() == VENDOR_ID
                    && d.product_id() == PRODUCT_ID
                    && d.usage_page() == RAW_HID_USAGE_PAGE
                    && d.usage() == RAW_HID_USAGE
            })
            .map(|d| DeviceInfo {
                path: DevicePath(d.path().to_string_lossy().into_owned()),
                product: d.product_string().map(str::to_owned),
                serial: d.serial_number().map(str::to_owned),
            })
            .collect())
    }

    fn open(&self, path: &DevicePath) -> Result<HidapiTransport, HidError> {
        let c_path = CString::new(path.0.as_str()).map_err(|e| HidError::Io(e.to_string()))?;
        let device = self.api.open_path(&c_path).map_err(|e| io(&e))?;
        Ok(HidapiTransport { device })
    }
}

/// An open Raw HID interface.
pub struct HidapiTransport {
    device: HidDevice,
}

impl Transport for HidapiTransport {
    fn send(&mut self, request: &Report) -> Result<(), HidError> {
        // hidapi wants the report id first; QMK Raw HID uses id 0.
        let mut out = [0u8; REPORT_LEN + 1];
        out[1..].copy_from_slice(request.as_bytes());
        self.device.write(&out).map_err(|e| io(&e))?;
        Ok(())
    }

    fn exchange(&mut self, request: &Report) -> Result<Report, HidError> {
        self.send(request)?;

        let mut reply = Report::zeroed();
        match self
            .device
            .read_timeout(&mut reply.0, READ_TIMEOUT_MS)
            .map_err(|e| io(&e))?
        {
            0 => Err(HidError::Timeout),
            _ => Ok(reply),
        }
    }
}
