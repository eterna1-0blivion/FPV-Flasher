use std::{io, path::Path};

/// Use a fixed shell command relative to a trusted working directory, not a path interpolated into CMD.
#[cfg(windows)]
pub fn run_legacy(root: &Path) -> io::Result<()> {
    let status = std::process::Command::new("cmd.exe")
        .args(["/d", "/c", "legacy\\run.cmd"])
        .current_dir(root)
        .status()?;
    if status.success() {
        Ok(())
    } else {
        Err(io::Error::other(format!(
            "Меню полётных контроллеров завершилось с ошибкой: {status}"
        )))
    }
}

#[cfg(not(windows))]
pub fn run_legacy(_root: &Path) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "Раздел полётных контроллеров пока доступен только на Windows.",
    ))
}
