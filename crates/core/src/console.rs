/// Clear the visible screen before a redraw. Only emitted for a real terminal.
pub const CLEAR_SCREEN: &str = "\x1B[2J\x1B[H";

/// Ask the Windows console to interpret ANSI sequences; it is off by default.
#[cfg(windows)]
pub fn enable_ansi() {
    use windows_sys::Win32::System::Console::{
        ENABLE_VIRTUAL_TERMINAL_PROCESSING, GetConsoleMode, GetStdHandle, STD_OUTPUT_HANDLE,
        SetConsoleMode,
    };

    // SAFETY: the handle comes from the system, is only queried and set, and is not stored.
    unsafe {
        let handle = GetStdHandle(STD_OUTPUT_HANDLE);
        let mut mode = 0;
        if handle.is_null() || GetConsoleMode(handle, &mut mode) == 0 {
            return;
        }
        SetConsoleMode(handle, mode | ENABLE_VIRTUAL_TERMINAL_PROCESSING);
    }
}

#[cfg(not(windows))]
pub fn enable_ansi() {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clear_sequence_erases_and_homes() {
        assert_eq!(CLEAR_SCREEN, "\x1B[2J\x1B[H");
    }
}
