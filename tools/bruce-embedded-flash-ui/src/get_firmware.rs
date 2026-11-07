//! GetFirmware, a component which finds firmware on GitHub and downloads it to the shared firmware folder, see
//! bruce-embedded-firmware: the repositories with firmware, each with its descriptions, its releases, to download, and
//! whether to keep it up to date, its newest release downloaded each time the component is shown.

use bruce_embedded_firmware::github::{self, Release, Repo, Settings};
use dioxus::prelude::*;

/// What a repository's releases are, once asked for.
#[derive(Clone, PartialEq)]
enum Releases {
    Loading,
    Loaded(Vec<(Release, bool)>),
    Failed(String),
}

/// Finds firmware on GitHub and downloads it, calling on_downloaded when it has, so that the page can read the
/// firmware again.
#[component]
pub fn GetFirmware(on_downloaded: EventHandler<()>) -> Element {
    let mut repos = use_signal(|| None::<Result<(Vec<Repo>, Vec<String>), String>>);
    let mut settings = use_signal(Settings::default);
    let mut releases = use_signal(std::collections::BTreeMap::<String, Releases>::new);
    let mut adding = use_signal(String::new);
    let mut status = use_signal(|| None::<Result<String, String>>);
    let mut busy = use_signal(|| false);

    let find = move || {
        spawn(async move {
            repos.set(None);
            let found = tokio::task::spawn_blocking(github::find).await;
            repos.set(Some(match found {
                Ok(Ok(found)) => Ok(found),
                Ok(Err(error)) => Err(format!("{error:#}")),
                Err(error) => Err(format!("{error}")),
            }));
        });
    };

    // Finds the repositories, and downloads the newest release of each kept up to date, when first shown.
    use_hook(move || {
        settings.set(Settings::load().unwrap_or_default());
        find();
        spawn(async move {
            if let Ok(Ok((downloaded, errors))) = tokio::task::spawn_blocking(github::update).await {
                if !downloaded.is_empty() {
                    status.set(Some(Ok(format!("Downloaded {} new firmware files", downloaded.len()))));
                    on_downloaded.call(());
                }
                if !errors.is_empty() {
                    status.set(Some(Err(format!("Unable to update {}", errors.join("; ")))));
                }
            }
        });
    });

    let show_releases = move |repo: Repo| {
        spawn(async move {
            let key = repo.full_name();
            releases.write().insert(key.clone(), Releases::Loading);
            let loaded = tokio::task::spawn_blocking(move || {
                let mut loaded = Vec::new();
                for release in github::releases(&repo)? {
                    let downloaded = github::is_downloaded(&repo, &release)?;
                    loaded.push((release, downloaded));
                }
                anyhow::Ok(loaded)
            })
            .await;
            releases.write().insert(key, match loaded {
                Ok(Ok(loaded)) => Releases::Loaded(loaded),
                Ok(Err(error)) => Releases::Failed(format!("{error:#}")),
                Err(error) => Releases::Failed(format!("{error}")),
            });
        });
    };

    let download = move |(repo, release): (Repo, Release)| {
        spawn(async move {
            busy.set(true);
            status.set(Some(Ok(format!("Downloading {} {}…", repo.name, release.version))));
            let (repo_copy, release_copy) = (repo.clone(), release.clone());
            let result = tokio::task::spawn_blocking(move || github::download(&repo_copy, &release_copy)).await;
            status.set(Some(match result {
                Ok(Ok(files)) => Ok(format!("Downloaded {} files of {} {}", files.len(), repo.name, release.version)),
                Ok(Err(error)) => Err(format!("{error:#}")),
                Err(error) => Err(format!("{error}")),
            }));
            busy.set(false);
            on_downloaded.call(());
            show_releases(repo);
        });
    };

    let add = move |_| {
        spawn(async move {
            let name = adding().trim().to_string();
            let checked = name.clone();
            match tokio::task::spawn_blocking(move || github::repo(&checked)).await {
                Ok(Ok(_)) => {
                    let mut changed = settings();
                    if !changed.added.contains(&name) {
                        changed.added.push(name);
                    }
                    match changed.save() {
                        Ok(()) => {
                            settings.set(changed);
                            adding.set(String::new());
                            find();
                        }
                        Err(error) => status.set(Some(Err(format!("{error:#}")))),
                    }
                }
                Ok(Err(error)) => status.set(Some(Err(format!("{name}: {error:#}")))),
                Err(error) => status.set(Some(Err(format!("{error}")))),
            }
        });
    };

    rsx! {
        div { class: "flasher",
            match repos() {
                None => rsx! { p { "Looking on GitHub…" } },
                Some(Err(error)) => rsx! { p { class: "flasher-error", "Unable to look on GitHub: {error}" } },
                Some(Ok((found, errors))) => rsx! {
                    if found.is_empty() {
                        p { "No repositories with firmware were found on GitHub." }
                    }
                    for repo in found.iter().cloned() {
                        {
                            let full_name = repo.full_name();
                            let kept = settings.read().is_kept_up_to_date(&full_name);
                            let shown = releases.read().get(&full_name).cloned();
                            let (keep_name, show_repo) = (full_name.clone(), repo.clone());
                            rsx! {
                                div { key: "{full_name}", class: "flasher-panel",
                                    p {
                                        b { "{repo.short}" }
                                        " "
                                        a { href: "{repo.url}", target: "_blank", "{full_name}" }
                                    }
                                    if !repo.long.is_empty() {
                                        p { class: "flasher-muted", "{repo.long}" }
                                    }
                                    label {
                                        input {
                                            r#type: "checkbox",
                                            checked: kept,
                                            onchange: move |event: FormEvent| {
                                                let mut changed = settings();
                                                changed.set_kept_up_to_date(&keep_name, event.checked());
                                                match changed.save() {
                                                    Ok(()) => settings.set(changed),
                                                    Err(error) => status.set(Some(Err(format!("{error:#}")))),
                                                }
                                            },
                                        }
                                        " Keep up to date: download its newest release each time this is shown"
                                    }
                                    match shown {
                                        None => rsx! {
                                            p { button { onclick: move |_| show_releases(show_repo.clone()), "Releases" } }
                                        },
                                        Some(Releases::Loading) => rsx! { p { "Reading its releases…" } },
                                        Some(Releases::Failed(error)) => rsx! { p { class: "flasher-error", "{error}" } },
                                        Some(Releases::Loaded(loaded)) => rsx! {
                                            if loaded.is_empty() {
                                                p { "It has no releases with firmware yet." }
                                            }
                                            table {
                                                for (release, downloaded) in loaded.iter().cloned() {
                                                    {
                                                        let download_repo = repo.clone();
                                                        let download_release = release.clone();
                                                        let published = release.published.get(..10).unwrap_or_default().to_string();
                                                        rsx! {
                                                            tr { key: "{release.tag}",
                                                                td { "{release.version}" }
                                                                td { class: "flasher-muted",
                                                                    "{published}"
                                                                    if release.prerelease {
                                                                        ", a prerelease"
                                                                    }
                                                                }
                                                                td { "{release.assets.len()} firmware files" }
                                                                td {
                                                                    if downloaded {
                                                                        "Downloaded"
                                                                    } else {
                                                                        button {
                                                                            disabled: busy(),
                                                                            onclick: move |_| download((download_repo.clone(), download_release.clone())),
                                                                            "Download"
                                                                        }
                                                                    }
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        },
                                    }
                                }
                            }
                        }
                    }
                    for error in errors.iter() {
                        p { class: "flasher-muted", "Left out {error}" }
                    }
                },
            }
            p {
                "Add a repository: "
                input {
                    r#type: "text",
                    placeholder: "owner/name",
                    value: "{adding}",
                    oninput: move |event| adding.set(event.value()),
                }
                " "
                button { disabled: adding().trim().is_empty(), onclick: add, "Add" }
                " "
                button { onclick: move |_| find(), "Look Again" }
            }
            match status() {
                Some(Ok(message)) => rsx! { p { class: "flasher-ok", "{message}" } },
                Some(Err(message)) => rsx! { p { class: "flasher-error", "{message}" } },
                None => rsx! {},
            }
        }
    }
}
