//! The firmware for boards built on the platform, as kept on a computer, so that every app which flashes boards,
//! e.g. Marco Sparko's Devices page and bruce-embedded-flash's window, finds the same firmware.
//!
//! It is kept in one folder, see folder, by the GitHub repository it comes from, e.g. `bruce-smart-home` for the
//! capture devices' firmware and `bruce-embedded-std` for the platform's demos, then by version, e.g. `0.5.0`, or
//! `dev` for what the repository's dev build script built, then a file for each firmware, its ELF file, whose
//! firmware record says which board it is for. Every version is kept, so that a bad release can be rolled back.
//!
//! ```text
//! <data dir>/bruce-embedded-std/firmware/bruce-smart-home/0.5.0/marco-sparko-capture-cyd-esp32-28.elf
//! ```

use std::path::{Path, PathBuf};

use anyhow::{Context, anyhow};

pub mod github;

/// The environment variable which, if set, names the folder instead, e.g. for tests.
pub const FOLDER_VARIABLE: &str = "BRUCE_EMBEDDED_FIRMWARE";

/// The version folder of what a repository's dev build script built.
pub const DEV_VERSION: &str = "dev";

/// The folder firmware is kept in: FOLDER_VARIABLE, or bruce-embedded-std/firmware in the user's data directory,
/// e.g. ~/Library/Application Support on macOS, ~/.local/share on Linux, and the roaming AppData on Windows.
pub fn folder() -> anyhow::Result<PathBuf> {
    if let Some(folder) = std::env::var_os(FOLDER_VARIABLE) {
        return Ok(PathBuf::from(folder));
    }
    let data = dirs::data_dir().ok_or_else(|| anyhow!("This computer has no data directory to keep firmware in"))?;
    Ok(data.join("bruce-embedded-std").join("firmware"))
}

/// The file a dev build of the firmware is kept in, for the repository, e.g. bruce-smart-home: the firmware's name,
/// with its board's if it does not have it already, as the dev build scripts name them, e.g.
/// bruce-smart-home/dev/marco-sparko-capture-cyd-esp32-28.elf.
pub fn dev_file(repo: &str, firmware: &str, board: &str) -> anyhow::Result<PathBuf> {
    let name = if firmware.contains(board) { format!("{firmware}.elf") } else { format!("{firmware}-{board}.elf") };
    Ok(folder()?.join(repo).join(DEV_VERSION).join(name))
}

/// Copies an ELF file of firmware into the repository's dev version, as dev_file names it, and returns where.
pub fn keep_dev(repo: &str, elf: &Path, firmware: &str, board: &str) -> anyhow::Result<PathBuf> {
    let file = dev_file(repo, firmware, board)?;
    if let Some(folder) = file.parent() {
        std::fs::create_dir_all(folder).with_context(|| format!("Unable to make {}", folder.display()))?;
    }
    std::fs::copy(elf, &file).with_context(|| format!("Unable to copy {} to {}", elf.display(), file.display()))?;
    Ok(file)
}

/// An ELF file of firmware in the folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalFirmware {
    /// The repository it comes from, e.g. `bruce-smart-home`.
    pub repo: String,
    /// Its release's version, e.g. `0.5.0`, or DEV_VERSION.
    pub version: String,
    pub path: PathBuf,
}

impl LocalFirmware {
    /// Where it came from, for the user, e.g. "bruce-smart-home 0.5.0".
    pub fn label(&self) -> String {
        format!("{} {}", self.repo, self.version)
    }
}

/// The firmware in the folder, by repository, then version, then file, or none if there is no folder yet.
pub fn local() -> anyhow::Result<Vec<LocalFirmware>> {
    local_in(&folder()?)
}

/// The firmware in folder, see local.
pub fn local_in(folder: &Path) -> anyhow::Result<Vec<LocalFirmware>> {
    let mut found = Vec::new();
    if !folder.exists() {
        return Ok(found);
    }
    for repo in subfolders(folder)? {
        for version in subfolders(&repo)? {
            let entries = std::fs::read_dir(&version).with_context(|| format!("Unable to list {}", version.display()))?;
            let mut files: Vec<PathBuf> = entries
                .filter_map(|entry| entry.ok())
                .map(|entry| entry.path())
                .filter(|path| path.extension().is_some_and(|extension| extension == "elf"))
                .collect();
            files.sort();
            for path in files {
                found.push(LocalFirmware { repo: name(&repo), version: name(&version), path });
            }
        }
    }
    Ok(found)
}

/// The folders in the folder, sorted.
fn subfolders(folder: &Path) -> anyhow::Result<Vec<PathBuf>> {
    let entries = std::fs::read_dir(folder).with_context(|| format!("Unable to list {}", folder.display()))?;
    let mut folders: Vec<PathBuf> =
        entries.filter_map(|entry| entry.ok()).map(|entry| entry.path()).filter(|path| path.is_dir()).collect();
    folders.sort();
    Ok(folders)
}

/// The last part of a path, e.g. a folder's name.
fn name(path: &Path) -> String {
    path.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_by_repo_and_version() {
        let folder = std::env::temp_dir().join(format!("bruce-embedded-firmware-test-{}", std::process::id()));
        let dev = folder.join("bruce-smart-home").join(DEV_VERSION);
        std::fs::create_dir_all(&dev).unwrap();
        std::fs::create_dir_all(folder.join("bruce-embedded-std").join("0.1.0")).unwrap();
        std::fs::write(dev.join("marco-sparko-capture-cyd-esp32-28.elf"), b"").unwrap();
        std::fs::write(dev.join("notes.txt"), b"").unwrap();
        let found = local_in(&folder).unwrap();
        std::fs::remove_dir_all(&folder).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].label(), "bruce-smart-home dev");
        assert!(local_in(&folder).unwrap().is_empty());
    }
}
