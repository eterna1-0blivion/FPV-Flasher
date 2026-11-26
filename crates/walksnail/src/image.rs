//! Walksnail firmware container (`*.img`).
//!
//! Header confirmed on all eight sample images: `41 53 57 00` ("ASW\0"), then `u32` type
//! (4 = transmitter / Sky, 5 = receiver / Gnd), then `u32` major, `u32` minor, `u32` patch.

use std::fmt;

pub const MAGIC: [u8; 4] = *b"ASW\0";
/// Signature, type and three version numbers.
pub const HEADER_LEN: usize = 20;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageKind {
    Transmitter,
    Receiver,
    Other(u32),
}

impl ImageKind {
    fn from_type(value: u32) -> Self {
        match value {
            4 => Self::Transmitter,
            5 => Self::Receiver,
            other => Self::Other(other),
        }
    }

    pub const fn code(self) -> u32 {
        match self {
            Self::Transmitter => 4,
            Self::Receiver => 5,
            Self::Other(value) => value,
        }
    }
}

impl fmt::Display for ImageKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Transmitter => write!(formatter, "передатчик (Sky)"),
            Self::Receiver => write!(formatter, "приёмник (Gnd)"),
            Self::Other(value) => write!(formatter, "неизвестный тип {value}"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl fmt::Display for Version {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}_{}_{}", self.major, self.minor, self.patch)
    }
}

impl Version {
    /// Minimum version accepted by Caddx PC Tool; older images are refused before flashing.
    pub const MINIMUM: Self = Self {
        major: 16,
        minor: 4,
        patch: 4,
    };

    pub fn parse(text: &str) -> Option<Self> {
        let mut parts = text.trim().split('_');
        let major = parts.next()?.parse().ok()?;
        let minor = parts.next()?.parse().ok()?;
        let patch = parts.next()?.parse().ok()?;
        if parts.next().is_some() {
            return None;
        }
        Some(Self {
            major,
            minor,
            patch,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImageInfo {
    pub kind: ImageKind,
    pub version: Version,
    pub size: usize,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ImageError {
    TooShort { got: usize },
    BadMagic,
}

impl fmt::Display for ImageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooShort { got } => write!(
                formatter,
                "файл прошивки короче заголовка: {got} из {HEADER_LEN} байт"
            ),
            Self::BadMagic => write!(formatter, "не файл прошивки Walksnail (нет сигнатуры ASW)"),
        }
    }
}

impl std::error::Error for ImageError {}

fn u32_at(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ])
}

impl ImageInfo {
    pub fn parse(bytes: &[u8]) -> Result<Self, ImageError> {
        if bytes.len() < HEADER_LEN {
            return Err(ImageError::TooShort { got: bytes.len() });
        }
        if bytes[..4] != MAGIC {
            return Err(ImageError::BadMagic);
        }
        Ok(Self {
            kind: ImageKind::from_type(u32_at(bytes, 4)),
            version: Version {
                major: u32_at(bytes, 8),
                minor: u32_at(bytes, 12),
                patch: u32_at(bytes, 16),
            },
            size: bytes.len(),
        })
    }

    /// A transmitter image must not be written to a receiver and the other way round.
    pub fn matches(&self, device: &crate::DeviceInfo) -> bool {
        let is_receiver = device.device_name.contains("Gnd");
        match self.kind {
            ImageKind::Receiver => is_receiver,
            ImageKind::Transmitter => !is_receiver,
            ImageKind::Other(_) => false,
        }
    }

    /// Caddx refuses images older than `16_4_4`; we do the same before touching the device.
    pub fn is_flashable(&self, current: Option<Version>) -> Result<(), String> {
        if self.version < Version::MINIMUM {
            return Err(format!(
                "версия {} старше минимально поддерживаемой {}",
                self.version,
                Version::MINIMUM
            ));
        }
        if let Some(current) = current
            && self.version <= current
        {
            return Err(format!(
                "версия {} не новее установленной {current}: понижение не поддерживается",
                self.version
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) fn header(kind: u32, version: &str) -> Vec<u8> {
        let parsed = Version::parse(version).unwrap();
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&MAGIC);
        bytes.extend_from_slice(&kind.to_le_bytes());
        for part in [parsed.major, parsed.minor, parsed.patch] {
            bytes.extend_from_slice(&part.to_le_bytes());
        }
        bytes
    }

    fn device(name: &str) -> crate::DeviceInfo {
        crate::DeviceInfo {
            max_receive_size: 0,
            sdk_version: String::new(),
            device_name: name.into(),
            cpu_temp: 0,
            firmware: "Ascent_G_Sky_16_5_9".into(),
            serial: String::new(),
            hardware: String::new(),
            status: 0,
            detail: String::new(),
        }
    }

    #[test]
    fn parses_all_supported_versions_from_the_real_headers() {
        for version in ["16_5_9", "17_5_8", "18_21_10", "19_2_14"] {
            let sky = ImageInfo::parse(&header(4, version)).unwrap();
            assert_eq!(sky.kind, ImageKind::Transmitter);
            assert_eq!(sky.version, Version::parse(version).unwrap());

            let gnd = ImageInfo::parse(&header(5, version)).unwrap();
            assert_eq!(gnd.kind, ImageKind::Receiver);
            assert_eq!(gnd.version.to_string(), version);
        }
    }

    #[test]
    fn rejects_foreign_and_truncated_files() {
        assert_eq!(
            ImageInfo::parse(b"ASW").unwrap_err(),
            ImageError::TooShort { got: 3 }
        );
        let mut foreign = header(4, "19_2_14");
        foreign[0] = b'X';
        assert_eq!(
            ImageInfo::parse(&foreign).unwrap_err(),
            ImageError::BadMagic
        );
    }

    #[test]
    fn matches_images_to_device_names() {
        let sky = ImageInfo::parse(&header(4, "17_5_8")).unwrap();
        let gnd = ImageInfo::parse(&header(5, "17_5_8")).unwrap();
        let transmitter = device("Ascent_GT_pro");
        let receiver = device("Ascent_Gnd");
        assert!(sky.matches(&transmitter));
        assert!(!gnd.matches(&transmitter));
        assert!(gnd.matches(&receiver));
        assert!(!sky.matches(&receiver));
    }

    #[test]
    fn refuses_old_and_downgrade_versions() {
        let old = ImageInfo::parse(&header(4, "16_4_3")).unwrap();
        assert!(old.is_flashable(None).is_err(), "ниже 16_4_4");

        let current = Version::parse("17_5_8").unwrap();
        let same = ImageInfo::parse(&header(4, "17_5_8")).unwrap();
        assert!(same.is_flashable(Some(current)).is_err(), "та же версия");

        let newer = ImageInfo::parse(&header(4, "18_21_10")).unwrap();
        assert!(newer.is_flashable(Some(current)).is_ok());
        assert!(newer.is_flashable(None).is_ok());
    }

    #[test]
    fn parses_version_text() {
        assert_eq!(
            Version::parse("18_21_10"),
            Some(Version {
                major: 18,
                minor: 21,
                patch: 10
            })
        );
        assert_eq!(Version::parse("19_2"), None);
        assert_eq!(Version::parse("19_2_14_1"), None);
        assert_eq!(Version::parse("a_b_c"), None);
        assert_eq!(Version::MINIMUM.to_string(), "16_4_4");
        assert_eq!(ImageKind::Transmitter.code(), 4);
    }
}
