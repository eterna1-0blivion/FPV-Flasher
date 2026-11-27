//! Кадр протокола AR: заголовок 36 байт, payload и CRC32.

use std::fmt;

pub const MAGIC: u32 = 0x4152_544F;
pub const VERSION: u16 = 0x0CDC;
pub const UNUSED: u16 = 0xABCD;
pub const FORMAT_BINARY: u16 = 2;
pub const HEADER_LEN: usize = 36;
pub const TYPE_REQUEST: u16 = 1;
pub const TYPE_ACK: u16 = 2;

/// Команды, встреченные в дампах и подтверждённые таблицей команд Caddx.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArCommand {
    Reboot,
    FindDevice,
    RemoteUpgrade,
    SendFileStart,
    SendFileData,
    SendFileEnd,
    UpgradeStatus,
    SendText,
    GetBbFreqConfig,
    SetBbFreqConfig,
    ResetBbFreqConfig,
    Unknown(u32),
}

impl ArCommand {
    pub const fn code(self) -> u32 {
        match self {
            Self::Reboot => 0x03,
            Self::FindDevice => 0x3C,
            Self::RemoteUpgrade => 0x72,
            Self::SendFileStart => 0x73,
            Self::SendFileData => 0x74,
            Self::SendFileEnd => 0x75,
            Self::UpgradeStatus => 0x76,
            Self::SendText => 0x77,
            Self::GetBbFreqConfig => 0x78,
            Self::SetBbFreqConfig => 0x79,
            Self::ResetBbFreqConfig => 0x7A,
            Self::Unknown(code) => code,
        }
    }

    pub const fn from_code(code: u32) -> Self {
        match code {
            0x03 => Self::Reboot,
            0x3C => Self::FindDevice,
            0x72 => Self::RemoteUpgrade,
            0x73 => Self::SendFileStart,
            0x74 => Self::SendFileData,
            0x75 => Self::SendFileEnd,
            0x76 => Self::UpgradeStatus,
            0x77 => Self::SendText,
            0x78 => Self::GetBbFreqConfig,
            0x79 => Self::SetBbFreqConfig,
            0x7A => Self::ResetBbFreqConfig,
            other => Self::Unknown(other),
        }
    }
}

/// Направление кадра: запрос от ПК или ответ (ACK) от устройства.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Request,
    Ack,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    Known(ArCommand),
    Unknown(u32),
}

impl Command {
    pub const fn code(self) -> u32 {
        match self {
            Self::Known(command) => command.code(),
            Self::Unknown(code) => code,
        }
    }
}

impl fmt::Display for ArCommand {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Reboot => write!(formatter, "REBOOT"),
            Self::FindDevice => write!(formatter, "FIND_DEVICE"),
            Self::RemoteUpgrade => write!(formatter, "REMOTE_UPGRADE"),
            Self::SendFileStart => write!(formatter, "SENDFILE_START"),
            Self::SendFileData => write!(formatter, "SENDFILE_DATA"),
            Self::SendFileEnd => write!(formatter, "SENDFILE_END"),
            Self::UpgradeStatus => write!(formatter, "UPGRADE_STATUS"),
            Self::SendText => write!(formatter, "SEND_TEXT"),
            Self::GetBbFreqConfig => write!(formatter, "GET_BB_FREQ_CONFIG"),
            Self::SetBbFreqConfig => write!(formatter, "SET_BB_FREQ_CONFIG"),
            Self::ResetBbFreqConfig => write!(formatter, "FACTRESET_BB_FREQ_CONFIG"),
            Self::Unknown(code) => write!(formatter, "0x{code:02X}"),
        }
    }
}

impl fmt::Display for Command {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Known(command) => command.fmt(formatter),
            Self::Unknown(code) => write!(formatter, "0x{code:02X}"),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum FrameError {
    TooShort { got: usize },
    BadMagic(u32),
    BadVersion(u16),
    LengthMismatch { declared: u32, actual: usize },
    BadCrc { expected: u32, calculated: u32 },
}

impl fmt::Display for FrameError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooShort { got } => {
                write!(
                    formatter,
                    "кадр короче заголовка: {got} из {HEADER_LEN} байт"
                )
            }
            Self::BadMagic(value) => write!(formatter, "неверная сигнатура кадра 0x{value:08X}"),
            Self::BadVersion(value) => {
                write!(formatter, "неизвестная версия протокола 0x{value:04X}")
            }
            Self::LengthMismatch { declared, actual } => write!(
                formatter,
                "длина payload в заголовке {declared}, фактически получено {actual}"
            ),
            Self::BadCrc {
                expected,
                calculated,
            } => write!(
                formatter,
                "CRC32 не совпал: в кадре 0x{expected:08X}, расчёт 0x{calculated:08X}"
            ),
        }
    }
}

impl std::error::Error for FrameError {}

const CRC_TABLE: [u32; 256] = {
    let mut table = [0_u32; 256];
    let mut index = 0;
    while index < 256 {
        let mut crc = index as u32;
        let mut bit = 0;
        while bit < 8 {
            crc = if crc & 1 == 1 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
            bit += 1;
        }
        table[index] = crc;
        index += 1;
    }
    table
};

fn crc32_update(mut crc: u32, data: &[u8]) -> u32 {
    for &byte in data {
        crc = CRC_TABLE[((crc ^ u32::from(byte)) & 0xFF) as usize] ^ (crc >> 8);
    }
    crc
}

/// Табличный CRC32 с отражённым полиномом IEEE.
/// Начальное значение и финальный XOR — `0xFFFFFFFF`.
pub fn crc32(data: &[u8]) -> u32 {
    crc32_update(u32::MAX, data) ^ u32::MAX
}

fn put_u16(buffer: &mut [u8], offset: usize, value: u16) {
    buffer[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn put_u32(buffer: &mut [u8], offset: usize, value: u32) {
    buffer[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn get_u16(buffer: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([buffer[offset], buffer[offset + 1]])
}

fn get_u32(buffer: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        buffer[offset],
        buffer[offset + 1],
        buffer[offset + 2],
        buffer[offset + 3],
    ])
}

/// Разобранный кадр владеет полезной нагрузкой и не зависит от исходного буфера.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Frame {
    pub version: u16,
    pub kind: Kind,
    pub msgid: u16,
    /// В запросах `0xABCD`, в ответах устройства `0`. Храним как есть, чтобы
    /// пересборка снятого кадра совпадала с оригиналом побайтно.
    pub unused: u16,
    pub command: Command,
    pub format: u16,
    pub userid: u16,
    pub seq: u32,
    pub retry: u32,
    pub payload: Vec<u8>,
}

impl Frame {
    pub fn request(command: ArCommand, seq: u32, payload: Vec<u8>) -> Self {
        Self {
            version: VERSION,
            kind: Kind::Request,
            msgid: 0,
            unused: UNUSED,
            command: Command::Known(command),
            format: FORMAT_BINARY,
            userid: 0,
            seq,
            retry: 0,
            payload,
        }
    }

    pub fn ack(command: ArCommand, seq: u32, payload: Vec<u8>) -> Self {
        Self {
            kind: Kind::Ack,
            unused: 0,
            ..Self::request(command, seq, payload)
        }
    }

    /// Собирает заголовок и данные, затем записывает CRC32 в заголовок.
    pub fn encode(&self) -> Vec<u8> {
        let mut bytes = vec![0_u8; HEADER_LEN + self.payload.len()];
        put_u32(&mut bytes, 0, MAGIC);
        put_u16(&mut bytes, 4, self.version);
        put_u16(
            &mut bytes,
            6,
            match self.kind {
                Kind::Request => TYPE_REQUEST,
                Kind::Ack => TYPE_ACK,
            },
        );
        put_u16(&mut bytes, 8, self.msgid);
        put_u16(&mut bytes, 10, self.unused);
        put_u32(&mut bytes, 12, self.command.code());
        put_u16(&mut bytes, 16, self.format);
        put_u16(&mut bytes, 18, self.userid);
        put_u32(&mut bytes, 20, self.payload.len() as u32);
        put_u32(&mut bytes, 24, self.seq);
        put_u32(&mut bytes, 28, self.retry);
        put_u32(&mut bytes, 32, 0);
        bytes[HEADER_LEN..].copy_from_slice(&self.payload);
        let crc = crc32(&bytes);
        put_u32(&mut bytes, 32, crc);
        bytes
    }

    /// Разбирает кадр, снятый с устройства. CRC покрывает заголовок с обнулённым
    /// собственным полем плюс ровно `length` байт payload.
    pub fn decode(bytes: &[u8]) -> Result<Self, FrameError> {
        Self::decode_allow_trailing(bytes, false)
    }

    /// Разбор буфера потока, где друг за другом могут идти несколько кадров.
    pub fn decode_allow_trailing(bytes: &[u8], allow_trailing: bool) -> Result<Self, FrameError> {
        if bytes.len() < HEADER_LEN {
            return Err(FrameError::TooShort { got: bytes.len() });
        }
        let magic = get_u32(bytes, 0);
        if magic != MAGIC {
            return Err(FrameError::BadMagic(magic));
        }
        let version = get_u16(bytes, 4);
        if version != VERSION {
            return Err(FrameError::BadVersion(version));
        }
        let declared = get_u32(bytes, 20) as usize;
        let available = bytes.len() - HEADER_LEN;
        if declared > available || (!allow_trailing && declared != available) {
            return Err(FrameError::LengthMismatch {
                declared: declared as u32,
                actual: available,
            });
        }

        let expected = get_u32(bytes, 32);
        let mut header = [0_u8; HEADER_LEN];
        header.copy_from_slice(&bytes[..HEADER_LEN]);
        header[32..36].fill(0);
        let mut running = u32::MAX;
        for &byte in header
            .iter()
            .chain(bytes[HEADER_LEN..HEADER_LEN + declared].iter())
        {
            running ^= u32::from(byte);
            for _ in 0..8 {
                running = if running & 1 == 1 {
                    (running >> 1) ^ 0xEDB8_8320
                } else {
                    running >> 1
                };
            }
        }
        let calculated = running ^ u32::MAX;
        if calculated != expected {
            return Err(FrameError::BadCrc {
                expected,
                calculated,
            });
        }

        let kind = match get_u16(bytes, 6) {
            TYPE_ACK => Kind::Ack,
            _ => Kind::Request,
        };
        Ok(Self {
            version,
            kind,
            msgid: get_u16(bytes, 8),
            unused: get_u16(bytes, 10),
            command: Command::Known(ArCommand::from_code(get_u32(bytes, 12))),
            format: get_u16(bytes, 16),
            userid: get_u16(bytes, 18),
            seq: get_u32(bytes, 24),
            retry: get_u32(bytes, 28),
            payload: bytes[HEADER_LEN..HEADER_LEN + declared].to_vec(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn from_hex(text: &str) -> Vec<u8> {
        let clean: String = text.chars().filter(|c| c.is_ascii_hexdigit()).collect();
        clean
            .as_bytes()
            .chunks(2)
            .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
            .collect()
    }

    // Реальные байты, снятые с Ascent GT через Caddx PC Tool 2.2.8.
    const FIND_DEVICE_REQUEST: &str =
        "4f545241dc0c01000000cdab3c00000002000000000000000000000000000000d1dd63d1";
    const REBOOT_CLEAN: &str =
        "4f545241dc0c01000000cdab03000000020000002000000001000000000000005122e339";
    const FIND_DEVICE_ACK: &str = "4f545241dc0c0200000000003c000000020000002c01000000000000000000002a21755a000010000000000000000000000000000000000000000000000000000000000000000000417363656e745f47545f70726f0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000002a000000417363656e745f475f536b795f31365f355f3900000000000000000000000000000000000000000000000000000000000000000000000000000000000000000036323039000000000000000000000000000000000000000000000000000000004650562d417363656e742d536b792d3438362d56312e302d312e300000000000000000004f4b0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000";

    #[test]
    fn crc_matches_captured_caddx_frames() {
        for hex in [FIND_DEVICE_REQUEST, FIND_DEVICE_ACK] {
            let bytes = from_hex(hex);
            let frame = Frame::decode(&bytes).unwrap();
            assert_eq!(frame.encode(), bytes, "пересборка должна совпасть побайтно");
        }
        let mut reboot = from_hex(REBOOT_CLEAN);
        reboot.extend_from_slice(b"clean\0\0\0");
        reboot.resize(HEADER_LEN + 32, 0);
        assert_eq!(Frame::decode(&reboot).unwrap().encode(), reboot);
    }

    #[test]
    fn decodes_find_device_request_fields() {
        let frame = Frame::decode(&from_hex(FIND_DEVICE_REQUEST)).unwrap();
        assert_eq!(frame.kind, Kind::Request);
        assert_eq!(frame.command, Command::Known(ArCommand::FindDevice));
        assert_eq!(frame.version, VERSION);
        assert_eq!(frame.format, FORMAT_BINARY);
        assert_eq!(frame.seq, 0);
        assert!(frame.payload.is_empty());
    }

    #[test]
    fn decodes_find_device_ack_payload() {
        let frame = Frame::decode(&from_hex(FIND_DEVICE_ACK)).unwrap();
        assert_eq!(frame.kind, Kind::Ack);
        assert_eq!(frame.payload.len(), 300, "ответ всегда 300 байт");
        let text = |offset: usize, len: usize| {
            let slice = &frame.payload[offset..offset + len];
            let end = slice.iter().position(|&b| b == 0).unwrap_or(slice.len());
            String::from_utf8_lossy(&slice[..end]).to_string()
        };
        assert_eq!(get_u32(&frame.payload, 0), 1_048_576, "receiveMaxSize");
        assert_eq!(get_u32(&frame.payload, 100), 42, "температура процессора");
        assert_eq!(text(4, 32), "", "SDK-версия в этой прошивке пустая");
        assert_eq!(text(36, 64), "Ascent_GT_pro", "DeviceName");
        assert_eq!(text(104, 64), "Ascent_G_Sky_16_5_9", "FirmwareInfo");
        assert_eq!(text(168, 32), "6209", "SN");
        assert_eq!(text(200, 32), "FPV-Ascent-Sky-486-V1.0-1.0", "HardwareVers");
        assert_eq!(get_u32(&frame.payload, 232), 0, "status 0");
        assert_eq!(text(236, 64), "OK", "detail");
    }

    #[test]
    fn reboot_clean_carries_the_mode() {
        let mut bytes = from_hex(REBOOT_CLEAN);
        // Устройство получает payload отдельной записью, фиксированным блоком на 32 байта.
        bytes.extend_from_slice(b"clean\0\0\0");
        bytes.resize(HEADER_LEN + 32, 0);
        let frame = Frame::decode(&bytes).unwrap();
        assert_eq!(frame.command, Command::Known(ArCommand::Reboot));
        assert!(frame.payload.starts_with(b"clean"));
    }

    #[test]
    fn rejects_corrupted_payload() {
        let mut bytes = from_hex(FIND_DEVICE_ACK);
        let last = bytes.len() - 1;
        bytes[last] ^= 0xFF;
        let error = Frame::decode(&bytes).unwrap_err();
        assert!(matches!(error, FrameError::BadCrc { .. }), "{error:?}");
    }

    #[test]
    fn rejects_truncated_oversized_and_foreign_frames() {
        let bytes = from_hex(FIND_DEVICE_REQUEST);
        assert_eq!(
            Frame::decode(&bytes[..10]).unwrap_err(),
            FrameError::TooShort { got: 10 }
        );

        // Заявленная длина превышает размер буфера: кадр должен быть отклонён.
        let mut lying = bytes.clone();
        put_u32(&mut lying, 20, 1_079_918_686);
        assert!(matches!(
            Frame::decode(&lying).unwrap_err(),
            FrameError::LengthMismatch { .. }
        ));

        let mut alien = bytes.clone();
        put_u32(&mut alien, 0, 0xDEAD_BEEF);
        assert!(matches!(
            Frame::decode(&alien).unwrap_err(),
            FrameError::BadMagic(_)
        ));

        let mut other_version = bytes;
        put_u16(&mut other_version, 4, 0x1234);
        assert!(matches!(
            Frame::decode(&other_version).unwrap_err(),
            FrameError::BadVersion(_)
        ));
    }

    #[test]
    fn builds_requests_that_the_device_can_verify() {
        let frame = Frame::request(ArCommand::FindDevice, 7, Vec::new());
        let bytes = frame.encode();
        assert_eq!(bytes.len(), HEADER_LEN);
        assert_eq!(get_u32(&bytes, 12), 0x3C);
        assert_eq!(get_u32(&bytes, 24), 7);
        Frame::decode(&bytes).unwrap();
    }

    #[test]
    fn keeps_unknown_commands_intact() {
        let command = ArCommand::from_code(0x7E);
        assert_eq!(command, ArCommand::Unknown(0x7E));
        assert_eq!(Command::Known(command).to_string(), "0x7E");
        assert_eq!(ArCommand::from_code(0x73), ArCommand::SendFileStart);
        assert_eq!(ArCommand::SendText.code(), 0x77);
    }

    #[test]
    fn splits_frames_from_a_stream_buffer() {
        let first = from_hex(FIND_DEVICE_REQUEST);
        let second = from_hex(REBOOT_CLEAN);
        let mut stream = first.clone();
        stream.extend_from_slice(&second);
        let frame = Frame::decode_allow_trailing(&stream, true).unwrap();
        assert_eq!(frame.command, Command::Known(ArCommand::FindDevice));
        assert!(Frame::decode(&stream).is_err());
    }
}
