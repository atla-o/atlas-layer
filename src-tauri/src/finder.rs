//! The filing cabinet and the shelf.
//!
//! Drive, Docs, Sheets, Gmail, and Calendar open in a Chromium `--app` window.
//! A pinned desktop entry for that site wins, because that is the same window
//! Chrome already made. The shelf lists real `.desktop` files and files under
//! the home directory, and opens them with `gtk-launch` or `xdg-open`.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::SystemTime;

use serde::Serialize;

use crate::comms::{self, Host};

struct WebApp {
    id: &'static str,
    label: &'static str,
    url: &'static str,
    names: &'static [&'static str],
}

const WEB: &[WebApp] = &[
    WebApp {
        id: "drive",
        label: "Drive",
        url: "https://drive.google.com",
        names: &["Google Drive", "Drive"],
    },
    WebApp {
        id: "docs",
        label: "Docs",
        url: "https://docs.google.com",
        names: &["Google Docs", "Docs"],
    },
    WebApp {
        id: "sheets",
        label: "Sheets",
        url: "https://docs.google.com/spreadsheets",
        names: &["Google Sheets", "Sheets"],
    },
    WebApp {
        id: "gmail",
        label: "Gmail",
        url: "https://mail.google.com",
        names: &["Gmail", "Google Mail"],
    },
    WebApp {
        id: "calendar",
        label: "Calendar",
        url: "https://calendar.google.com",
        names: &["Google Calendar"],
    },
];

const BROWSERS: &[&str] = &[
    "google-chrome-stable",
    "google-chrome",
    "google-chrome-unstable",
    "chromium",
    "chromium-browser",
];

const BROWSER_FLATPAKS: &[&str] = &["com.google.Chrome", "org.chromium.Chromium"];

#[derive(Debug, Clone, PartialEq, Eq)]
struct DeskEntry {
    id: String,
    name: String,
    path: PathBuf,
    exec: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShelfHit {
    pub kind: String,
    pub id: String,
    pub label: String,
    pub path: String,
}

pub fn cabinet_ready(host: &Host) -> bool {
    !cabinet_apps(host).is_empty()
}

pub fn shelf_ready(host: &Host) -> bool {
    host.has_gtk_launch || host.binaries.iter().any(|bin| bin == "xdg-open")
}

pub fn cabinet_apps(host: &Host) -> Vec<(String, String)> {
    let entries = desktop_index(&comms::desktop_dirs());
    WEB.iter()
        .filter(|app| web_argv(host, &entries, app).is_some())
        .map(|app| (app.id.into(), app.label.into()))
        .collect()
}

pub fn is_web(id: &str) -> bool {
    WEB.iter().any(|app| app.id == id)
}

pub fn launch_web(host: &Host, id: &str) -> Result<(), String> {
    let app = WEB
        .iter()
        .find(|app| app.id == id)
        .ok_or_else(|| "That is not a cabinet link.".to_string())?;
    let entries = desktop_index(&comms::desktop_dirs());
    let argv = web_argv(host, &entries, app)
        .ok_or_else(|| "Chromium is not installed, so that page has no window.".to_string())?;
    spawn(&argv)
}

pub fn search(home: &Path, query: &str) -> Vec<ShelfHit> {
    let needle = query.trim().to_lowercase();
    let mut hits = Vec::new();
    for entry in desktop_index(&comms::desktop_dirs()) {
        if !needle.is_empty()
            && !entry.name.to_lowercase().contains(&needle)
            && !entry.id.to_lowercase().contains(&needle)
        {
            continue;
        }
        hits.push(ShelfHit {
            kind: "app".into(),
            id: entry.id,
            label: entry.name,
            path: entry.path.display().to_string(),
        });
        if hits.len() == 400 {
            break;
        }
    }
    if needle.chars().count() >= 2 {
        let mut files = Vec::new();
        let mut budget = 8000u32;
        walk_files(home, &needle, &mut files, &mut budget);
        files.sort_by(|left, right| left.1.cmp(&right.1));
        for (_, path) in files.into_iter().take(24) {
            hits.push(ShelfHit {
                kind: "file".into(),
                id: String::new(),
                label: path
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_else(|| path.display().to_string()),
                path: path.display().to_string(),
            });
        }
    }
    hits
}

pub fn open_hit(host: &Host, kind: &str, id: &str, path: &str, home: &Path) -> Result<(), String> {
    match kind {
        "app" => {
            let entries = desktop_index(&comms::desktop_dirs());
            let entry = entries
                .into_iter()
                .find(|entry| entry.id == id)
                .ok_or_else(|| "That app is not installed.".to_string())?;
            if host.has_gtk_launch {
                spawn(&["gtk-launch".into(), entry.id])
            } else if host.binaries.iter().any(|bin| bin == "xdg-open") {
                spawn(&["xdg-open".into(), entry.path.display().to_string()])
            } else {
                Err("gtk-launch and xdg-open are both missing.".into())
            }
        }
        "file" => {
            if !host.binaries.iter().any(|bin| bin == "xdg-open") {
                return Err("xdg-open is not installed.".into());
            }
            let file = checked_home_file(home, path)?;
            spawn(&["xdg-open".into(), file.display().to_string()])
        }
        _ => Err("That is not something on the shelf.".into()),
    }
}

fn web_argv(host: &Host, entries: &[DeskEntry], app: &WebApp) -> Option<Vec<String>> {
    if host.has_gtk_launch {
        if let Some(pinned) = entries.iter().find(|entry| pinned(entry, app)) {
            return Some(vec!["gtk-launch".into(), pinned.id.clone()]);
        }
    }
    for bin in BROWSERS {
        if host.binaries.iter().any(|have| have == bin) {
            return Some(vec![(*bin).into(), format!("--app={}", app.url)]);
        }
    }
    if host.has_flatpak {
        for id in BROWSER_FLATPAKS {
            if host.flatpaks.iter().any(|have| have == id) {
                return Some(vec![
                    "flatpak".into(),
                    "run".into(),
                    (*id).into(),
                    format!("--app={}", app.url),
                ]);
            }
        }
    }
    None
}

fn pinned(entry: &DeskEntry, app: &WebApp) -> bool {
    let exec = entry.exec.to_lowercase();
    if !exec.contains("--app") {
        return false;
    }
    let name = entry.name.to_lowercase();
    app.names.iter().any(|want| name == want.to_lowercase())
}

fn desktop_index(dirs: &[PathBuf]) -> Vec<DeskEntry> {
    let mut found = BTreeMap::new();
    // Earlier directories are higher priority, so they are applied last.
    for dir in dirs.iter().rev() {
        let Ok(read) = fs::read_dir(dir) else {
            continue;
        };
        for entry in read.flatten() {
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("desktop") {
                continue;
            }
            if let Some(parsed) = parse_desktop(&path) {
                found.insert(parsed.id.clone(), parsed);
            }
        }
    }
    let mut entries: Vec<_> = found.into_values().collect();
    entries.sort_by(|left, right| left.name.to_lowercase().cmp(&right.name.to_lowercase()));
    entries
}

fn parse_desktop(path: &Path) -> Option<DeskEntry> {
    let text = fs::read_to_string(path).ok()?;
    let mut name = None;
    let mut name_en = None;
    let mut exec = String::new();
    let mut kind = String::new();
    let mut hidden = false;
    let mut no_display = false;
    let mut try_exec = None;
    let mut in_entry = false;
    let mut seen_entry = false;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            if seen_entry {
                break;
            }
            in_entry = line.eq_ignore_ascii_case("[Desktop Entry]");
            seen_entry = in_entry;
            continue;
        }
        if !in_entry || line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let value = value.trim();
        match key {
            "Name" => name = Some(value.to_string()),
            "Name[en]" | "Name[en_US]" => name_en = Some(value.to_string()),
            "Exec" => exec = value.to_string(),
            "Type" => kind = value.to_string(),
            "Hidden" => hidden = value.eq_ignore_ascii_case("true"),
            "NoDisplay" => no_display = value.eq_ignore_ascii_case("true"),
            "TryExec" => try_exec = Some(value.to_string()),
            _ => {}
        }
    }
    if hidden || no_display {
        return None;
    }
    if kind != "Application" && kind != "Link" {
        return None;
    }
    if kind == "Application" && exec.is_empty() {
        return None;
    }
    if let Some(bin) = try_exec.as_deref() {
        if !try_exec_exists(bin) {
            return None;
        }
    }
    let label = name.or(name_en)?;
    if label.is_empty() {
        return None;
    }
    let id = path.file_stem()?.to_str()?.to_string();
    if id.is_empty() {
        return None;
    }
    Some(DeskEntry {
        id,
        name: label,
        path: path.to_path_buf(),
        exec,
    })
}

fn try_exec_exists(bin: &str) -> bool {
    let path = Path::new(bin);
    if path.is_absolute() {
        return path.is_file();
    }
    let Some(path_var) = std::env::var_os("PATH") else {
        return false;
    };
    std::env::split_paths(&path_var).any(|dir| dir.join(bin).is_file())
}

const SKIP: &[&str] = &[
    "node_modules",
    "target",
    "dist",
    "build",
    "flatpak",
    "cache",
    "Cache",
    "Trash",
];

fn walk_files(dir: &Path, needle: &str, found: &mut Vec<(SystemTime, PathBuf)>, budget: &mut u32) {
    if *budget == 0 || found.len() >= 80 {
        return;
    }
    let Ok(read) = fs::read_dir(dir) else {
        return;
    };
    for entry in read.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with('.') || SKIP.contains(&name.as_ref()) {
            continue;
        }
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if kind.is_symlink() {
            continue;
        }
        let path = entry.path();
        if kind.is_dir() {
            *budget = budget.saturating_sub(1);
            walk_files(&path, needle, found, budget);
            continue;
        }
        if name.to_lowercase().contains(needle) {
            let when = entry
                .metadata()
                .and_then(|meta| meta.modified())
                .unwrap_or(SystemTime::UNIX_EPOCH);
            found.push((when, path));
        }
    }
}

fn checked_home_file(home: &Path, raw: &str) -> Result<PathBuf, String> {
    let path = PathBuf::from(raw);
    if !path.is_absolute() {
        return Err("That file path is not absolute.".into());
    }
    let home = home.canonicalize().map_err(|err| err.to_string())?;
    let canon = path.canonicalize().map_err(|err| err.to_string())?;
    if !canon.starts_with(&home) {
        return Err("That file is outside the home directory.".into());
    }
    if !canon.is_file() {
        return Err("That is not a file.".into());
    }
    Ok(canon)
}

fn spawn(argv: &[String]) -> Result<(), String> {
    let Some(program) = argv.first() else {
        return Err("There is nothing to open.".into());
    };
    let allowed = program == "gtk-launch"
        || program == "xdg-open"
        || program == "flatpak"
        || BROWSERS.contains(&program.as_str());
    if !allowed {
        return Err("That launch is not allowed.".into());
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn host_with(bins: &[&str], gtk: bool) -> Host {
        Host {
            desktop_files: Vec::new(),
            binaries: bins.iter().map(|bin| (*bin).to_string()).collect(),
            flatpaks: Vec::new(),
            has_gtk_launch: gtk,
            has_flatpak: false,
        }
    }

    fn write_desktop(dir: &Path, file: &str, body: &str) {
        fs::create_dir_all(dir).unwrap();
        fs::write(dir.join(file), body).unwrap();
    }

    #[test]
    fn drive_uses_a_pinned_app_window_when_one_exists() {
        let dir = std::env::temp_dir().join(format!("atlas-desk-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        write_desktop(
            &dir,
            "chrome-drive.desktop",
            "[Desktop Entry]\nType=Application\nName=Google Drive\nExec=/usr/bin/google-chrome --profile-directory=Default --app-id=abc\n",
        );
        let entries = desktop_index(&[dir.clone()]);
        let host = host_with(&["google-chrome-stable"], true);
        let drive = WEB.iter().find(|app| app.id == "drive").unwrap();
        assert_eq!(
            web_argv(&host, &entries, drive).unwrap(),
            vec!["gtk-launch", "chrome-drive"]
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn drive_falls_back_to_chromium_app_mode() {
        let host = host_with(&["google-chrome-stable"], true);
        let drive = WEB.iter().find(|app| app.id == "drive").unwrap();
        assert_eq!(
            web_argv(&host, &[], drive).unwrap(),
            vec!["google-chrome-stable", "--app=https://drive.google.com"]
        );
        assert!(web_argv(&host_with(&[], true), &[], drive).is_none());
    }

    #[test]
    fn the_shelf_lists_apps_and_matching_files_only() {
        let root = std::env::temp_dir().join(format!("atlas-shelf-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let apps = root.join("applications");
        let home = root.join("home");
        write_desktop(
            &apps,
            "krita.desktop",
            "[Desktop Entry]\nType=Application\nName=Krita\nExec=krita %F\n",
        );
        write_desktop(
            &apps,
            "hidden.desktop",
            "[Desktop Entry]\nType=Application\nName=Secret\nNoDisplay=true\nExec=true\n",
        );
        fs::create_dir_all(home.join("notes")).unwrap();
        fs::create_dir_all(home.join(".cache")).unwrap();
        fs::write(home.join("notes").join("kiln.txt"), b"notes").unwrap();
        fs::write(home.join(".cache").join("kiln.txt"), b"no").unwrap();
        let listed = desktop_index(&[apps]);
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].name, "Krita");
        let mut files = Vec::new();
        let mut budget = 100u32;
        walk_files(&home, "kiln", &mut files, &mut budget);
        assert_eq!(files.len(), 1);
        assert!(files[0].1.ends_with("notes/kiln.txt"));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_file_outside_home_is_refused() {
        let home = std::env::temp_dir().join(format!("atlas-home-{}", std::process::id()));
        let _ = fs::remove_dir_all(&home);
        fs::create_dir_all(&home).unwrap();
        let outside = std::env::temp_dir().join(format!("atlas-out-{}", std::process::id()));
        fs::write(&outside, b"no").unwrap();
        assert!(checked_home_file(&home, outside.to_str().unwrap()).is_err());
        let inside = home.join("note.txt");
        fs::write(&inside, b"yes").unwrap();
        assert!(checked_home_file(&home, inside.to_str().unwrap()).is_ok());
        let _ = fs::remove_dir_all(&home);
        let _ = fs::remove_file(&outside);
    }
}
