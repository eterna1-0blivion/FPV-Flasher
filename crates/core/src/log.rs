use std::{
    fs::{File, OpenOptions},
    io::Write,
    path::Path,
};

use chrono::Local;

/// Append-only session log next to the executable, keeping the console readable.
pub struct SessionLog {
    file: Option<File>,
}

impl SessionLog {
    /// Logging is best effort: a read-only folder must not stop flashing.
    pub fn create(path: &Path) -> Self {
        let file = OpenOptions::new().create(true).append(true).open(path).ok();
        Self { file }
    }

    pub fn record(&mut self, message: &str) {
        println!("{message}");
        if let Some(file) = self.file.as_mut() {
            let _ = writeln!(
                file,
                "{} {message}",
                Local::now().format("%Y-%m-%d %H:%M:%S")
            );
            let _ = file.flush();
        }
    }

    pub fn record_error(&mut self, message: &str) {
        eprintln!("{message}");
        if let Some(file) = self.file.as_mut() {
            let _ = writeln!(
                file,
                "{} ОШИБКА {message}",
                Local::now().format("%Y-%m-%d %H:%M:%S")
            );
            let _ = file.flush();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_timestamped_lines_to_file() {
        let directory = std::env::temp_dir().join(format!("fpv-log-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("session.log");

        let mut log = SessionLog::create(&path);
        log.record("строка с кириллицей");
        log.record_error("сбой");
        drop(log);

        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("строка с кириллицей"));
        assert!(text.contains("ОШИБКА сбой"));
        std::fs::remove_dir_all(&directory).unwrap();
    }

    #[test]
    fn keeps_working_when_the_file_cannot_be_created() {
        let missing = std::env::temp_dir()
            .join("fpv-missing-directory")
            .join("nested")
            .join("session.log");
        let mut log = SessionLog::create(&missing);
        log.record("no panic");
        assert!(!missing.exists());
    }
}
