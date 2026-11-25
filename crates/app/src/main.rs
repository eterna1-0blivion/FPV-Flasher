use std::{
    env,
    io::{self, BufRead, Write},
    process::ExitCode,
};

const MENU: &str = concat!(
    "\nFPV-Flasher ",
    env!("CARGO_PKG_VERSION"),
    "\n",
    "1. Полётные контроллеры (STM32)\n",
    "2. Модули управления\n",
    "3. Walksnail\n",
    "0. Выход\n",
    "> ",
);

fn run_menu(
    mut input: impl BufRead,
    mut output: impl Write,
    mut run_fc: impl FnMut() -> io::Result<()>,
) -> io::Result<()> {
    let mut line = String::new();
    loop {
        output.write_all(MENU.as_bytes())?;
        output.flush()?;
        line.clear();
        if input.read_line(&mut line)? == 0 {
            return Ok(());
        }
        match line.trim() {
            "0" => return Ok(()),
            "1" => {
                if let Err(error) = run_fc() {
                    writeln!(output, "Ошибка: {error}")?;
                }
            }
            "2" => writeln!(output, "{}", fpv_radio::STATUS)?,
            "3" => writeln!(output, "{}", fpv_walksnail::STATUS)?,
            "" => (),
            _ => writeln!(output, "Выберите пункт от 0 до 3.")?,
        }
    }
}

fn main() -> ExitCode {
    let result = run_menu(io::stdin().lock(), io::stdout(), || {
        use std::io::IsTerminal;
        if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
            return Err(io::Error::other(
                "Для legacy-меню нужна интерактивная консоль.",
            ));
        }
        fpv_fc::run_legacy(&fpv_core::find_app_root(&env::current_exe()?)?)
    });
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("Ошибка консоли: {error}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dispatches_sections_and_recovers_from_legacy_errors() {
        let mut output = Vec::new();
        let mut launched = 0;
        run_menu(&b"bad\n1\n2\n3\n0\n1\n"[..], &mut output, || {
            launched += 1;
            Err(io::Error::other("test failure"))
        })
        .unwrap();
        assert_eq!(launched, 1);
        let output = String::from_utf8(output).unwrap();
        for expected in [
            "Выберите пункт",
            "Ошибка: test failure",
            fpv_radio::STATUS,
            fpv_walksnail::STATUS,
        ] {
            assert!(output.contains(expected));
        }
    }

    #[test]
    fn exits_on_eof_without_running_legacy() {
        run_menu(&b""[..], io::sink(), || panic!("unexpected launch")).unwrap();
    }
}
