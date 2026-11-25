use std::{
    env,
    io::{self, BufRead, IsTerminal, Write},
    process::ExitCode,
};

use fpv_core::{console, devices, log::SessionLog, ports, usb};

const MENU_ITEMS: &str = "\
  1. Полётные контроллеры (STM32)\n\
  2. Модули управления\n\
  3. Walksnail\n\
  0. Выход\n";

#[derive(Debug, PartialEq, Eq)]
enum Action {
    Exit,
    FlightController,
    Radio,
    Walksnail,
    Unknown,
}

fn parse(line: &str) -> Action {
    match line.trim() {
        "0" => Action::Exit,
        "1" => Action::FlightController,
        "2" => Action::Radio,
        "3" => Action::Walksnail,
        _ => Action::Unknown,
    }
}

/// Colour is applied only for a real terminal, so redirected output and pipes stay plain.
struct Style {
    enabled: bool,
}

impl Style {
    fn paint(&self, code: &str, text: &str) -> String {
        if self.enabled {
            format!("\x1B[{code}m{text}\x1B[0m")
        } else {
            text.to_string()
        }
    }
}

/// Connected hardware: COM ports with a recognised type, plus DFU bootloaders.
fn describe_devices() -> Vec<String> {
    let mut lines = Vec::new();
    match ports::list_ports() {
        Ok(list) if list.is_empty() => lines.push("последовательных портов нет".to_string()),
        Ok(list) => lines.extend(
            list.iter()
                .map(|port| format!("  {}", devices::describe(port))),
        ),
        Err(error) => lines.push(format!("не удалось перечислить COM-порты ({error})")),
    }
    match usb::list_dfu_devices() {
        Ok(found) => lines.extend(
            found
                .iter()
                .map(|device| format!("  {}", device.display_name())),
        ),
        Err(error) => lines.push(format!("не удалось перечислить USB-устройства ({error})")),
    }
    lines
}

fn run_menu(
    mut input: impl BufRead,
    output: &mut impl Write,
    log: &mut SessionLog,
    is_terminal: bool,
    mut run_fc: impl FnMut() -> io::Result<()>,
) -> io::Result<()> {
    let style = Style {
        enabled: is_terminal,
    };
    let title = style.paint(
        "1;36",
        &format!(
            "==========   FPV-Flasher {}   ==========",
            env!("CARGO_PKG_VERSION")
        ),
    );
    let separator = style.paint("90", &"-".repeat(52));
    let heading = style.paint("1;37", "Устройства:");

    let mut line = String::new();
    loop {
        if is_terminal {
            output.write_all(console::CLEAR_SCREEN.as_bytes())?;
        }
        writeln!(output, "{title}")?;
        writeln!(output, "{separator}")?;
        writeln!(output, "{heading}")?;
        for entry in describe_devices() {
            writeln!(output, "{entry}")?;
        }
        write!(output, "\n{MENU_ITEMS}\n> ")?;
        output.flush()?;

        line.clear();
        if input.read_line(&mut line)? == 0 {
            return Ok(());
        }
        match parse(&line) {
            Action::Exit => return Ok(()),
            Action::FlightController => {
                log.record("Раздел 1: полётные контроллеры");
                if let Err(error) = run_fc() {
                    log.record_error(&format!("Полётные контроллеры: {error}"));
                }
            }
            Action::Radio => log.record(fpv_radio::STATUS),
            Action::Walksnail => log.record(fpv_walksnail::STATUS),
            Action::Unknown => log.record("Выберите пункт от 0 до 3."),
        }
    }
}

fn describe_ports_once(log: &mut SessionLog) {
    match ports::list_ports() {
        Ok(list) if list.is_empty() => log.record("Последовательные порты не найдены."),
        Ok(list) => {
            for port in &list {
                log.record(&devices::describe(port));
            }
        }
        Err(error) => log.record_error(&format!("Не удалось перечислить порты: {error}")),
    }
    match usb::list_dfu_devices() {
        Ok(found) if found.is_empty() => log.record("DFU-устройства не найдены."),
        Ok(found) => {
            for device in &found {
                log.record(&device.display_name());
            }
        }
        Err(error) => log.record_error(&format!("Не удалось перечислить USB: {error}")),
    }
}

fn main() -> ExitCode {
    let mut log = match env::current_exe() {
        Ok(exe) => exe.parent().map_or_else(
            || SessionLog::create(std::path::Path::new("fpv-flasher.log")),
            |dir| SessionLog::create(&dir.join("fpv-flasher.log")),
        ),
        Err(_) => SessionLog::create(std::path::Path::new("fpv-flasher.log")),
    };
    if env::args().skip(1).any(|argument| argument == "--ports") {
        describe_ports_once(&mut log);
        return ExitCode::SUCCESS;
    }

    console::enable_ansi();
    let interactive = io::stdin().is_terminal() && io::stdout().is_terminal();
    let result = run_menu(
        io::stdin().lock(),
        &mut io::stdout(),
        &mut log,
        interactive,
        || {
            if !interactive {
                return Err(io::Error::other(
                    "Для legacy-меню нужна интерактивная консоль.",
                ));
            }
            fpv_fc::run_legacy(&fpv_core::find_app_root(&env::current_exe()?)?)
        },
    );

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            log.record_error(&format!("Консоль: {error}"));
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temporary_log(tag: &str) -> (SessionLog, std::path::PathBuf) {
        let directory = std::env::temp_dir().join("fpv-app-log");
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join(format!("{}-{tag}.log", std::process::id()));
        let _ = std::fs::remove_file(&path);
        (SessionLog::create(&path), path)
    }

    #[test]
    fn maps_known_and_unknown_menu_entries() {
        assert_eq!(parse("0"), Action::Exit);
        assert_eq!(parse(" 1 \n"), Action::FlightController);
        assert_eq!(parse("2"), Action::Radio);
        assert_eq!(parse("3"), Action::Walksnail);
        assert_eq!(parse("42"), Action::Unknown);
        assert_eq!(parse("\n"), Action::Unknown);
    }

    #[test]
    fn dispatches_sections_and_recovers_from_legacy_errors() {
        let (mut log, path) = temporary_log("dispatch");
        let mut output = Vec::new();
        let mut launched = 0;
        run_menu(
            &b"bad\n1\n2\n3\n0\n1\n"[..],
            &mut output,
            &mut log,
            false,
            || {
                launched += 1;
                Err(io::Error::other("test failure"))
            },
        )
        .unwrap();
        assert_eq!(launched, 1);
        let screen = String::from_utf8(output).unwrap();
        let menu_count = screen.matches("FPV-Flasher").count();
        assert_eq!(menu_count, 5, "меню печатается до каждого чтения ввода");
        assert!(
            !screen.contains("\x1B[2J"),
            "неинтерактивный вывод не должен содержать управляющих последовательностей"
        );

        let written = std::fs::read_to_string(&path).unwrap();
        for expected in [
            "Выберите пункт от 0 до 3.",
            fpv_radio::STATUS,
            fpv_walksnail::STATUS,
            "ОШИБКА Полётные контроллеры: test failure",
        ] {
            assert!(written.contains(expected), "нет строки: {expected}");
        }
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn exits_on_eof_without_running_legacy() {
        let (mut log, path) = temporary_log("eof");
        run_menu(&b""[..], &mut io::sink(), &mut log, true, || {
            panic!("unexpected launch")
        })
        .unwrap();
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn clears_the_screen_only_for_a_terminal() {
        let (mut log, path) = temporary_log("clear");
        let mut output = Vec::new();
        run_menu(&b"0\n"[..], &mut output, &mut log, true, || Ok(())).unwrap();
        let screen = String::from_utf8(output).unwrap();
        assert!(screen.starts_with(console::CLEAR_SCREEN));
        std::fs::remove_file(&path).unwrap();
    }
}
