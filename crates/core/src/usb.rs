use nusb::MaybeFuture;
use std::fmt;

/// DFU-capable bootloaders are raw USB devices, so `serialport` can never see them.
/// Values come from ST (STM32) and Artery (AT32) documentation; AT32 boards appear in FPV hardware.
pub const DFU_TARGETS: &[(&str, u16, u16)] = &[
    ("STM32 DFU (STM32Cube bootloader)", 0x0483, 0xDF11),
    ("AT32 DFU (Artery bootloader)", 0x2E3C, 0xDF11),
];

#[derive(Clone, PartialEq, Eq)]
pub struct DfuDevice {
    pub title: &'static str,
    pub vendor_id: u16,
    pub product_id: u16,
    pub serial: Option<String>,
}

impl fmt::Debug for DfuDevice {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DfuDevice")
            .field("title", &self.title)
            .field("vid", &format_args!("{:04X}", self.vendor_id))
            .field("pid", &format_args!("{:04X}", self.product_id))
            .field("serial", &self.serial)
            .finish()
    }
}

impl DfuDevice {
    pub fn display_name(&self) -> String {
        let mut text = format!(
            "{} [{:04X}:{:04X}]",
            self.title, self.vendor_id, self.product_id
        );
        if let Some(serial) = &self.serial {
            text.push_str(&format!(" серийный номер {serial}"));
        }
        text
    }
}

fn title_for(vendor_id: u16, product_id: u16) -> Option<&'static str> {
    DFU_TARGETS
        .iter()
        .find(|(_, vid, pid)| *vid == vendor_id && *pid == product_id)
        .map(|(title, _, _)| *title)
}

/// Enumerate USB devices and keep bootloaders that can accept a DFU transfer.
pub fn list_dfu_devices() -> Result<Vec<DfuDevice>, nusb::Error> {
    let mut found = Vec::new();
    for device in nusb::list_devices().wait()? {
        let (vendor_id, product_id) = (device.vendor_id(), device.product_id());
        if let Some(title) = title_for(vendor_id, product_id) {
            found.push(DfuDevice {
                title,
                vendor_id,
                product_id,
                serial: device.serial_number().map(str::to_string),
            });
        }
    }
    Ok(found)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognises_stm32_and_at32_bootloaders_only() {
        assert!(title_for(0x0483, 0xDF11).unwrap().contains("STM32"));
        assert!(title_for(0x2E3C, 0xDF11).unwrap().contains("AT32"));
        assert!(title_for(0x0483, 0x5740).is_none(), "VCP не DFU");
        assert!(title_for(0x1D76, 0x0101).is_none(), "Walksnail не DFU");
    }

    #[test]
    fn formats_a_device_without_panicking_on_missing_serial() {
        let device = DfuDevice {
            title: title_for(0x0483, 0xDF11).unwrap(),
            vendor_id: 0x0483,
            product_id: 0xDF11,
            serial: None,
        };
        assert_eq!(
            device.display_name(),
            "STM32 DFU (STM32Cube bootloader) [0483:DF11]"
        );
    }
}
