//! Parsing of the 300-byte `FIND_DEVICE` answer.

use std::fmt;

/// Field order confirmed by the Caddx `ResDeviceInfo` layout and by captured frames.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeviceInfo {
    /// `receiveMaxSize`, 1 048 576 on current firmware.
    pub max_receive_size: u32,
    pub sdk_version: String,
    pub device_name: String,
    pub cpu_temp: i32,
    pub firmware: String,
    pub serial: String,
    pub hardware: String,
    pub status: i32,
    pub detail: String,
}

pub const PAYLOAD_LEN: usize = 300;

/// Transmitter (GT Pro, GT Max, Lite: firmware `Ascent_G_Sky_…`) or receiver (VRX family).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Transmitter,
    Receiver,
}

#[derive(Debug, PartialEq, Eq)]
pub enum DeviceInfoError {
    TooShort { got: usize },
}

impl fmt::Display for DeviceInfoError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooShort { got } => write!(
                formatter,
                "ответ об устройстве короче {PAYLOAD_LEN} байт: получено {got}"
            ),
        }
    }
}

impl std::error::Error for DeviceInfoError {}

fn field(payload: &[u8], offset: usize, len: usize) -> String {
    let slice = &payload[offset..offset + len];
    let end = slice.iter().position(|&b| b == 0).unwrap_or(slice.len());
    String::from_utf8_lossy(&slice[..end]).trim().to_string()
}

fn u32_at(payload: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        payload[offset],
        payload[offset + 1],
        payload[offset + 2],
        payload[offset + 3],
    ])
}

impl DeviceInfo {
    pub fn parse(payload: &[u8]) -> Result<Self, DeviceInfoError> {
        if payload.len() < PAYLOAD_LEN {
            return Err(DeviceInfoError::TooShort { got: payload.len() });
        }
        Ok(Self {
            max_receive_size: u32_at(payload, 0),
            sdk_version: field(payload, 4, 32),
            device_name: field(payload, 36, 64),
            cpu_temp: u32_at(payload, 100) as i32,
            firmware: field(payload, 104, 64),
            serial: field(payload, 168, 32),
            hardware: field(payload, 200, 32),
            status: u32_at(payload, 232) as i32,
            detail: field(payload, 236, 64),
        })
    }

    /// Short human-readable summary for menus and logs.
    pub fn summary(&self) -> String {
        format!(
            "{} | прошивка {} | железо {} | SN {} | {} °C",
            self.device_name, self.firmware, self.hardware, self.serial, self.cpu_temp
        )
    }

    /// Which side of the link the device is. The firmware tag (`Ascent_G_Sky_…` on GT/Lite
    /// transmitters, `…_Gnd_…` on VRX receivers) decides; the model name is a cross-check.
    /// Returns `None` when the two disagree or neither is recognised.
    pub fn side(&self) -> Option<Side> {
        let firmware = self.firmware.to_ascii_lowercase();
        let by_firmware = if firmware.contains("_sky_") {
            Some(Side::Transmitter)
        } else if firmware.contains("_gnd_") {
            Some(Side::Receiver)
        } else {
            None
        };
        let name = self.device_name.to_ascii_lowercase();
        let by_name = if name.contains("vrx") {
            Some(Side::Receiver)
        } else if name.contains("_gt") || name.contains("lite") {
            Some(Side::Transmitter)
        } else {
            None
        };
        match (by_firmware, by_name) {
            (Some(a), Some(b)) if a != b => None,
            (Some(side), _) | (None, Some(side)) => Some(side),
            (None, None) => None,
        }
    }

    /// The trailing `NN_NN_NN` of the firmware name, which a firmware file must match.
    pub fn firmware_version(&self) -> Option<&str> {
        let parts: Vec<&str> = self.firmware.rsplit('_').take(4).collect();
        if parts.len() < 3
            || !parts[..3]
                .iter()
                .all(|p| p.chars().all(|c| c.is_ascii_digit()) && !p.is_empty())
        {
            return None;
        }
        let tail_len = parts[0].len() + parts[1].len() + parts[2].len() + 2;
        self.firmware
            .len()
            .checked_sub(tail_len)
            .map(|start| &self.firmware[start..])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Exact 300-byte payload captured from an Ascent GT running 16_5_9.
    const PAYLOAD: &str = concat!(
        "0000100000000000000000000000000000000000000000000000000000000000",
        "00000000417363656e745f47545f70726f000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "000000002a000000417363656e745f475f536b795f31365f355f390000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000363230390000000000000000000000000000000000000000",
        "00000000000000004650562d417363656e742d536b792d3438362d56312e302d",
        "312e300000000000000000004f4b000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "000000000000000000000000"
    );

    fn from_hex(text: &str) -> Vec<u8> {
        let clean: String = text.chars().filter(|c| c.is_ascii_hexdigit()).collect();
        clean
            .as_bytes()
            .chunks(2)
            .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
            .collect()
    }

    #[test]
    fn parses_a_captured_device_info() {
        let payload = from_hex(PAYLOAD);
        assert_eq!(
            payload.len(),
            PAYLOAD_LEN,
            "смещения рассчитаны на 300 байт"
        );
        let info = DeviceInfo::parse(&payload).unwrap();
        assert_eq!(info.device_name, "Ascent_GT_pro");
        assert_eq!(info.firmware, "Ascent_G_Sky_16_5_9");
        assert_eq!(info.hardware, "FPV-Ascent-Sky-486-V1.0-1.0");
        assert_eq!(info.serial, "6209");
        assert_eq!(info.cpu_temp, 42);
        assert_eq!(info.max_receive_size, 1_048_576);
        assert_eq!(info.status, 0);
        assert_eq!(info.detail, "OK");
        assert!(info.sdk_version.is_empty());
        assert!(info.summary().contains("Ascent_G_Sky_16_5_9"));
    }

    #[test]
    fn extracts_the_numeric_firmware_version() {
        let mut info = DeviceInfo::parse(&from_hex(PAYLOAD)).unwrap();
        assert_eq!(info.firmware_version(), Some("16_5_9"));
        info.firmware = "Ascent_G_Sky_18_21_10".into();
        assert_eq!(info.firmware_version(), Some("18_21_10"));
        info.firmware = "Ascent_G_Sky".into();
        assert_eq!(info.firmware_version(), None);
        info.firmware = "Ascent_G_Sky_beta_1_2".into();
        assert_eq!(info.firmware_version(), None, "нечисловая часть не версия");
    }

    #[test]
    fn decides_the_side_from_firmware_and_model() {
        let mut info = DeviceInfo::parse(&from_hex(PAYLOAD)).unwrap();
        assert_eq!(
            info.side(),
            Some(Side::Transmitter),
            "GT Pro с прошивкой Sky"
        );

        info.device_name = "Ascent_VRX_Pro".into();
        info.firmware = "Ascent_G_Gnd_19_2_14".into();
        assert_eq!(info.side(), Some(Side::Receiver));

        info.firmware = "Ascent_G_Sky_19_2_14".into();
        assert_eq!(
            info.side(),
            None,
            "модель VRX с прошивкой Sky — противоречие"
        );

        info.device_name = "new device".into();
        info.firmware = "unknown".into();
        assert_eq!(info.side(), None);
    }

    #[test]
    fn rejects_a_short_payload() {
        let payload = from_hex(PAYLOAD);
        assert_eq!(
            DeviceInfo::parse(&payload[..299]).unwrap_err(),
            DeviceInfoError::TooShort { got: 299 }
        );
    }
}
