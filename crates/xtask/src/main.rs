use std::{
    fs::{self, File},
    io,
    path::Path,
    process::Command,
};

use sha2::{Digest, Sha256};
use zip::{ZipWriter, write::SimpleFileOptions};

fn main() {
    if let Err(error) = run() {
        eprintln!("Ошибка сборки архива: {error}");
        std::process::exit(1);
    }
}

fn context<T, E: std::fmt::Display>(
    result: Result<T, E>,
    step: &str,
) -> Result<T, Box<dyn std::error::Error>> {
    result.map_err(|error| format!("{step}: {error}").into())
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .ok_or("не найден корень проекта")?;
    let manifest = context(
        fs::read_to_string(root.join("vendor.sha256")),
        "чтение vendor.sha256",
    )?;
    let entries = parse_manifest(&manifest)?;
    context(verify(root, &entries), "проверка vendor")?;

    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    if !Path::new(&cargo).is_file() && std::env::var_os("CARGO").is_none() {
        return Err(format!("cargo не найден: {cargo}").into());
    }
    let status = context(
        Command::new(&cargo)
            .args([
                "build",
                "--release",
                "--locked",
                "--offline",
                "-p",
                "fpv-flasher",
            ])
            .current_dir(root)
            .status(),
        "запуск cargo build",
    )?;
    if !status.success() {
        return Err("cargo build завершился с ошибкой".into());
    }

    let version = env!("CARGO_PKG_VERSION");
    let dist = root.join("dist");
    fs::create_dir_all(&dist)?;
    let archive_path = dist.join(format!("FPV-Flasher-{version}.zip"));
    let file = File::create(&archive_path)?;
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    let exe = root.join("target/release/fpv-flasher.exe");
    add_file(&mut zip, options, &exe, "fpv-flasher.exe")?;
    for path in [
        "legacy",
        "docs",
        "README.md",
        "LICENSE",
        "THIRD-PARTY-NOTICES.md",
    ] {
        add_tree(&mut zip, options, root, &root.join(path))?;
    }
    for entry in &entries {
        add_file(
            &mut zip,
            options,
            &root.join("vendor").join(&entry.path),
            &format!("vendor/{}", entry.path),
        )?;
    }
    zip.finish()?;
    println!(
        "Архив: {} ({})",
        archive_path.display(),
        human_size(fs::metadata(&archive_path)?.len())
    );
    Ok(())
}

struct Entry {
    hash: String,
    path: String,
}

/// Разбирает вывод `sha256sum`: 64 hex-символа, два пробела, относительный путь.
fn parse_manifest(text: &str) -> Result<Vec<Entry>, Box<dyn std::error::Error>> {
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            let (hash, path) = line
                .split_once("  ")
                .ok_or_else(|| format!("строка манифеста без разделителя: {line}"))?;
            if hash.len() != 64 || !hash.chars().all(|c| c.is_ascii_hexdigit()) {
                return Err(format!("некорректный хэш в манифесте: {line}").into());
            }
            if !is_relative_path(path) {
                return Err(format!("недопустимый путь в манифесте: {line}").into());
            }
            Ok(Entry {
                hash: hash.to_ascii_lowercase(),
                path: path.to_string(),
            })
        })
        .collect()
}

/// Отсекает абсолютные пути и компоненты `..` до обращения к файлам.
fn is_relative_path(path: &str) -> bool {
    !path.is_empty()
        && !path.starts_with(['/', '\\'])
        && !path.contains(':')
        && !Path::new(path).is_absolute()
        && !path.split(['/', '\\']).any(|part| part == "..")
}

fn verify(root: &Path, entries: &[Entry]) -> Result<(), Box<dyn std::error::Error>> {
    if entries.is_empty() {
        return Err("манифест vendor.sha256 пуст".into());
    }
    for entry in entries {
        let path = root.join("vendor").join(&entry.path);
        let actual = hex(&Sha256::digest(fs::read(&path)?));
        if actual != entry.hash {
            return Err(format!(
                "контрольная сумма не совпадает: {}\nожидалось {}\nполучено  {actual}",
                entry.path, entry.hash
            )
            .into());
        }
    }
    Ok(())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn add_file(
    zip: &mut ZipWriter<File>,
    options: SimpleFileOptions,
    source: &Path,
    name: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    zip.start_file(name.replace('\\', "/"), options)?;
    let mut file = File::open(source)
        .map_err(|error| io::Error::new(error.kind(), format!("{}: {error}", source.display())))?;
    io::copy(&mut file, zip)?;
    Ok(())
}

fn add_tree(
    zip: &mut ZipWriter<File>,
    options: SimpleFileOptions,
    root: &Path,
    directory: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    if directory.is_file() {
        let name = directory.strip_prefix(root)?.to_string_lossy().to_string();
        return add_file(zip, options, directory, &name);
    }
    let mut children = fs::read_dir(directory)?.collect::<Result<Vec<_>, _>>()?;
    children.sort_by_key(|child| child.file_name());
    for child in children {
        add_tree(zip, options, root, &child.path())?;
    }
    Ok(())
}

fn human_size(bytes: u64) -> String {
    format!("{:.1} МиБ", bytes as f64 / (1024.0 * 1024.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_well_formed_manifest_lines() {
        let text = format!(
            "{}  STM32CubeCLT/bin/a.exe\n\n{}  b/c.d\n",
            "a".repeat(64),
            "B".repeat(64)
        );
        let entries = parse_manifest(&text).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[1].path, "b/c.d");
        assert_eq!(entries[1].hash, "b".repeat(64));
    }

    #[test]
    fn rejects_traversal_and_absolute_paths() {
        let hash = "a".repeat(64);
        for path in [
            "../evil.exe",
            "..\\evil.exe",
            "a/../../evil.exe",
            "/windows/system32/evil.exe",
            "\\windows\\evil.exe",
            "C:/windows/evil.exe",
            "C:\\windows\\evil.exe",
            "",
        ] {
            assert!(
                parse_manifest(&format!("{hash}  {path}\n")).is_err(),
                "путь принят: {path}"
            );
        }
    }

    #[test]
    fn accepts_relative_manifest_paths() {
        let hash = "a".repeat(64);
        for path in ["a.exe", "STM32CubeCLT/bin/a.exe", "dir/with space/file.inf"] {
            assert!(
                parse_manifest(&format!("{hash}  {path}\n")).is_ok(),
                "{path}"
            );
        }
    }
}
