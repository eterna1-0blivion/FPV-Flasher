use crate::ports::PortInfo;

/// One device class we recognise. `vids` is a list because a vendor may ship several revisions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DeviceKind {
    pub id: &'static str,
    pub title: &'static str,
    pub vids: &'static [u16],
    pub pids: &'static [u16],
}

impl DeviceKind {
    pub fn matches(&self, port: &PortInfo) -> bool {
        match port.usb {
            // An empty PID list means the vendor ID alone is enough to recognise the device.
            Some((vid, pid)) => {
                self.vids.contains(&vid.value())
                    && (self.pids.is_empty() || self.pids.contains(&pid.value()))
            }
            None => false,
        }
    }
}

/// Known FPV hardware. Values come from official documentation and from real devices.
pub const KNOWN_DEVICES: &[DeviceKind] = &[
    DeviceKind {
        id: "walksnail",
        title: "Walksnail (Ascent)",
        vids: &[0x1D76],
        pids: &[0x0101],
    },
    DeviceKind {
        // WCH CH340/CH341 adapters: radio modules, and the Caddx GM V2 gimbal. PID varies by chip.
        id: "ch34x",
        title: "USB-UART CH34x (радиомодуль или гимбал Caddx GM V2)",
        vids: &[0x1A86],
        pids: &[],
    },
    DeviceKind {
        id: "stm32-vcp",
        title: "STM32 виртуальный COM (полётный контроллер)",
        vids: &[0x0483],
        pids: &[0x5740],
    },
];

/// STM32 ROM bootloader in DFU mode: raw USB, never a COM port.
pub const STM32_DFU: (u16, u16) = (0x0483, 0xDF11);

pub fn identify(port: &PortInfo) -> Option<&'static DeviceKind> {
    KNOWN_DEVICES.iter().find(|kind| kind.matches(port))
}

/// Label a port for menus: recognised hardware, unknown USB, or plain port name.
pub fn describe(port: &PortInfo) -> String {
    match identify(port) {
        Some(kind) => format!("{} — {}", port.display_name(), kind.title),
        None => port.display_name(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ports::UsbId;

    fn port(vid: u16, pid: u16) -> PortInfo {
        PortInfo {
            name: "COM4".into(),
            usb: Some((UsbId::new(vid), UsbId::new(pid))),
        }
    }

    #[test]
    fn recognises_walksnail_and_stm32_from_real_observations() {
        assert_eq!(identify(&port(0x1D76, 0x0101)).unwrap().id, "walksnail");
        assert_eq!(
            describe(&port(0x1D76, 0x0101)),
            "COM4 (1D76:0101) — Walksnail (Ascent)"
        );
        let stm32 = identify(&port(0x0483, 0x5740)).unwrap();
        assert!(stm32.id == "stm32-vcp", "первым в списке идёт STM32 VCP");
    }

    #[test]
    fn ignores_unknown_or_non_usb_ports() {
        assert!(identify(&port(0x1234, 0x5678)).is_none());
        assert!(
            identify(&PortInfo {
                name: "COM1".into(),
                usb: None
            })
            .is_none()
        );
        assert_eq!(
            describe(&PortInfo {
                name: "COM1".into(),
                usb: None
            }),
            "COM1"
        );
    }

    #[test]
    fn ch34x_matches_vendor_without_pid_filter() {
        assert_eq!(identify(&port(0x1A86, 0x7523)).unwrap().id, "ch34x");
        assert_eq!(identify(&port(0x1A86, 0x55D4)).unwrap().id, "ch34x");
    }

    #[test]
    fn a_known_vendor_with_a_different_pid_is_not_a_walksnail() {
        assert!(identify(&port(0x1D76, 0x9999)).is_none());
        assert!(identify(&port(0x0483, 0xDF11)).is_none(), "DFU не COM-порт");
    }
}
