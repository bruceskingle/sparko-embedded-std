//! Finding firmware on GitHub and downloading it to the shared folder, see the crate's description.
//!
//! A repository with firmware has the topic TOPIC, which GitHub's search finds without a login, and DESCRIPTOR in its
//! root, whose first line, a heading, is its short description, e.g. "# Capture and Display Devices for
//! bruce-smart-home", and the rest its long description. Each release attaches an ELF file for each firmware, whose
//! firmware record says which board it is for. The user can also add a repository by its owner and name, and ask for
//! one to be kept up to date, its newest release downloaded whenever the firmware is updated, see update.

use std::path::{Path, PathBuf};

use anyhow::{Context, anyhow, bail};
use serde::{Deserialize, Serialize};

/// The GitHub topic of repositories with firmware.
pub const TOPIC: &str = "bruce-embedded-firmware";

/// The file in a repository's root which says it has firmware, and what it is.
pub const DESCRIPTOR: &str = "bruce-embedded-std.md";

/// The user agent GitHub's API requires.
const USER_AGENT: &str = concat!("bruce-embedded-firmware/", env!("CARGO_PKG_VERSION"));

/// A repository with firmware.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Repo {
    pub owner: String,
    pub name: String,
    /// What its firmware is, from its DESCRIPTOR, e.g. "Capture and Display Devices for bruce-smart-home".
    pub short: String,
    /// More about it, Markdown.
    pub long: String,
    /// Its page on GitHub.
    pub url: String,
}

impl Repo {
    /// e.g. bruceskingle/bruce-smart-home.
    pub fn full_name(&self) -> String {
        format!("{}/{}", self.owner, self.name)
    }
}

/// A release of a repository, with its firmware files.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Release {
    pub tag: String,
    /// The tag from its first digit, e.g. 0.5.0 from release0.5.0 or v0.5.0, the name of its version folder.
    pub version: String,
    pub name: String,
    /// When it was published, as GitHub says it, e.g. 2026-11-07T10:00:00Z.
    pub published: String,
    pub prerelease: bool,
    pub assets: Vec<Asset>,
}

/// A firmware file attached to a release.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Asset {
    pub name: String,
    pub url: String,
    pub size: u64,
}

/// The repositories the user has added, by owner/name, and those to keep up to date, kept beside the shared folder.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub added: Vec<String>,
    pub keep_up_to_date: Vec<String>,
}

impl Settings {
    fn file() -> anyhow::Result<PathBuf> {
        Ok(crate::folder()?.with_file_name("firmware-repos.json"))
    }

    /// The settings, or the defaults if there are none yet.
    pub fn load() -> anyhow::Result<Settings> {
        match std::fs::read(Self::file()?) {
            Ok(bytes) => Ok(serde_json::from_slice(&bytes).context("Unable to read the firmware repositories")?),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Settings::default()),
            Err(error) => Err(error).context("Unable to read the firmware repositories"),
        }
    }

    pub fn save(&self) -> anyhow::Result<()> {
        let file = Self::file()?;
        if let Some(folder) = file.parent() {
            std::fs::create_dir_all(folder)?;
        }
        write_atomically(&file, &serde_json::to_vec_pretty(self)?)
    }

    /// Whether the repository, by owner/name, is kept up to date.
    pub fn is_kept_up_to_date(&self, full_name: &str) -> bool {
        self.keep_up_to_date.iter().any(|kept| kept == full_name)
    }

    pub fn set_kept_up_to_date(&mut self, full_name: &str, keep: bool) {
        self.keep_up_to_date.retain(|kept| kept != full_name);
        if keep {
            self.keep_up_to_date.push(full_name.to_string());
        }
    }
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder().user_agent(USER_AGENT).build().into()
}

fn get_json<T: serde::de::DeserializeOwned>(url: &str) -> anyhow::Result<T> {
    agent()
        .get(url)
        .header("Accept", "application/vnd.github+json")
        .call()
        .with_context(|| format!("Unable to fetch {url}"))?
        .body_mut()
        .read_json()
        .with_context(|| format!("Unable to read {url}"))
}

/// The repositories with firmware: those with TOPIC, and those the user added, each with its DESCRIPTOR. A
/// repository without one is left out, and why is in the errors, as are those which could not be read.
pub fn find() -> anyhow::Result<(Vec<Repo>, Vec<String>)> {
    #[derive(Deserialize)]
    struct Search {
        items: Vec<Item>,
    }
    #[derive(Deserialize)]
    struct Item {
        full_name: String,
    }
    let search: Search =
        get_json(&format!("https://api.github.com/search/repositories?q=topic:{TOPIC}&per_page=100"))?;
    let mut names: Vec<String> = search.items.into_iter().map(|item| item.full_name).collect();
    for added in Settings::load()?.added {
        if !names.contains(&added) {
            names.push(added);
        }
    }
    let (mut repos, mut errors) = (Vec::new(), Vec::new());
    for name in names {
        match repo(&name) {
            Ok(repo) => repos.push(repo),
            Err(error) => errors.push(format!("{name}: {error:#}")),
        }
    }
    Ok((repos, errors))
}

/// The repository, by owner/name, with its DESCRIPTOR.
pub fn repo(full_name: &str) -> anyhow::Result<Repo> {
    let (owner, name) = full_name
        .split_once('/')
        .ok_or_else(|| anyhow!("{full_name} is not a repository's owner/name"))?;
    let url = format!("https://raw.githubusercontent.com/{owner}/{name}/HEAD/{DESCRIPTOR}");
    let text = agent()
        .get(&url)
        .call()
        .map_err(|error| anyhow!("It has no {DESCRIPTOR}, so is not one with firmware ({error})"))?
        .body_mut()
        .read_to_string()
        .with_context(|| format!("Unable to read {url}"))?;
    let (short, long) = parse_descriptor(&text);
    Ok(Repo {
        owner: owner.to_string(),
        name: name.to_string(),
        short,
        long,
        url: format!("https://github.com/{owner}/{name}"),
    })
}

/// The short and long descriptions in a DESCRIPTOR: its first line, without the heading's #, and the rest.
pub fn parse_descriptor(text: &str) -> (String, String) {
    let text = text.trim_start();
    let (first, rest) = text.split_once('\n').unwrap_or((text, ""));
    (first.trim_start_matches('#').trim().to_string(), rest.trim().to_string())
}

/// The repository's releases, newest first, each with its ELF files, leaving out drafts and those with none.
pub fn releases(repo: &Repo) -> anyhow::Result<Vec<Release>> {
    #[derive(Deserialize)]
    struct GitHubRelease {
        tag_name: String,
        name: Option<String>,
        published_at: Option<String>,
        draft: bool,
        prerelease: bool,
        assets: Vec<GitHubAsset>,
    }
    #[derive(Deserialize)]
    struct GitHubAsset {
        name: String,
        browser_download_url: String,
        size: u64,
    }
    let found: Vec<GitHubRelease> =
        get_json(&format!("https://api.github.com/repos/{}/releases?per_page=30", repo.full_name()))?;
    Ok(found
        .into_iter()
        .filter(|release| !release.draft)
        .map(|release| Release {
            version: release.tag_name.trim_start_matches(|c: char| !c.is_ascii_digit()).to_string(),
            name: release.name.unwrap_or_else(|| release.tag_name.clone()),
            tag: release.tag_name,
            published: release.published_at.unwrap_or_default(),
            prerelease: release.prerelease,
            assets: release
                .assets
                .into_iter()
                .filter(|asset| asset.name.ends_with(".elf"))
                .map(|asset| Asset { name: asset.name, url: asset.browser_download_url, size: asset.size })
                .collect(),
        })
        .filter(|release| !release.assets.is_empty())
        .collect())
}

/// The newest release which is not a prerelease, from releases newest first.
pub fn newest(releases: &[Release]) -> Option<&Release> {
    releases.iter().find(|release| !release.prerelease)
}

/// The version folder a release of the repository goes in.
pub fn release_folder(repo: &Repo, release: &Release) -> anyhow::Result<PathBuf> {
    Ok(crate::folder()?.join(&repo.name).join(&release.version))
}

/// Whether the release's files are all in the shared folder.
pub fn is_downloaded(repo: &Repo, release: &Release) -> anyhow::Result<bool> {
    let folder = release_folder(repo, release)?;
    Ok(release.assets.iter().all(|asset| {
        std::fs::metadata(folder.join(&asset.name)).is_ok_and(|metadata| metadata.len() == asset.size)
    }))
}

/// Downloads the release's files to its version folder, those not already there, and returns those downloaded.
pub fn download(repo: &Repo, release: &Release) -> anyhow::Result<Vec<PathBuf>> {
    let folder = release_folder(repo, release)?;
    std::fs::create_dir_all(&folder).with_context(|| format!("Unable to make {}", folder.display()))?;
    let mut downloaded = Vec::new();
    for asset in &release.assets {
        let path = folder.join(&asset.name);
        if std::fs::metadata(&path).is_ok_and(|metadata| metadata.len() == asset.size) {
            continue;
        }
        let bytes = agent()
            .get(&asset.url)
            .call()
            .with_context(|| format!("Unable to download {}", asset.name))?
            .body_mut()
            .with_config()
            .limit(64 * 1024 * 1024)
            .read_to_vec()
            .with_context(|| format!("Unable to download {}", asset.name))?;
        if bytes.len() as u64 != asset.size {
            bail!("{} was {} bytes, not {}", asset.name, bytes.len(), asset.size);
        }
        write_atomically(&path, &bytes)?;
        downloaded.push(path);
    }
    Ok(downloaded)
}

/// Downloads the newest release of each repository the user keeps up to date, and returns the files downloaded, and
/// why any repository could not be.
pub fn update() -> anyhow::Result<(Vec<PathBuf>, Vec<String>)> {
    let (mut downloaded, mut errors) = (Vec::new(), Vec::new());
    for full_name in Settings::load()?.keep_up_to_date {
        let result = repo(&full_name).and_then(|repo| {
            let releases = releases(&repo)?;
            match newest(&releases) {
                Some(release) => download(&repo, release),
                None => Ok(Vec::new()),
            }
        });
        match result {
            Ok(files) => downloaded.extend(files),
            Err(error) => errors.push(format!("{full_name}: {error:#}")),
        }
    }
    Ok((downloaded, errors))
}

/// Writes the file beside it and moves it over it, so that it is never left half written.
fn write_atomically(path: &Path, bytes: &[u8]) -> anyhow::Result<()> {
    let partial = path.with_extension("part");
    std::fs::write(&partial, bytes).with_context(|| format!("Unable to write {}", partial.display()))?;
    std::fs::rename(&partial, path).with_context(|| format!("Unable to write {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn descriptor() {
        let (short, long) = parse_descriptor("# Capture and Display Devices for bruce-smart-home\n\nThey poll.\n");
        assert_eq!(short, "Capture and Display Devices for bruce-smart-home");
        assert_eq!(long, "They poll.");
        assert_eq!(parse_descriptor("# Only a heading"), ("Only a heading".to_string(), String::new()));
    }
}
