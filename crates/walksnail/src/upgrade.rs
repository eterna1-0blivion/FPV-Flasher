//! Upgrade sequence for Walksnail Ascent, derived from the Caddx capture.
//!
//! These functions only build frames and interpret answers; they never touch a port.
//! The caller writes each frame and feeds the ACK back in.

use std::fmt;

use crate::{
    device::DeviceInfo,
    frame::{ArCommand, Frame},
    image::{ImageInfo, Version},
};

/// Payload chunk size that Caddx uses for `SENDFILE_DATA`.
pub const CHUNK_SIZE: usize = 1 << 20;

/// Fixed 32-byte `REBOOT` payload that asks the device to enter its clean system.
pub fn reboot_clean_payload() -> Vec<u8> {
    let mut payload = vec![0_u8; 32];
    payload[..5].copy_from_slice(b"clean");
    payload
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Step {
    FindDevice {
        seq: u32,
    },
    Reboot {
        seq: u32,
    },
    RemoteUpgrade {
        seq: u32,
    },
    SendFileStart {
        seq: u32,
        info: Vec<u8>,
    },
    SendFileData {
        seq: u32,
        offset: usize,
        chunk: Vec<u8>,
    },
    SendFileEnd {
        seq: u32,
    },
    UpgradeStatus {
        seq: u32,
    },
}

impl Step {
    pub fn seq(&self) -> u32 {
        match self {
            Self::FindDevice { seq }
            | Self::Reboot { seq }
            | Self::RemoteUpgrade { seq }
            | Self::SendFileStart { seq, .. }
            | Self::SendFileData { seq, .. }
            | Self::SendFileEnd { seq }
            | Self::UpgradeStatus { seq } => *seq,
        }
    }

    pub fn frame(&self) -> Frame {
        match self {
            Self::FindDevice { seq } => Frame::request(ArCommand::FindDevice, *seq, Vec::new()),
            Self::Reboot { seq } => Frame::request(ArCommand::Reboot, *seq, reboot_clean_payload()),
            Self::RemoteUpgrade { seq } => {
                Frame::request(ArCommand::RemoteUpgrade, *seq, Vec::new())
            }
            Self::SendFileStart { seq, info } => {
                Frame::request(ArCommand::SendFileStart, *seq, info.clone())
            }
            Self::SendFileData { seq, chunk, .. } => {
                Frame::request(ArCommand::SendFileData, *seq, chunk.clone())
            }
            Self::SendFileEnd { seq } => Frame::request(ArCommand::SendFileEnd, *seq, Vec::new()),
            Self::UpgradeStatus { seq } => {
                Frame::request(ArCommand::UpgradeStatus, *seq, Vec::new())
            }
        }
    }
}

impl fmt::Display for Step {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FindDevice { seq } => write!(formatter, "{seq}: FIND_DEVICE"),
            Self::Reboot { seq } => write!(formatter, "{seq}: REBOOT clean"),
            Self::RemoteUpgrade { seq } => write!(formatter, "{seq}: REMOTE_UPGRADE"),
            Self::SendFileStart { seq, .. } => write!(formatter, "{seq}: SENDFILE_START"),
            Self::SendFileData { seq, offset, chunk } => write!(
                formatter,
                "{seq}: SENDFILE_DATA {} Б со смещения {offset}",
                chunk.len()
            ),
            Self::SendFileEnd { seq } => write!(formatter, "{seq}: SENDFILE_END"),
            Self::UpgradeStatus { seq } => write!(formatter, "{seq}: UPGRADE_STATUS"),
        }
    }
}

/// `ArFileInfo`: 64-byte hex MD5, `u32` length, `u32` saveAsFile, 128-byte device path,
/// 128-byte local path. Layout confirmed by the captured `SENDFILE_START` frame.
pub fn file_info_payload(file: &[u8], device_path: &str) -> Vec<u8> {
    let mut payload = vec![0_u8; 328];
    let digest = md5_of(file);
    let hex = digest
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    payload[..32].copy_from_slice(hex.as_bytes());
    payload[64..68].copy_from_slice(&(file.len() as u32).to_le_bytes());
    payload[68..72].copy_from_slice(&1_u32.to_le_bytes());
    let path = device_path.as_bytes();
    let len = path.len().min(127);
    payload[72..72 + len].copy_from_slice(&path[..len]);
    payload
}

/// MD5 of the image, the same value the device checks after the transfer.
pub fn md5_of(data: &[u8]) -> [u8; 16] {
    use md5::{Digest, Md5};
    let mut hasher = Md5::new();
    hasher.update(data);
    hasher.finalize().into()
}

/// Split the image into the chunks that Caddx sends, with their offsets.
pub fn chunks(image: &[u8]) -> Vec<(usize, &[u8])> {
    let mut result = Vec::new();
    let mut offset = 0;
    for chunk in image.chunks(CHUNK_SIZE) {
        result.push((offset, chunk));
        offset += chunk.len();
    }
    result
}

/// Full upgrade plan, in the order observed in the capture. After every step the caller
/// must wait for an ACK with the same `seq`.
pub fn plan(image: &[u8]) -> Vec<Step> {
    let mut steps = Vec::new();
    let mut seq = 0_u32;
    let mut push = |step: Step, seq: &mut u32| {
        steps.push(step);
        *seq += 1;
    };
    push(Step::FindDevice { seq }, &mut seq);
    push(Step::Reboot { seq }, &mut seq);
    push(Step::RemoteUpgrade { seq }, &mut seq);
    push(
        Step::SendFileStart {
            seq,
            info: file_info_payload(image, "/tmp/pc/update.img"),
        },
        &mut seq,
    );
    for (offset, chunk) in chunks(image) {
        push(
            Step::SendFileData {
                seq,
                offset,
                chunk: chunk.to_vec(),
            },
            &mut seq,
        );
    }
    push(Step::SendFileEnd { seq }, &mut seq);
    push(Step::UpgradeStatus { seq }, &mut seq);
    steps
}

/// Status reported by `UPGRADE_STATUS`: 72-byte payload, percent then state code.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UpgradeStatus {
    pub percent: u32,
    pub status: u32,
}

impl UpgradeStatus {
    pub fn parse(payload: &[u8]) -> Option<Self> {
        if payload.len() < 8 {
            return None;
        }
        Some(Self {
            percent: u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]),
            status: u32::from_le_bytes([payload[4], payload[5], payload[6], payload[7]]),
        })
    }

    pub fn is_finished(self) -> bool {
        self.status == 3
    }

    pub fn is_failed(self) -> bool {
        (self.status as i32) < 0 || self.status == 5
    }
}

impl fmt::Display for UpgradeStatus {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}% — {}",
            self.percent,
            status_text(self.status)
        )
    }
}

fn status_text(status: u32) -> String {
    match status as i32 {
        0 => "проверка образа".to_string(),
        1 => "запуск".to_string(),
        2 => "в процессе".to_string(),
        3 => "готово".to_string(),
        4 => "вход в clean-систему".to_string(),
        5 => "ошибка".to_string(),
        -1 => "нет SD-карты".to_string(),
        -2 => "нет связи с ПК".to_string(),
        -3 => "файл не найден".to_string(),
        -4 => "файл повреждён".to_string(),
        -5 => "не та плата".to_string(),
        -6 => "версия слишком мала".to_string(),
        -7 => "неверный режим".to_string(),
        -8 => "неверный тип образа".to_string(),
        -9 => "ошибка CRC".to_string(),
        -10 => "превышен максимум".to_string(),
        -11 => "ошибка flash".to_string(),
        -12 => "неизвестный ID чипа".to_string(),
        other => format!("неизвестный статус {other}"),
    }
}

/// Checks that must pass before anything is written to a device.
pub fn preflight(image: &ImageInfo, device: &DeviceInfo) -> Result<(), String> {
    if !image.matches(device) {
        return Err(format!(
            "образ для «{}» не подходит устройству «{}»",
            image.kind, device.device_name
        ));
    }
    let current = device.firmware_version().and_then(Version::parse);
    image.is_flashable(current)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_follows_the_captured_order() {
        let image = vec![7_u8; 100];
        let steps = plan(&image);
        assert_eq!(steps[0].frame().command.code(), 0x3C, "сначала FIND_DEVICE");
        assert_eq!(steps[1].frame().command.code(), 0x03, "затем REBOOT");
        assert_eq!(
            steps[2].frame().command.code(),
            0x72,
            "затем REMOTE_UPGRADE"
        );
        assert_eq!(
            steps[3].frame().command.code(),
            0x73,
            "затем SENDFILE_START"
        );
        assert_eq!(steps[4].frame().command.code(), 0x74, "затем данные");
        assert_eq!(steps[5].frame().command.code(), 0x75, "затем SENDFILE_END");
        assert_eq!(steps[6].frame().command.code(), 0x76, "затем статус");
        assert_eq!(steps.len(), 7);

        // Sequence numbers increase by one and each frame encodes and decodes intact.
        for (index, step) in steps.iter().enumerate() {
            assert_eq!(step.seq(), index as u32);
            let bytes = step.frame().encode();
            Frame::decode(&bytes).unwrap();
        }
    }

    #[test]
    fn big_images_are_split_into_one_megabyte_chunks() {
        let image = vec![0_u8; CHUNK_SIZE * 2 + 10];
        let parts = chunks(&image);
        assert_eq!(parts.len(), 3);
        assert_eq!(parts[0], (0, &image[..CHUNK_SIZE]));
        assert_eq!(parts[1].0, CHUNK_SIZE);
        assert_eq!(parts[2].0, CHUNK_SIZE * 2);
        assert_eq!(parts[2].1.len(), 10);
        assert_eq!(
            parts.iter().map(|(_, c)| c.len()).sum::<usize>(),
            image.len()
        );
    }

    #[test]
    fn reboot_payload_is_a_fixed_32_byte_field() {
        let payload = reboot_clean_payload();
        assert_eq!(payload.len(), 32);
        assert!(payload.starts_with(b"clean"));
        assert!(payload[5..].iter().all(|&b| b == 0));
    }

    #[test]
    fn file_info_matches_the_captured_frame() {
        // MD5 and size from the captured SENDFILE_START for the 17_5_8 transmitter image.
        let file = b"stub";
        let info = file_info_payload(file, "/tmp/pc/Ascent_G_Sky_17_5_8.img");
        assert_eq!(info.len(), 328, "фиксированный размер ArFileInfo");
        assert_eq!(
            u32::from_le_bytes([info[64], info[65], info[66], info[67]]),
            file.len() as u32
        );
        assert_eq!(
            u32::from_le_bytes([info[68], info[69], info[70], info[71]]),
            1,
            "saveAsFile"
        );
        let path = "/tmp/pc/Ascent_G_Sky_17_5_8.img";
        assert_eq!(
            &info[72..72 + path.len()],
            path.as_bytes(),
            "путь на устройстве"
        );
        assert!(
            info[72 + path.len()..200].iter().all(|&b| b == 0),
            "остаток поля пути добит нулями"
        );
    }

    #[test]
    fn hashes_the_real_image_like_caddx_did() {
        // The 17_5_8 transmitter image is 12 164 404 bytes with this MD5; Caddx sent both
        // in its SENDFILE_START frame. Skipped when the analysed image is not present.
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../analyze/Walksnail/firmware/Ascent_G_Sky_17_5_8.img");
        let Ok(bytes) = std::fs::read(&path) else {
            return;
        };
        assert_eq!(bytes.len(), 12_164_404);
        let digest = md5_of(&bytes);
        let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
        assert_eq!(hex, "03e2f83d1fd89a3071f5b0ba35e3d857");
    }

    #[test]
    fn md5_matches_known_vectors() {
        assert_eq!(
            md5_of(b""),
            [
                0xd4, 0x1d, 0x8c, 0xd9, 0x8f, 0x00, 0xb2, 0x04, 0xe9, 0x80, 0x09, 0x98, 0xec, 0xf8,
                0x42, 0x7e
            ]
        );
        assert_eq!(
            md5_of(b"abc"),
            [
                0x90, 0x01, 0x50, 0x98, 0x3c, 0xd2, 0x4f, 0xb0, 0xd6, 0x96, 0x3f, 0x7d, 0x28, 0xe1,
                0x7f, 0x72
            ]
        );
    }

    #[test]
    fn reads_upgrade_status_and_classifies_it() {
        let mut payload = vec![0_u8; 72];
        payload[..4].copy_from_slice(&38_u32.to_le_bytes());
        payload[4..8].copy_from_slice(&2_u32.to_le_bytes());
        let status = UpgradeStatus::parse(&payload).unwrap();
        assert_eq!(
            status,
            UpgradeStatus {
                percent: 38,
                status: 2
            }
        );
        assert!(!status.is_finished());
        assert!(!status.is_failed());
        assert_eq!(status.to_string(), "38% — в процессе");

        let mut done = payload.clone();
        done[..4].copy_from_slice(&100_u32.to_le_bytes());
        done[4..8].copy_from_slice(&3_u32.to_le_bytes());
        assert!(UpgradeStatus::parse(&done).unwrap().is_finished());

        let mut failed = payload.clone();
        failed[4..8].copy_from_slice(&(-6_i32 as u32).to_le_bytes());
        let failure = UpgradeStatus::parse(&failed).unwrap();
        assert!(failure.is_failed());
        assert!(failure.to_string().contains("версия слишком мала"));

        assert!(UpgradeStatus::parse(&payload[..4]).is_none());
    }

    #[test]
    fn preflight_refuses_wrong_kind_and_downgrades() {
        let device = crate::DeviceInfo {
            max_receive_size: 0,
            sdk_version: String::new(),
            device_name: "Ascent_GT_pro".into(),
            cpu_temp: 0,
            firmware: "Ascent_G_Sky_17_5_8".into(),
            serial: String::new(),
            hardware: String::new(),
            status: 0,
            detail: String::new(),
        };
        let newer = ImageInfo::parse(&crate::image::tests::header(4, "19_2_14")).unwrap();
        assert!(preflight(&newer, &device).is_ok());

        let same = ImageInfo::parse(&crate::image::tests::header(4, "17_5_8")).unwrap();
        assert!(preflight(&same, &device).is_err(), "понижение запрещено");

        let receiver = ImageInfo::parse(&crate::image::tests::header(5, "19_2_14")).unwrap();
        assert!(preflight(&receiver, &device).is_err(), "образ приёмника");
    }
}
