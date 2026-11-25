use std::{
    thread,
    time::{Duration, Instant},
};

use crate::ports::{PortInfo, UsbId, list_ports};

/// Poll interval chosen to beat the fixed 10-second waits used by Caddx PC Tool.
pub const POLL_INTERVAL: Duration = Duration::from_millis(200);

pub fn find_by_usb(vid: u16, pid: u16) -> Result<Option<PortInfo>, serialport::Error> {
    Ok(list_ports()?
        .into_iter()
        .find(|port| port.usb == Some((UsbId::new(vid), UsbId::new(pid)))))
}

/// Wait until a device with the given VID:PID reports a COM port.
pub fn wait_until_present(
    vid: u16,
    pid: u16,
    timeout: Duration,
) -> Result<Option<PortInfo>, serialport::Error> {
    wait_for(POLL_INTERVAL, timeout, || find_by_usb(vid, pid))
}

/// Wait until the device stops reporting a COM port, for example while it reboots.
pub fn wait_until_absent(vid: u16, pid: u16, timeout: Duration) -> Result<bool, serialport::Error> {
    let deadline = Instant::now() + timeout;
    loop {
        if find_by_usb(vid, pid)?.is_none() {
            return Ok(true);
        }
        if Instant::now() >= deadline {
            return Ok(false);
        }
        thread::sleep(POLL_INTERVAL);
    }
}

fn wait_for<T>(
    interval: Duration,
    timeout: Duration,
    mut probe: impl FnMut() -> Result<Option<T>, serialport::Error>,
) -> Result<Option<T>, serialport::Error> {
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(value) = probe()? {
            return Ok(Some(value));
        }
        if Instant::now() >= deadline {
            return Ok(None);
        }
        thread::sleep(interval);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn returns_the_first_matching_probe() {
        let mut calls = 0;
        let result = wait_for(Duration::from_millis(1), Duration::from_secs(1), || {
            calls += 1;
            Ok((calls == 3).then_some(calls))
        })
        .unwrap();
        assert_eq!(result, Some(3));
    }

    #[test]
    fn gives_up_after_the_timeout() {
        let result = wait_for::<u8>(Duration::from_millis(5), Duration::from_millis(20), || {
            Ok(None)
        })
        .unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn propagates_probe_errors_instead_of_looping() {
        wait_for::<u8>(Duration::from_millis(1), Duration::from_secs(1), || {
            Err(serialport::Error::new(
                serialport::ErrorKind::Unknown,
                "probe failed",
            ))
        })
        .unwrap_err();
    }

    #[test]
    fn reports_absence_immediately_when_probe_is_empty() {
        assert!(
            wait_until_absent(0xFFFF, 0xFFFF, Duration::from_millis(10)).unwrap(),
            "устройства с таким VID:PID не существует"
        );
    }
}
