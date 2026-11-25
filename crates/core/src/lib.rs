use std::{
    io,
    path::{Path, PathBuf},
};

pub mod console;
pub mod devices;
pub mod log;
pub mod ports;
pub mod usb;
pub mod wait;

/// Locate an unpacked app or a local Cargo build without relying on the working directory.
pub fn find_app_root(executable: &Path) -> io::Result<PathBuf> {
    let find = || {
        let parent = executable.parent()?;
        if parent.join("legacy/run.cmd").is_file() {
            return Some(parent);
        }
        let profile = parent.file_name()?.to_str()?;
        let target = parent.parent()?;
        if !matches!(profile, "debug" | "release") || target.file_name()? != "target" {
            return None;
        }
        let root = target.parent()?;
        root.join("legacy/run.cmd").is_file().then_some(root)
    };
    find().map(Path::to_path_buf).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "Не найден legacy\\run.cmd рядом с приложением. Распакуйте архив целиком.",
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_root_for_packaged_and_local_builds() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap();
        for executable in [
            "fpv-flasher.exe",
            "target/debug/fpv-flasher.exe",
            "target/release/fpv-flasher.exe",
        ] {
            assert_eq!(find_app_root(&root.join(executable)).unwrap(), root);
        }
    }

    #[test]
    fn does_not_search_arbitrary_ancestors() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap();
        assert_eq!(
            find_app_root(&root.join("crates/core/fpv-flasher.exe"))
                .unwrap_err()
                .kind(),
            io::ErrorKind::NotFound
        );
    }
}
