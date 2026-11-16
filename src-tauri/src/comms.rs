//! Office objects for mail, the phone, and local chat apps.
//!
//! The webviews keep their own cookie jar under the app data directory.
//! This module never sees or stores a password.

use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, Url, WebviewUrl, WebviewWindowBuilder};

const MAIL_URL: &str = "https://mail.google.com/mail/u/0/#inbox?authuser=account%40atla-o.com";
const VOICE_URL: &str = "https://voice.google.com/u/0/calls?authuser=account%40atla-o.com";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommDto {
    pub id: String,
    pub label: String,
    pub mode: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpenPlan {
    Web { window: String, title: String, url: String },
    Local { argv: Vec<String> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Host {
    pub desktop_files: Vec<String>,
    pub binaries: Vec<String>,
    pub flatpaks: Vec<String>,
    pub has_gtk_launch: bool,
    pub has_flatpak: bool,
}

struct AppSpec {
    desktop_ids: &'static [&'static str],
    bins: &'static [&'static str],
    flatpaks: &'static [&'static str],
}

const THUNDERBIRD: AppSpec = AppSpec {
    desktop_ids: &["thunderbird", "mozilla-thunderbird", "org.mozilla.Thunderbird"],
    bins: &["thunderbird"],
    flatpaks: &["org.mozilla.Thunderbird"],
};

const SIGNAL: AppSpec = AppSpec {
    desktop_ids: &["signal", "signal-desktop", "org.signal.Signal"],
    bins: &["signal-desktop", "signal"],
    flatpaks: &["org.signal.Signal"],
};

const DISCORD: AppSpec = AppSpec {
    desktop_ids: &["discord", "discord-canary", "com.discordapp.Discord"],
    bins: &["discord", "Discord"],
    flatpaks: &["com.discordapp.Discord"],
};

#[derive(Clone, Serialize)]
struct BadgeEvent {
    id: String,
    unread: u32,
}

pub fn objects(host: &Host) -> Vec<CommDto> {
    let mut found = vec![
        CommDto {
            id: "mail".into(),
            label: "Mail".into(),
            mode: if native(host, &THUNDERBIRD).is_some() {
                "local".into()
            } else {
                "window".into()
            },
        },
        CommDto {
            id: "voice".into(),
            label: "Phone".into(),
            mode: "window".into(),
        },
    ];
    if native(host, &SIGNAL).is_some() {
        found.push(CommDto {
            id: "signal".into(),
            label: "Signal".into(),
            mode: "local".into(),
        });
    }
    if native(host, &DISCORD).is_some() {
        found.push(CommDto {
            id: "discord".into(),
            label: "Discord".into(),
            mode: "local".into(),
        });
    }
    found
}

pub fn plan(host: &Host, id: &str) -> Result<OpenPlan, String> {
    match id {
        "mail" => {
            if let Some(argv) = native(host, &THUNDERBIRD) {
                Ok(OpenPlan::Local { argv })
            } else {
                Ok(OpenPlan::Web {
                    window: "mail".into(),
                    title: "Atlas Layer — Mail".into(),
                    url: MAIL_URL.into(),
                })
            }
        }
        "voice" => Ok(OpenPlan::Web {
            window: "voice".into(),
            title: "Atlas Layer — Phone".into(),
            url: VOICE_URL.into(),
        }),
        "signal" => native(host, &SIGNAL)
            .map(|argv| OpenPlan::Local { argv })
            .ok_or_else(|| "Signal is not installed.".into()),
        "discord" => native(host, &DISCORD)
            .map(|argv| OpenPlan::Local { argv })
            .ok_or_else(|| "Discord is not installed.".into()),
        _ => Err("That is not an office object.".into()),
    }
}

fn native(host: &Host, spec: &AppSpec) -> Option<Vec<String>> {
    if host.has_gtk_launch {
        for id in spec.desktop_ids.iter().chain(spec.flatpaks.iter()) {
            if host.desktop_files.iter().any(|file| file == id) {
                return Some(vec!["gtk-launch".into(), (*id).into()]);
            }
        }
    }
    for bin in spec.bins {
        if host.binaries.iter().any(|have| have == bin) {
            return Some(vec![(*bin).into()]);
        }
    }
    if host.has_flatpak {
        for id in spec.flatpaks {
            if host.flatpaks.iter().any(|have| have == id) {
                return Some(vec!["flatpak".into(), "run".into(), (*id).into()]);
            }
        }
    }
    None
}

/// A count only when the page title states one. Anything else stays unknown.
pub fn unread_from_title(id: &str, title: &str) -> Option<u32> {
    let title = title.trim();
    if title.is_empty() {
        return None;
    }
    match id {
        "mail" => gmail_unread(title),
        "voice" => voice_unread(title),
        _ => None,
    }
}

fn gmail_unread(title: &str) -> Option<u32> {
    if !title.to_ascii_lowercase().contains("gmail") {
        return None;
    }
    if let Some(count) = leading_count(title) {
        return Some(count);
    }
    if let Some(count) = inbox_count(title) {
        return Some(count);
    }
    let lower = title.to_ascii_lowercase();
    if lower.starts_with("inbox") || lower.starts_with("primary") {
        Some(0)
    } else {
        None
    }
}

fn voice_unread(title: &str) -> Option<u32> {
    if !title.to_ascii_lowercase().contains("google voice") {
        return None;
    }
    leading_count(title).or_else(|| inbox_count(title))
}

fn leading_count(title: &str) -> Option<u32> {
    let rest = title.strip_prefix('(')?;
    let (num, tail) = rest.split_once(')')?;
    if tail.starts_with(' ') && !num.is_empty() && num.chars().all(|c| c.is_ascii_digit()) {
        num.parse().ok()
    } else {
        None
    }
}

fn inbox_count(title: &str) -> Option<u32> {
    let open = title.find('(')?;
    let close = title[open + 1..].find(')')? + open + 1;
    let num = &title[open + 1..close];
    let before = title[..open].trim();
    if !num.is_empty()
        && num.chars().all(|c| c.is_ascii_digit())
        && before.to_ascii_lowercase().ends_with("inbox")
    {
        num.parse().ok()
    } else {
        None
    }
}

pub fn open(app: &AppHandle, id: &str) -> Result<(), String> {
    match plan(&cached_host(), id)? {
        OpenPlan::Local { argv } => spawn_detached(&argv),
        OpenPlan::Web { window, title, url } => open_window(app, id, &window, &title, &url),
    }
}

fn open_window(app: &AppHandle, id: &str, window: &str, title: &str, url: &str) -> Result<(), String> {
    let label = format!("comm-{window}");
    if let Some(existing) = app.get_webview_window(&label) {
        let _ = existing.unminimize();
        let _ = existing.show();
        let _ = existing.set_focus();
        return Ok(());
    }
    let parsed = Url::parse(url).map_err(|err| err.to_string())?;
    let session = session_dir(app)?;
    let parent = app.get_webview_window("main");
    let handle = app.clone();
    let badge_id = id.to_string();
    let mut builder = WebviewWindowBuilder::new(app, &label, WebviewUrl::External(parsed))
        .title(title)
        .inner_size(1040.0, 720.0)
        .min_inner_size(640.0, 480.0)
        .resizable(true)
        .center()
        .focused(true)
        .incognito(false)
        .data_directory(session)
        .enable_clipboard_access()
        .on_document_title_changed(move |_window, title| {
            if let Some(unread) = unread_from_title(&badge_id, &title) {
                let _ = handle.emit(
                    "comms-badge",
                    BadgeEvent {
                        id: badge_id.clone(),
                        unread,
                    },
                );
            }
        })
        .on_new_window(|_url, _features| tauri::webview::NewWindowResponse::Allow);
    if let Some(parent) = parent.as_ref() {
        builder = builder.parent(parent).map_err(|err| err.to_string())?;
    }
    builder.build().map(|_| ()).map_err(|err| err.to_string())
}

fn session_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|err| err.to_string())?
        .join("session");
    fs::create_dir_all(&dir).map_err(|err| err.to_string())?;
    Ok(dir)
}

fn spawn_detached(argv: &[String]) -> Result<(), String> {
    let Some(program) = argv.first() else {
        return Err("There is nothing to open.".into());
    };
    let status = Command::new("setsid")
        .arg("-f")
        .args(argv)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|err| format!("Could not start {program} ({err})."))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("Could not start {program} ({status})."))
    }
}

fn cached_host() -> Host {
    static CACHE: Mutex<Option<(Instant, Host)>> = Mutex::new(None);
    let mut guard = CACHE.lock().unwrap_or_else(|err| err.into_inner());
    if let Some((seen, host)) = guard.as_ref() {
        if seen.elapsed() < Duration::from_secs(30) {
            return host.clone();
        }
    }
    let host = scan_host();
    *guard = Some((Instant::now(), host.clone()));
    host
}

pub fn current_objects() -> Vec<CommDto> {
    objects(&cached_host())
}

pub fn current_host() -> Host {
    cached_host()
}

fn scan_host() -> Host {
    let mut desktop_files = Vec::new();
    for dir in desktop_dirs() {
        let Ok(read) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in read.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if let Some(id) = name.strip_suffix(".desktop") {
                if !id.is_empty() {
                    desktop_files.push(id.to_string());
                }
            }
        }
    }
    let watch = [
        "thunderbird",
        "signal-desktop",
        "signal",
        "discord",
        "Discord",
        "gtk-launch",
        "flatpak",
        "xdg-open",
        "google-chrome",
        "google-chrome-stable",
        "google-chrome-unstable",
        "chromium",
        "chromium-browser",
    ];
    let binaries: Vec<String> = watch
        .into_iter()
        .filter(|name| on_path(name))
        .map(str::to_string)
        .collect();
    let has_flatpak = binaries.iter().any(|bin| bin == "flatpak");
    let flatpaks = if has_flatpak {
        installed_flatpaks()
    } else {
        Vec::new()
    };
    Host {
        desktop_files,
        has_gtk_launch: binaries.iter().any(|bin| bin == "gtk-launch"),
        has_flatpak,
        binaries,
        flatpaks,
    }
}

pub(crate) fn desktop_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(home) = std::env::var_os("XDG_DATA_HOME") {
        dirs.push(PathBuf::from(home).join("applications"));
    } else if let Some(home) = dirs::home_dir() {
        dirs.push(home.join(".local/share/applications"));
    }
    let data_dirs = std::env::var_os("XDG_DATA_DIRS")
        .unwrap_or_else(|| "/usr/local/share:/usr/share".into());
    for dir in std::env::split_paths(&data_dirs) {
        dirs.push(dir.join("applications"));
    }
    if let Some(home) = dirs::home_dir() {
        dirs.push(
            home.join(".local/share/flatpak/exports/share/applications"),
        );
    }
    dirs.push(PathBuf::from("/var/lib/flatpak/exports/share/applications"));
    dirs
}

fn on_path(name: &str) -> bool {
    let Some(path) = std::env::var_os("PATH") else {
        return false;
    };
    std::env::split_paths(&path).any(|dir| dir.join(name).is_file())
}

fn installed_flatpaks() -> Vec<String> {
    let Ok(output) = Command::new("flatpak")
        .args(["list", "--app", "--columns=application"])
        .stdin(Stdio::null())
        .output()
    else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.contains(' '))
        .map(str::to_string)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty() -> Host {
        Host {
            desktop_files: Vec::new(),
            binaries: Vec::new(),
            flatpaks: Vec::new(),
            has_gtk_launch: true,
            has_flatpak: false,
        }
    }

    #[test]
    fn mail_and_phone_are_always_objects() {
        let found = objects(&empty());
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].id, "mail");
        assert_eq!(found[0].mode, "window");
        assert_eq!(found[1].id, "voice");
    }

    #[test]
    fn thunderbird_replaces_the_gmail_window() {
        let mut host = empty();
        host.desktop_files.push("org.mozilla.Thunderbird".into());
        let found = objects(&host);
        assert_eq!(found[0].mode, "local");
        match plan(&host, "mail").unwrap() {
            OpenPlan::Local { argv } => {
                assert_eq!(argv, vec!["gtk-launch", "org.mozilla.Thunderbird"]);
            }
            OpenPlan::Web { .. } => panic!("mail should open Thunderbird"),
        }
    }

    #[test]
    fn signal_and_discord_appear_only_when_installed() {
        let mut host = empty();
        assert!(objects(&host).iter().all(|item| item.id != "signal"));
        host.binaries.push("signal-desktop".into());
        host.flatpaks.push("com.discordapp.Discord".into());
        host.has_flatpak = true;
        let found = objects(&host);
        assert!(found.iter().any(|item| item.id == "signal" && item.mode == "local"));
        assert!(found.iter().any(|item| item.id == "discord"));
        match plan(&host, "signal").unwrap() {
            OpenPlan::Local { argv } => assert_eq!(argv, vec!["signal-desktop"]),
            OpenPlan::Web { .. } => panic!("signal is a local app"),
        }
        match plan(&host, "discord").unwrap() {
            OpenPlan::Local { argv } => {
                assert_eq!(argv, vec!["flatpak", "run", "com.discordapp.Discord"]);
            }
            OpenPlan::Web { .. } => panic!("discord is a local app"),
        }
    }

    #[test]
    fn gmail_count_comes_only_from_the_title() {
        assert_eq!(
            unread_from_title("mail", "(3) Inbox - account@atla-o.com - Gmail"),
            Some(3)
        );
        assert_eq!(unread_from_title("mail", "Inbox (12) - Gmail"), Some(12));
        assert_eq!(unread_from_title("mail", "Inbox - account@atla-o.com - Gmail"), Some(0));
        assert_eq!(unread_from_title("mail", "Gmail"), None);
        assert_eq!(unread_from_title("mail", "Sign in - Google Accounts"), None);
        assert_eq!(unread_from_title("voice", "Google Voice"), None);
        assert_eq!(unread_from_title("voice", "(2) Google Voice"), Some(2));
        assert_eq!(unread_from_title("voice", "(2) Something else"), None);
    }

    #[test]
    fn plans_use_the_fixed_google_urls() {
        match plan(&empty(), "mail").unwrap() {
            OpenPlan::Web { url, .. } => {
                assert!(url.contains("mail.google.com"));
                assert!(url.contains("authuser=account%40atla-o.com"));
                assert!(!url.to_ascii_lowercase().contains("password"));
            }
            OpenPlan::Local { .. } => panic!("no local mail client"),
        }
        match plan(&empty(), "voice").unwrap() {
            OpenPlan::Web { url, .. } => {
                assert!(url.starts_with("https://voice.google.com/"));
                assert!(url.contains("authuser=account%40atla-o.com"));
            }
            OpenPlan::Local { .. } => panic!("phone is the voice window"),
        }
        assert!(plan(&empty(), "https://example.com").is_err());
    }
}
