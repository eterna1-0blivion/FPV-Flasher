/// Очистка экрана перед перерисовкой. Отправляется только в настоящий терминал.
pub const CLEAR_SCREEN: &str = "\x1B[2J\x1B[H";

/// Включает разбор ANSI-последовательностей в консоли Windows: по умолчанию он выключен,
/// и без этого цвета и очистка экрана печатались бы как мусорные символы.
#[cfg(windows)]
pub fn enable_ansi() {
    use windows_sys::Win32::System::Console::{
        ENABLE_VIRTUAL_TERMINAL_PROCESSING, GetConsoleMode, GetStdHandle, STD_OUTPUT_HANDLE,
        SetConsoleMode,
    };

    // БЕЗОПАСНОСТЬ: дескриптор получен от системы, мы его только читаем и меняем режим,
    // нигде не сохраняем и не освобождаем.
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
