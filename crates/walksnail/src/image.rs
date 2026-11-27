//! Контейнер прошивки Walksnail (`*.img`).
//!
//! Заголовок проверен на всех восьми образцах: `41 53 57 00` («ASW\0»), затем `u32` тип
//! (4 — передатчик Sky, 5 — приёмник Gnd), затем `u32` major, `u32` minor, `u32` patch.

use std::fmt;

pub const MAGIC: [u8; 4] = *b"ASW\0";
/// Сигнатура, тип и три числа версии.
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
    /// Минимальная версия, которую принимает Caddx PC Tool; более старые образы
    /// отклоняются ещё до прошивки. Причину порога мы не подтвердили (см. документацию).
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

/// В образах передатчика по этому смещению лежит размер файла, а в четырёх байтах перед ним —
/// CRC32 всего тела начиная с [`BODY_START`]. У 16_5_9 контрольная сумма нулевая.
const SKY_CRC_OFFSET: usize = 112;
const SKY_SIZE_OFFSET: usize = 116;
const BODY_START: usize = 0x80;

/// Результат проверки целостности, записанной в самом образе.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Integrity {
    /// CRC32 в образе совпал с содержимым.
    Verified,
    /// Контрольная сумма в образе не записана (так у 16_5_9 и у приёмников).
    NotRecorded,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImageInfo {
    pub kind: ImageKind,
    pub version: Version,
    pub size: usize,
    pub integrity: Integrity,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ImageError {
    TooShort { got: usize },
    BadMagic,
    SizeMismatch { declared: u32, actual: usize },
    BadCrc { stored: u32, calculated: u32 },
}

impl fmt::Display for ImageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooShort { got } => write!(
                formatter,
                "файл прошивки короче заголовка: {got} из {HEADER_LEN} байт"
            ),
            Self::BadMagic => write!(formatter, "не файл прошивки Walksnail (нет сигнатуры ASW)"),
            Self::SizeMismatch { declared, actual } => write!(
                formatter,
                "размер в заголовке {declared} байт, файл {actual} байт: файл обрезан или дописан"
            ),
            Self::BadCrc { stored, calculated } => write!(
                formatter,
                "контрольная сумма образа не совпала (0x{stored:08X} в заголовке, 0x{calculated:08X} \
                 по содержимому): файл повреждён"
            ),
        }
    }
}

/// Причина отказа писать образ либо то, что пользователь обязан подтвердить.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refusal {
    WrongKind { image: ImageKind, device: String },
    BelowMinimum { image: Version },
    UnknownCurrent,
}

impl fmt::Display for Refusal {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongKind { image, device } => {
                write!(
                    formatter,
                    "образ для «{image}» не подходит устройству «{device}»"
                )
            }
            Self::BelowMinimum { image } => write!(
                formatter,
                "версия {image} ниже {}: такие образы не принимаются (см. docs/protocols)",
                Version::MINIMUM
            ),
            Self::UnknownCurrent => {
                write!(
                    formatter,
                    "не удалось прочитать версию прошивки на устройстве"
                )
            }
        }
    }
}

/// Направление изменения — определяется только по содержимому образа и ответу устройства.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Change {
    Upgrade,
    Reinstall,
    /// Разрешено, только если вызывающий код передал `allow_downgrade`.
    Downgrade,
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

/// Результат проверок вместе с предупреждениями, которые нужно показать перед подтверждением.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Assessment {
    pub change: Change,
    pub current: Version,
    pub warnings: Vec<String>,
}

/// Граница для предупреждения: в захватах переход с 18.21.10 на 16.x или 17.x
/// завершался кодом −8. Причина отказа пока не установлена.
const NEW_LAYOUT_SINCE: Version = Version {
    major: 18,
    minor: 0,
    patch: 0,
};

impl ImageInfo {
    /// Читает тип, версию и целостность из содержимого файла. Имя файла не используется.
    pub fn parse(bytes: &[u8]) -> Result<Self, ImageError> {
        if bytes.len() < HEADER_LEN {
            return Err(ImageError::TooShort { got: bytes.len() });
        }
        if bytes[..4] != MAGIC {
            return Err(ImageError::BadMagic);
        }
        let kind = ImageKind::from_type(u32_at(bytes, 4));
        let mut integrity = Integrity::NotRecorded;
        if kind == ImageKind::Transmitter && bytes.len() >= BODY_START {
            let declared = u32_at(bytes, SKY_SIZE_OFFSET);
            if declared != 0 && declared as usize != bytes.len() {
                return Err(ImageError::SizeMismatch {
                    declared,
                    actual: bytes.len(),
                });
            }
            let stored = u32_at(bytes, SKY_CRC_OFFSET);
            if stored != 0 {
                let calculated = crate::frame::crc32(&bytes[BODY_START..]);
                if calculated != stored {
                    return Err(ImageError::BadCrc { stored, calculated });
                }
                integrity = Integrity::Verified;
            }
        }
        Ok(Self {
            kind,
            version: Version {
                major: u32_at(bytes, 8),
                minor: u32_at(bytes, 12),
                patch: u32_at(bytes, 16),
            },
            size: bytes.len(),
            integrity,
        })
    }

    /// Определяет, что произойдёт при записи. Понижение здесь не отклоняется:
    /// вызывающий код обязан показать `warnings` и получить явное подтверждение.
    pub fn assess(&self, device: &crate::DeviceInfo) -> Result<Assessment, Refusal> {
        let kind_ok = matches!(
            (self.kind, device.side()),
            (ImageKind::Transmitter, Some(crate::Side::Transmitter))
                | (ImageKind::Receiver, Some(crate::Side::Receiver))
        );
        if !kind_ok {
            return Err(Refusal::WrongKind {
                image: self.kind,
                device: device.device_name.clone(),
            });
        }
        if self.version < Version::MINIMUM {
            return Err(Refusal::BelowMinimum {
                image: self.version,
            });
        }
        let current = device
            .firmware_version()
            .and_then(Version::parse)
            .ok_or(Refusal::UnknownCurrent)?;

        let change = match self.version.cmp(&current) {
            std::cmp::Ordering::Greater => Change::Upgrade,
            std::cmp::Ordering::Equal => Change::Reinstall,
            std::cmp::Ordering::Less => Change::Downgrade,
        };
        let mut warnings = Vec::new();
        if change == Change::Downgrade {
            warnings.push(format!(
                "понижение версии {current} → {}: подтверждено успешными перепрошивками",
                self.version
            ));
            if self.kind == ImageKind::Transmitter
                && current >= NEW_LAYOUT_SINCE
                && self.version < NEW_LAYOUT_SINCE
            {
                warnings.push(
                    "с версии 18.x на 17.x и 16.x передатчики во всех проверенных случаях \
                     отвечали «image type error» (-8): прошивка не начнётся, устройство \
                     останется на текущей версии"
                        .to_string(),
                );
            }
        }
        if self.integrity == Integrity::NotRecorded {
            warnings.push(
                "в образе нет контрольной суммы, целостность проверит только устройство"
                    .to_string(),
            );
        }
        Ok(Assessment {
            change,
            current,
            warnings,
        })
    }

    /// Путь для `SENDFILE_START` строится из заголовка, а не из локального имени файла.
    /// При понижении `+200` воспроизводит имя из успешных захватов с ручным
    /// переименованием. Влияние имени на проверки устройства не установлено.
    pub fn device_path(&self, change: Change) -> String {
        let side = match self.kind {
            ImageKind::Receiver => "Gnd",
            _ => "Sky",
        };
        let major = match change {
            Change::Downgrade => self.version.major + 200,
            _ => self.version.major,
        };
        format!(
            "/tmp/pc/Ascent_G_{side}_{major}_{}_{}.img",
            self.version.minor, self.version.patch
        )
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

    pub(crate) fn device(name: &str, firmware: &str) -> crate::DeviceInfo {
        crate::DeviceInfo {
            max_receive_size: 0,
            sdk_version: String::new(),
            device_name: name.into(),
            cpu_temp: 0,
            firmware: firmware.into(),
            serial: String::new(),
            hardware: String::new(),
            status: 0,
            detail: String::new(),
        }
    }

    fn sample(name: &str) -> Option<Vec<u8>> {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../analyze/Walksnail/firmware")
            .join(name);
        std::fs::read(path).ok()
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

    // Реальные имена моделей: GT Pro — передатчик (прошивка Sky), VRX — приёмник (Gnd).
    const GT_PRO: &str = "Ascent_GT_pro";
    const VRX: &str = "Ascent_VRX";

    #[test]
    fn checks_kind_by_content_not_by_name() {
        let sky = ImageInfo::parse(&header(4, "17_5_8")).unwrap();
        let gnd = ImageInfo::parse(&header(5, "17_5_8")).unwrap();
        let transmitter = device(GT_PRO, "Ascent_G_Sky_16_5_9");
        let receiver = device(VRX, "Ascent_G_Gnd_16_5_9");
        assert!(sky.assess(&transmitter).is_ok());
        assert!(matches!(
            gnd.assess(&transmitter),
            Err(Refusal::WrongKind { .. })
        ));
        assert!(gnd.assess(&receiver).is_ok());
        assert!(matches!(
            sky.assess(&receiver),
            Err(Refusal::WrongKind { .. })
        ));
        let unknown = device("new device", "unknown");
        assert!(matches!(
            sky.assess(&unknown),
            Err(Refusal::WrongKind { .. })
        ));
    }

    #[test]
    fn classifies_upgrade_reinstall_and_downgrade() {
        let on_17 = device(GT_PRO, "Ascent_G_Sky_17_5_8");
        let assess = |version: &str| {
            ImageInfo::parse(&header(4, version))
                .unwrap()
                .assess(&on_17)
        };
        assert_eq!(assess("19_2_14").unwrap().change, Change::Upgrade);
        assert_eq!(assess("17_5_8").unwrap().change, Change::Reinstall);
        let down = assess("16_5_9").unwrap();
        assert_eq!(
            down.change,
            Change::Downgrade,
            "понижение разрешено, но помечено"
        );
        assert!(down.warnings.iter().any(|w| w.contains("понижение")));
        assert!(matches!(
            assess("16_4_3"),
            Err(Refusal::BelowMinimum { .. })
        ));
    }

    #[test]
    fn warns_about_downgrading_from_the_18_layout() {
        let on_18 = device(GT_PRO, "Ascent_G_Sky_18_21_10");
        let down = ImageInfo::parse(&header(4, "17_5_8"))
            .unwrap()
            .assess(&on_18)
            .unwrap();
        assert!(down.warnings.iter().any(|w| w.contains("image type error")));
        let on_19 = device(GT_PRO, "Ascent_G_Sky_19_2_14");
        let within = ImageInfo::parse(&header(4, "18_21_10"))
            .unwrap()
            .assess(&on_19)
            .unwrap();
        assert!(
            !within
                .warnings
                .iter()
                .any(|w| w.contains("image type error"))
        );
    }

    #[test]
    fn device_path_is_built_from_the_header() {
        let image = ImageInfo::parse(&header(4, "16_5_9")).unwrap();
        assert_eq!(
            image.device_path(Change::Upgrade),
            "/tmp/pc/Ascent_G_Sky_16_5_9.img"
        );
        assert_eq!(
            image.device_path(Change::Downgrade),
            "/tmp/pc/Ascent_G_Sky_216_5_9.img",
            "как в успешном дампе 17.5.8 → 16.5.9"
        );
        let gnd = ImageInfo::parse(&header(5, "19_2_14")).unwrap();
        assert_eq!(
            gnd.device_path(Change::Reinstall),
            "/tmp/pc/Ascent_G_Gnd_19_2_14.img"
        );
    }

    #[test]
    fn verifies_the_stored_checksum_of_real_images() {
        let expectations = [
            ("Ascent_G_Sky_16_5_9.img", Integrity::NotRecorded),
            ("Ascent_G_Sky_17_5_8.img", Integrity::Verified),
            ("Ascent_G_Sky_18_21_10.img", Integrity::Verified),
            ("Ascent_G_Sky_19_2_14.img", Integrity::Verified),
            ("Ascent_G_Gnd_19_2_14.img", Integrity::NotRecorded),
        ];
        for (name, integrity) in expectations {
            let Some(bytes) = sample(name) else { return };
            let info = ImageInfo::parse(&bytes).unwrap();
            assert_eq!(info.integrity, integrity, "{name}");
            assert!(
                name.contains(&info.version.to_string()),
                "{name}: версия из заголовка"
            );
        }
    }

    #[test]
    fn detects_damaged_and_truncated_real_images() {
        let Some(mut bytes) = sample("Ascent_G_Sky_19_2_14.img") else {
            return;
        };
        let middle = bytes.len() / 2;
        bytes[middle] ^= 0x01;
        assert!(matches!(
            ImageInfo::parse(&bytes),
            Err(ImageError::BadCrc { .. })
        ));
        bytes[middle] ^= 0x01;
        bytes.truncate(bytes.len() - 1);
        assert!(matches!(
            ImageInfo::parse(&bytes),
            Err(ImageError::SizeMismatch { .. })
        ));
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
