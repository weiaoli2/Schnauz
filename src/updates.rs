//! Opt-in, read-only GitHub release discovery. Never replaces or restarts the app.
use super::*;
use semver::Version;

const REPOSITORY: &str = "https://github.com/weiaoli2/Schnauz";
const ENDPOINT: &str = "https://api.github.com/repos/weiaoli2/Schnauz/releases?per_page=100";
const MAX_RESPONSE: usize = 512 * 1024;

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Release {
    pub version: Version,
    pub notes: String,
    pub download: String,
    pub prerelease: bool,
}
#[derive(Clone, Default)]
pub(super) struct State {
    pub busy: bool,
    pub checked: bool,
    pub release: Option<Release>,
    pub ahead_of: Option<Version>,
    pub error: Option<String>,
}
impl gpui::Global for State {}
impl State {
    fn label(&self) -> String {
        if self.busy {
            "Checking GitHub…".into()
        } else if let Some(error) = &self.error {
            error.clone()
        } else if let Some(release) = &self.release {
            format!(
                "Schnauz {} available{}",
                release.version,
                if release.prerelease {
                    " · Prerelease"
                } else {
                    ""
                }
            )
        } else if let Some(latest) = &self.ahead_of {
            format!(
                "Your installed version {} is newer than the latest GitHub release {latest}.",
                env!("CARGO_PKG_VERSION")
            )
        } else if self.checked {
            "No newer release available.".into()
        } else {
            "Not checked yet.".into()
        }
    }
}

#[derive(serde::Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
    state: String,
}
#[derive(serde::Deserialize)]
struct Published {
    tag_name: String,
    html_url: String,
    draft: bool,
    prerelease: bool,
    published_at: Option<String>,
    assets: Vec<Asset>,
}

#[derive(Debug)]
struct Comparison {
    update: Option<Release>,
    ahead_of: Option<Version>,
}

fn select(bytes: &[u8], current: &str, architecture: &str) -> Result<Comparison, String> {
    if bytes.len() > MAX_RESPONSE {
        return Err("Update response was too large. Try again later.".into());
    }
    let current = Version::parse(current).map_err(|_| "Cannot compare this app version.")?;
    let releases: Vec<Published> = serde_json::from_slice(bytes)
        .map_err(|_| "GitHub returned an invalid release list. Try again later.")?;
    let latest = releases
        .into_iter()
        .filter_map(|release| {
            if release.draft || release.published_at.is_none() {
                return None;
            }
            let version = Version::parse(
                release
                    .tag_name
                    .strip_prefix('v')
                    .unwrap_or(&release.tag_name),
            )
            .ok()?;
            let notes = format!("{REPOSITORY}/releases/tag/{}", release.tag_name);
            if release.html_url != notes {
                return None;
            }
            let name = format!("Schnauz-{version}-macos-{architecture}.dmg");
            let download = format!("{REPOSITORY}/releases/download/{}/{name}", release.tag_name);
            release
                .assets
                .iter()
                .any(|a| {
                    a.name == name && a.state == "uploaded" && a.browser_download_url == download
                })
                .then_some(Release {
                    version,
                    notes,
                    download,
                    prerelease: release.prerelease,
                })
        })
        .max_by(|a, b| a.version.cmp_precedence(&b.version));
    let ahead_of = latest
        .as_ref()
        .filter(|release| current.cmp_precedence(&release.version).is_gt())
        .map(|release| release.version.clone());
    let update = latest.filter(|release| release.version.cmp_precedence(&current).is_gt());
    Ok(Comparison { update, ahead_of })
}

fn fetch() -> Result<Comparison, String> {
    use std::io::Read;
    // Ignore personal curl configuration. Fixed public HTTPS endpoint, no token,
    // redirects, cookies, local file URLs or data from the user's app profile.
    let mut child = Command::new("/usr/bin/curl")
        .args([
            "--disable",
            "--fail",
            "--silent",
            "--proto",
            "=https",
            "--connect-timeout",
            "3",
            "--max-time",
            "8",
            "--header",
            "Accept: application/vnd.github+json",
            "--header",
            "X-GitHub-Api-Version: 2022-11-28",
            "--user-agent",
            concat!("Schnauz/", env!("CARGO_PKG_VERSION")),
            ENDPOINT,
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| "Couldn't start the update check. Try again later.")?;
    let mut bytes = Vec::new();
    let read = child
        .stdout
        .take()
        .ok_or("Couldn't read the update response.")?
        .take(MAX_RESPONSE as u64 + 1)
        .read_to_end(&mut bytes);
    if read.is_err() || bytes.len() > MAX_RESPONSE {
        let _ = child.kill();
        let _ = child.wait();
        return Err("Couldn't read the update response. Try again later.".into());
    }
    if !child.wait().is_ok_and(|status| status.success()) {
        return Err("Couldn't reach GitHub. Check your connection and try again later.".into());
    }
    let architecture = if cfg!(target_arch = "aarch64") {
        "arm64"
    } else {
        "x86_64"
    };
    select(&bytes, env!("CARGO_PKG_VERSION"), architecture)
}

pub(super) fn check(cx: &mut App) {
    let mut state = cx.try_global::<State>().cloned().unwrap_or_default();
    if state.busy {
        return;
    }
    state.busy = true;
    state.error = None;
    cx.set_global(state);
    // Network waits use a dedicated worker, never the shared file-work pool.
    let (send, receive) = futures::channel::oneshot::channel();
    if std::thread::Builder::new()
        .name("schnauz-update-check".into())
        .spawn(move || {
            let _ = send.send(fetch());
        })
        .is_err()
    {
        let mut state = cx.global::<State>().clone();
        state.busy = false;
        state.error = Some("Couldn't start the update check.".into());
        cx.set_global(state);
        return;
    }
    cx.spawn(async move |cx| {
        let result = receive
            .await
            .unwrap_or_else(|_| Err("The update check was interrupted. Try again later.".into()));
        let _ = cx.update(|cx| {
            let mut state = cx.global::<State>().clone();
            state.busy = false;
            state.checked = true;
            match result {
                Ok(comparison) => {
                    state.release = comparison.update;
                    state.ahead_of = comparison.ahead_of;
                    state.error = None;
                }
                Err(error) => state.error = Some(error),
            }
            cx.set_global(state);
        });
    })
    .detach();
}

pub(super) fn badge(cx: &App) -> Option<(u32, &'static str)> {
    let state = cx.try_global::<State>()?;
    if state.release.is_some() {
        Some((0x3478c5, "Update available"))
    } else if state.ahead_of.is_some() {
        Some((
            0xb77900,
            "Installed version is newer than the latest GitHub release",
        ))
    } else {
        None
    }
}

pub(super) fn controls<T: 'static>(width: f32, cx: &Context<T>) -> AnyElement {
    let state = cx.try_global::<State>().cloned().unwrap_or_default();
    let label = state.label();
    let t = theme();
    let width = width.max(240.);
    let enabled = prefs().check_updates_at_startup;
    let mut buttons = div().w(px(width)).flex().flex_wrap().gap_2().child(
        div()
            .id("check-updates")
            .px_2()
            .py_1()
            .rounded_md()
            .text_color(rgb(t.accent))
            .cursor_pointer()
            .hover(|d| d.bg(rgb(t.hover)))
            .child(if state.busy {
                "Checking…"
            } else {
                "Check for Updates…"
            })
            .on_click(|_, _, cx| check(cx)),
    );
    if let Some(release) = state.release {
        buttons = buttons.child(
            div()
                .id("update-notes")
                .px_2()
                .py_1()
                .rounded_md()
                .cursor_pointer()
                .text_color(rgb(t.accent))
                .child("Release Notes")
                .on_click(move |_, _, cx| cx.open_url(&release.notes)),
        );
        buttons = buttons.child(
            div()
                .id("update-download")
                .px_2()
                .py_1()
                .rounded_md()
                .cursor_pointer()
                .text_color(rgb(t.accent))
                .child(if cfg!(target_arch = "aarch64") {
                    "Download for Apple Silicon"
                } else {
                    "Download for Intel"
                })
                .on_click(move |_, _, cx| cx.open_url(&release.download)),
        );
    }
    div()
        .w(px(width))
        .relative()
        .children(ui_checks::probe("update-controls"))
        .flex_none()
        .flex()
        .flex_col()
        .gap_2()
        .text_size(px(UI_FONT_SIZE))
        .child(
            div()
                .w(px(width))
                .font_weight(FontWeight::SEMIBOLD)
                .child("Updates"),
        )
        .child(
            div()
                .id("startup-updates")
                .w(px(width))
                .flex()
                .items_center()
                .gap_2()
                .cursor_pointer()
                .child(
                    div()
                        .flex_none()
                        .w(px(14.))
                        .h(px(14.))
                        .border_1()
                        .border_color(rgb(t.accent))
                        .rounded_sm()
                        .when(enabled, |d| {
                            d.child(ui_icon(Icon::Check, 12., rgb(t.accent)))
                        }),
                )
                .child(
                    div()
                        .w(px(width - 22.))
                        .child("Automatically check for updates at startup"),
                )
                .on_click(cx.listener(|_, _, _, cx| {
                    let mut p = prefs();
                    p.check_updates_at_startup = !p.check_updates_at_startup;
                    apply_prefs(p, cx);
                    cx.notify();
                })),
        )
        .child(
            div()
                .w(px(width))
                .flex()
                .text_size(px(UI_SMALL_FONT_SIZE))
                .text_color(rgb(t.text_muted))
                .child(ui_checks::text(
                    "update-source",
                    "Updates are checked via GitHub.".into(),
                )),
        )
        .child(
            div()
                .w(px(width))
                .text_size(px(UI_SMALL_FONT_SIZE))
                .child(ui_checks::text("update-status", label.into())),
        )
        .child(buttons)
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn release(version: &str, prerelease: bool, architecture: &str) -> serde_json::Value {
        let tag = format!("v{version}");
        let name = format!("Schnauz-{version}-macos-{architecture}.dmg");
        serde_json::json!({"tag_name":tag,"html_url":format!("{REPOSITORY}/releases/tag/{tag}"),"draft":false,"prerelease":prerelease,"published_at":"2026-10-19T00:00:00Z","assets":[{"name":name,"state":"uploaded","browser_download_url":format!("{REPOSITORY}/releases/download/{tag}/{name}")}]})
    }
    fn choose(values: Vec<serde_json::Value>, current: &str, arch: &str) -> Option<Release> {
        select(&serde_json::to_vec(&values).unwrap(), current, arch)
            .unwrap()
            .update
    }
    #[test]
    fn semantic_order_includes_public_previews_and_never_downgrades() {
        let result = choose(
            vec![
                release("0.5.9", false, "arm64"),
                release("0.5.12", true, "arm64"),
                release("0.5.10", false, "arm64"),
            ],
            "0.5.11",
            "arm64",
        )
        .unwrap();
        assert_eq!(result.version, Version::parse("0.5.12").unwrap());
        assert!(result.prerelease);
        assert!(choose(vec![release("0.5.9", true, "arm64")], "0.5.11", "arm64").is_none());
        assert!(choose(
            vec![release("1.0.0+new", false, "arm64")],
            "1.0.0+local",
            "arm64"
        )
        .is_none());
        assert!(choose(vec![release("1.0.0-rc.1", true, "arm64")], "1.0.0", "arm64").is_none());
    }
    #[test]
    fn developer_build_warning_distinguishes_newer_equal_and_missing_releases() {
        let bytes = serde_json::to_vec(&vec![release("0.5.9", true, "arm64")]).unwrap();
        let newer = select(&bytes, "0.5.11", "arm64").unwrap();
        assert!(newer.update.is_none());
        assert_eq!(newer.ahead_of, Some(Version::parse("0.5.9").unwrap()));
        let state = State {
            checked: true,
            ahead_of: newer.ahead_of,
            ..Default::default()
        };
        assert!(state
            .label()
            .contains("newer than the latest GitHub release 0.5.9"));
        for (current, architecture) in [("0.5.9", "arm64"), ("0.5.11", "x86_64")] {
            let equal_or_missing = select(&bytes, current, architecture).unwrap();
            assert!(equal_or_missing.update.is_none());
            assert!(equal_or_missing.ahead_of.is_none());
        }
        let update = select(&bytes, "0.5.8", "arm64").unwrap();
        assert!(update.update.is_some());
        assert!(update.ahead_of.is_none());
    }
    #[test]
    fn requires_published_release_and_exact_repository_architecture_asset() {
        let mut draft = release("2.0.0", false, "arm64");
        draft["draft"] = true.into();
        let mut unpublished = release("2.0.0", false, "arm64");
        unpublished["published_at"] = serde_json::Value::Null;
        let mut wrong = release("2.0.0", false, "arm64");
        wrong["assets"][0]["browser_download_url"] = "https://invalid.example/app.dmg".into();
        let mut notes = release("2.0.0", false, "arm64");
        notes["html_url"] = "file:///tmp/app".into();
        assert!(choose(
            vec![
                draft,
                unpublished,
                wrong,
                notes,
                release("2.0.0", false, "x86_64")
            ],
            "1.0.0",
            "arm64"
        )
        .is_none());
        assert!(choose(vec![release("2.0.0", false, "x86_64")], "1.0.0", "x86_64").is_some());
        assert!(select(b"not json", "1.0.0", "arm64").is_err());
        assert!(select(&vec![b' '; MAX_RESPONSE + 1], "1.0.0", "arm64").is_err());
    }
    #[test]
    fn checks_are_opt_in_and_combined_releases_select_the_installed_binary() {
        assert!(!Prefs::default().check_updates_at_startup);
        let mut both = release("2.0.0", true, "arm64");
        both["assets"]
            .as_array_mut()
            .unwrap()
            .push(release("2.0.0", true, "x86_64")["assets"][0].clone());
        for arch in ["arm64", "x86_64"] {
            let found = choose(vec![both.clone()], "1.0.0", arch).unwrap();
            assert!(found.download.ends_with(&format!("macos-{arch}.dmg")));
        }
    }
}
