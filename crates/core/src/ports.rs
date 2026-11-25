use std::fmt;

/// USB identifiers are kept as four hex digits so they can be compared with Windows device IDs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct UsbId(u16);

impl UsbId {
    pub const fn new(value: u16) -> Self {
        Self(value)
    }

    pub fn from_hex(text: &str) -> Option<Self> {
        u16::from_str_radix(text.trim_start_matches("0x"), 16)
            .ok()
            .map(Self)
    }

    pub fn value(self) -> u16 {
        self.0
    }
}

impl fmt::Display for UsbId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{:04X}", self.0)
    }
}

/// A serial port plus the USB identifiers Windows reports for it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PortInfo {
    pub name: String,
    pub usb: Option<(UsbId, UsbId)>,
}

impl PortInfo {
    pub fn display_name(&self) -> String {
        match self.usb {
            Some((vid, pid)) => format!("{} ({vid}:{pid})", self.name),
            None => self.name.clone(),
        }
    }
}

/// Enumerate ports without opening them: opening a port can reset attached hardware.
pub fn list_ports() -> Result<Vec<PortInfo>, serialport::Error> {
    let mut ports = serialport::available_ports()?
        .into_iter()
        .map(|port| PortInfo {
            usb: match port.port_type {
                serialport::SerialPortType::UsbPort(usb) => {
                    Some((UsbId::new(usb.vid), UsbId::new(usb.pid)))
                }
                _ => None,
            },
            name: port.port_name,
        })
        .collect::<Vec<_>>();
    ports.sort_by_key(|port| port_number(&port.name));
    Ok(ports)
}

/// Sort COM10 after COM9 instead of lexicographically.
fn port_number(name: &str) -> (u8, u32, String) {
    let digits: String = name.chars().filter(char::is_ascii_digit).collect();
    match digits.parse() {
        Ok(number) if !digits.is_empty() => (0, number, name.to_string()),
        _ => (1, 0, name.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_usb_ids_from_hex() {
        assert_eq!(UsbId::from_hex("1d76").unwrap().to_string(), "1D76");
        assert_eq!(UsbId::from_hex("0x0483").unwrap().value(), 0x0483);
        assert!(UsbId::from_hex("DF11").is_some());
        assert!(UsbId::from_hex("zz").is_none());
    }

    #[test]
    fn orders_numeric_port_names() {
        let mut names = ["COM10", "COM9", "COM1"];
        names.sort_by_key(|name| port_number(name));
        assert_eq!(names, ["COM1", "COM9", "COM10"]);
    }

    #[test]
    fn formats_ports_with_and_without_usb_ids() {
        let usb = PortInfo {
            name: "COM4".into(),
            usb: Some((UsbId::new(0x1d76), UsbId::new(0x0101))),
        };
        assert_eq!(usb.display_name(), "COM4 (1D76:0101)");
        let plain = PortInfo {
            name: "COM1".into(),
            usb: None,
        };
        assert_eq!(plain.display_name(), "COM1");
    }
}
