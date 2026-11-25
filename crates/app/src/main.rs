use std::{
    env,
    io::{self, BufRead, IsTerminal, Write},
    process::ExitCode,
};

use fpv_core::{console, devices, log::SessionLog, ports};

const MENU_HEADER: &str = concat!("FPV-Flasher ", env!("CARGO_PKG_VERSION"));

const MENU_BODY: &str = "\n1. Полётные контроллеры (STM32)\n\
2. Модули управления\n\
3. Walksnail\n\
0. Выход\n\
> ";

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

/// Ports are inspected per redraw; a missing device must never stop the menu.
fn describe_devices() -> String {
    match ports::list_ports() {
        Ok(list) if list.is_empty() => {
            "Устройства: последовательных портов не найдено.".to_string()
        }
        Ok(list) => {
            let mut lines = vec!["Устройства:".to_string()];
            lines.extend(
                list.iter()
                    .map(|port| format!("  {}", devices::describe(port))),
            );
            lines.join("\n")
        }
        Err(error) => format!("Устройства: не удалось перечислить порты ({error})."),
    }
}

fn run_menu(
    mut input: impl BufRead,
    output: &mut impl Write,
    log: &mut SessionLog,
    is_terminal: bool,
    mut run_fc: impl FnMut() -> io::Result<()>,
) -> io::Result<()> {
    let mut line = String::new();
    loop {
        if is_terminal {
            output.write_all(console::CLEAR_SCREEN.as_bytes())?;
        }
        writeln!(output, "{MENU_HEADER}")?;
        output.write_all(describe_devices().as_bytes())?;
        output.write_all(MENU_BODY.as_bytes())?;
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

fn main() -> ExitCode {
    let mut log = match env::current_exe() {
        Ok(exe) => exe.parent().map_or_else(
            || SessionLog::create(std::path::Path::new("fpv-flasher.log")),
            |dir| SessionLog::create(&dir.join("fpv-flasher.log")),
        ),
        Err(_) => SessionLog::create(std::path::Path::new("fpv-flasher.log")),
    };
    if env::args().skip(1).any(|argument| argument == "--ports") {
        match ports::list_ports() {
            Ok(list) if list.is_empty() => log.record("Последовательные порты не найдены."),
            Ok(list) => {
                for port in &list {
                    log.record(&devices::describe(port));
                }
            }
            Err(error) => log.record_error(&format!("Не удалось перечислить порты: {error}")),
        }
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
