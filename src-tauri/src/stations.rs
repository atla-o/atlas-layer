//! Clickable work and play spots in the office.
//!
//! A spot is drawn only when at least one of its apps is installed.
//! Launch goes through that app's desktop entry (`gtk-launch`) or, if the
//! desktop file is missing, `flatpak run`. Nothing here is a placeholder.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::SystemTime;

use serde::Serialize;

use crate::comms::Host;

struct Tool {
    id: &'static str,
    label: &'static str,
    station: &'static str,
    desktop_ids: &'static [&'static str],
    flatpaks: &'static [&'static str],
    extensions: &'static [&'static str],
}

const TOOLS: &[Tool] = &[
    Tool {
        id: "freecad",
        label: "FreeCAD",
        station: "cad",
        desktop_ids: &["freecad", "org.freecad.FreeCAD", "org.freecadweb.FreeCAD"],
        flatpaks: &["org.freecad.FreeCAD", "org.freecadweb.FreeCAD"],
        extensions: &["fcstd"],
    },
    Tool {
        id: "kicad",
        label: "KiCad",
        station: "cad",
        desktop_ids: &["kicad", "org.kicad.KiCad"],
        flatpaks: &["org.kicad.KiCad"],
        extensions: &["kicad_pro"],
    },
    Tool {
        id: "librecad",
        label: "LibreCAD",
        station: "cad",
        desktop_ids: &["librecad", "org.librecad.LibreCAD"],
        flatpaks: &["org.librecad.LibreCAD"],
        extensions: &["dxf"],
    },
    Tool {
        id: "blender",
        label: "Blender",
        station: "cad",
        desktop_ids: &["blender", "org.blender.Blender"],
        flatpaks: &["org.blender.Blender"],
        extensions: &["blend"],
    },
    Tool {
        id: "gimp",
        label: "GIMP",
        station: "easel",
        desktop_ids: &["gimp", "org.gimp.GIMP"],
        flatpaks: &["org.gimp.GIMP"],
        extensions: &[],
    },
    Tool {
        id: "inkscape",
        label: "Inkscape",
        station: "easel",
        desktop_ids: &["inkscape", "org.inkscape.Inkscape"],
        flatpaks: &["org.inkscape.Inkscape"],
        extensions: &[],
    },
    Tool {
        id: "krita",
        label: "Krita",
        station: "easel",
        desktop_ids: &["krita", "org.kde.krita"],
        flatpaks: &["org.kde.krita"],
        extensions: &[],
    },
    Tool {
        id: "kdenlive",
        label: "Kdenlive",
        station: "easel",
        desktop_ids: &["kdenlive", "org.kde.kdenlive"],
        flatpaks: &["org.kde.kdenlive"],
        extensions: &[],
    },
    Tool {
        id: "darktable",
        label: "darktable",
        station: "easel",
        desktop_ids: &["darktable", "org.darktable.Darktable"],
        flatpaks: &["org.darktable.Darktable"],
        extensions: &[],
    },
    Tool {
        id: "retroarch",
        label: "RetroArch",
        station: "arcade",
        desktop_ids: &["retroarch", "org.libretro.RetroArch", "com.libretro.RetroArch"],
        flatpaks: &["org.libretro.RetroArch"],
        extensions: &[],
    },
    Tool {
        id: "openttd",
        label: "OpenTTD",
        station: "arcade",
        desktop_ids: &["openttd", "org.openttd.OpenTTD"],
        flatpaks: &["org.openttd.OpenTTD"],
        extensions: &[],
    },
    Tool {
        id: "zeroad",
        label: "0 A.D.",
        station: "arcade",
        desktop_ids: &["0ad", "play0ad", "com.play0ad.zeroad"],
        flatpaks: &["com.play0ad.zeroad"],
        extensions: &[],
    },
    Tool {
        id: "supertuxkart",
        label: "SuperTuxKart",
        station: "arcade",
        desktop_ids: &["supertuxkart", "net.supertuxkart.SuperTuxKart"],
        flatpaks: &["net.supertuxkart.SuperTuxKart"],
        extensions: &[],
    },
    Tool {
        id: "mindustry",
        label: "Mindustry",
        station: "arcade",
        desktop_ids: &["mindustry", "com.github.Anuken.Mindustry"],
        flatpaks: &["com.github.Anuken.Mindustry"],
        extensions: &[],
    },
    Tool {
        id: "shattered",
        label: "Shattered Pixel Dungeon",
        station: "arcade",
        desktop_ids: &[
            "shattered-pixel-dungeon",
            "com.shatteredpixel.shatteredpixeldungeon",
        ],
        flatpaks: &["com.shatteredpixel.shatteredpixeldungeon"],
        extensions: &[],
    },
    Tool {
        id: "wesnoth",
        label: "Wesnoth",
        station: "arcade",
        desktop_ids: &["wesnoth", "org.wesnoth.Wesnoth"],
        flatpaks: &["org.wesnoth.Wesnoth"],
        extensions: &[],
    },
    Tool {
        id: "luanti",
        label: "Luanti",
        station: "arcade",
        desktop_ids: &["luanti", "org.luanti.luanti", "net.minetest.Minetest", "minetest"],
        flatpaks: &["org.luanti.luanti", "net.minetest.Minetest"],
        extensions: &[],
    },
    Tool {
        id: "endless-sky",
        label: "Endless Sky",
        station: "arcade",
        desktop_ids: &["endless-sky", "io.github.endless_sky.endless_sky"],
        flatpaks: &["io.github.endless_sky.endless_sky"],
        extensions: &[],
    },
    Tool {
        id: "xonotic",
        label: "Xonotic",
        station: "arcade",
        desktop_ids: &["xonotic", "org.xonotic.Xonotic"],
        flatpaks: &["org.xonotic.Xonotic"],
        extensions: &[],
    },
    Tool {
        id: "stremio",
        label: "Stremio",
        station: "tv",
        desktop_ids: &["stremio", "com.stremio.Stremio"],
        flatpaks: &["com.stremio.Stremio"],
        extensions: &[],
    },
    Tool {
        id: "steam",
        label: "Steam",
        station: "tv",
        desktop_ids: &["steam", "com.valvesoftware.Steam"],
        flatpaks: &["com.valvesoftware.Steam"],
        extensions: &[],
    },
    Tool {
        id: "heroic",
        label: "Heroic",
        station: "tv",
        desktop_ids: &["heroic", "com.heroicgameslauncher.hgl"],
        flatpaks: &["com.heroicgameslauncher.hgl"],
        extensions: &[],
    },
    Tool {
        id: "geforce-now",
        label: "GeForce NOW",
        station: "tv",
        desktop_ids: &[
            "geforce-now",
            "nvidia-geforce-now",
            "com.nvidia.geforcenow",
            "com.nvidia.GeForceNOW",
        ],
        flatpaks: &["com.nvidia.geforcenow"],
        extensions: &[],
    },
];

const STATIONS: &[(&str, &str)] = &[
    ("cad", "CAD bench"),
    ("easel", "Easel"),
    ("arcade", "Arcade"),
    ("tv", "TV"),
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StationDto {
    pub id: String,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppDto {
    pub id: String,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecentFile {
    pub app: String,
    pub name: String,
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StationDetail {
    pub id: String,
    pub label: String,
    pub apps: Vec<AppDto>,
    pub files: Vec<RecentFile>,
}

pub fn stations(host: &Host) -> Vec<StationDto> {
    STATIONS
        .iter()
        .filter_map(|(id, label)| {
            let any = TOOLS
                .iter()
                .any(|tool| tool.station == *id && launch_argv(host, tool).is_some());
            if any {
                Some(StationDto {
                    id: (*id).into(),
                    label: (*label).into(),
                })
            } else {
                None
            }
        })
        .collect()
}

pub fn detail(host: &Host, id: &str, home: &Path) -> Result<StationDetail, String> {
    let label = STATIONS
        .iter()
        .find(|(station, _)| *station == id)
        .map(|(_, label)| *label)
        .ok_or_else(|| "That is not an office object.".to_string())?;
    let apps: Vec<AppDto> = TOOLS
        .iter()
        .filter(|tool| tool.station == id && launch_argv(host, tool).is_some())
        .map(|tool| AppDto {
            id: tool.id.into(),
            label: tool.label.into(),
        })
        .collect();
    if apps.is_empty() {
        return Err(format!("{label} has nothing installed."));
    }
    let files = if id == "cad" {
        recent_for(&apps, home)
    } else {
        Vec::new()
    };
    Ok(StationDetail {
        id: id.into(),
        label: label.into(),
        apps,
        files,
    })
}

pub fn launch(host: &Host, app_id: &str, file: Option<&str>, home: &Path) -> Result<(), String> {
    let tool = TOOLS
        .iter()
        .find(|tool| tool.id == app_id)
        .ok_or_else(|| "That app is not on a bench.".to_string())?;
    let mut argv = launch_argv(host, tool)
        .ok_or_else(|| format!("{} is not installed.", tool.label))?;
    if let Some(raw) = file {
        let path = checked_file(home, tool, raw)?;
        if argv.first().is_some_and(|program| program == "gtk-launch") {
            argv.push(file_uri(&path));
        } else {
            argv.push(path.display().to_string());
        }
    }
    spawn(&argv)
}

fn launch_argv(host: &Host, tool: &Tool) -> Option<Vec<String>> {
    if host.has_gtk_launch {
        for id in tool.desktop_ids.iter().chain(tool.flatpaks.iter()) {
            if host.desktop_files.iter().any(|file| file == id) {
                return Some(vec!["gtk-launch".into(), (*id).into()]);
            }
        }
    }
    if host.has_flatpak {
        for id in tool.flatpaks {
            if host.flatpaks.iter().any(|have| have == id) {
                return Some(vec!["flatpak".into(), "run".into(), (*id).into()]);
            }
        }
    }
    None
}

fn checked_file(home: &Path, tool: &Tool, raw: &str) -> Result<PathBuf, String> {
    if tool.extensions.is_empty() {
        return Err(format!("{} opens on its own.", tool.label));
    }
    let path = PathBuf::from(raw);
    if !path.is_absolute() {
        return Err("That file path is not absolute.".into());
    }
    let home = home.canonicalize().map_err(|err| err.to_string())?;
    let canon = path.canonicalize().map_err(|err| err.to_string())?;
    if !canon.starts_with(&home) {
        return Err("That file is outside the home directory.".into());
    }
    let ext = canon
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if !tool.extensions.iter().any(|want| *want == ext) {
        return Err(format!("That file is not a {} file.", tool.label));
    }
    Ok(canon)
}

fn file_uri(path: &Path) -> String {
    let mut uri = String::from("file://");
    for byte in path.to_string_lossy().bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'/' | b'-' | b'_' | b'.' | b'~' => {
                uri.push(byte as char);
            }
            _ => uri.push_str(&format!("%{byte:02X}")),
        }
    }
    uri
}

fn recent_for(apps: &[AppDto], home: &Path) -> Vec<RecentFile> {
    let mut found = Vec::new();
    let mut budget = 8000u32;
    walk(home, &mut found, &mut budget);
    found.sort_by(|left, right| right.0.cmp(&left.0).then(left.1.cmp(&right.1)));
    let mut files = Vec::new();
    for app in apps {
        let Some(tool) = TOOLS.iter().find(|tool| tool.id == app.id) else {
            continue;
        };
        let mut kept = 0usize;
        for (_, path) in &found {
            let ext = path
                .extension()
                .and_then(|ext| ext.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            if !tool.extensions.iter().any(|want| *want == ext) {
                continue;
            }
            files.push(RecentFile {
                app: app.id.clone(),
                name: path
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_else(|| path.display().to_string()),
                path: path.display().to_string(),
            });
            kept += 1;
            if kept == 8 {
                break;
            }
        }
    }
    files
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

fn walk(dir: &Path, found: &mut Vec<(SystemTime, PathBuf)>, budget: &mut u32) {
    if *budget == 0 {
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
            walk(&path, found, budget);
            continue;
        }
        let ext = path
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if !matches!(ext.as_str(), "fcstd" | "kicad_pro" | "dxf" | "blend") {
            continue;
        }
        let when = entry
            .metadata()
            .and_then(|meta| meta.modified())
            .unwrap_or(SystemTime::UNIX_EPOCH);
        found.push((when, path));
    }
}

fn spawn(argv: &[String]) -> Result<(), String> {
    let Some(program) = argv.first() else {
        return Err("There is nothing to launch.".into());
    };
    if program != "gtk-launch" && program != "flatpak" {
        return Err("That launch is not a desktop entry.".into());
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

    fn host(desktops: &[&str], flatpaks: &[&str]) -> Host {
        Host {
            desktop_files: desktops.iter().map(|id| (*id).to_string()).collect(),
            binaries: Vec::new(),
            flatpaks: flatpaks.iter().map(|id| (*id).to_string()).collect(),
            has_gtk_launch: true,
            has_flatpak: !flatpaks.is_empty(),
        }
    }

    #[test]
    fn a_spot_appears_only_when_an_app_can_launch() {
        let empty = host(&[], &[]);
        assert!(stations(&empty).is_empty());
        let machine = host(&["org.blender.Blender", "org.gimp.GIMP"], &[]);
        let found = stations(&machine);
        assert_eq!(
            found.iter().map(|item| item.id.as_str()).collect::<Vec<_>>(),
            vec!["cad", "easel"]
        );
    }

    #[test]
    fn blender_opens_through_gtk_launch_with_the_blend_file() {
        let machine = host(&[], &["org.blender.Blender"]);
        let mut machine = machine;
        machine.desktop_files.push("org.blender.Blender".into());
        let home = std::env::temp_dir().join(format!("atlas-station-{}", std::process::id()));
        let _ = fs::remove_dir_all(&home);
        fs::create_dir_all(home.join("sims")).unwrap();
        let blend = home.join("sims").join("cart.blend");
        fs::write(&blend, b"BLENDER").unwrap();
        let argv = {
            let tool = TOOLS.iter().find(|tool| tool.id == "blender").unwrap();
            let mut argv = launch_argv(&machine, tool).unwrap();
            let path = checked_file(&home, tool, blend.to_str().unwrap()).unwrap();
            argv.push(file_uri(&path));
            argv
        };
        assert_eq!(argv[0], "gtk-launch");
        assert_eq!(argv[1], "org.blender.Blender");
        assert!(argv[2].starts_with("file://"));
        assert!(argv[2].ends_with("/sims/cart.blend"));
        assert!(checked_file(&home, TOOLS.iter().find(|tool| tool.id == "kicad").unwrap(), blend.to_str().unwrap()).is_err());
        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn flatpak_is_the_launch_when_there_is_no_desktop_file() {
        let machine = host(&[], &["org.openttd.OpenTTD"]);
        let tool = TOOLS.iter().find(|tool| tool.id == "openttd").unwrap();
        assert_eq!(
            launch_argv(&machine, tool).unwrap(),
            vec!["flatpak", "run", "org.openttd.OpenTTD"]
        );
        assert!(launch_argv(&host(&[], &[]), tool).is_none());
    }

    #[test]
    fn recent_files_are_grouped_by_app_and_skip_hidden_dirs() {
        let home = std::env::temp_dir().join(format!("atlas-recent-{}", std::process::id()));
        let _ = fs::remove_dir_all(&home);
        fs::create_dir_all(home.join("models")).unwrap();
        fs::create_dir_all(home.join(".secret")).unwrap();
        fs::create_dir_all(home.join("boards")).unwrap();
        fs::write(home.join("models").join("arm.FCStd"), b"cad").unwrap();
        fs::write(home.join("boards").join("shelf.kicad_pro"), b"pcb").unwrap();
        fs::write(home.join("models").join("notes.dxf"), b"dxf").unwrap();
        fs::write(home.join("models").join("town.blend"), b"blend").unwrap();
        fs::write(home.join(".secret").join("hidden.blend"), b"no").unwrap();
        fs::write(home.join("models").join("backup.blend1"), b"no").unwrap();
        let apps = vec![
            AppDto { id: "freecad".into(), label: "FreeCAD".into() },
            AppDto { id: "kicad".into(), label: "KiCad".into() },
            AppDto { id: "librecad".into(), label: "LibreCAD".into() },
            AppDto { id: "blender".into(), label: "Blender".into() },
        ];
        let files = recent_for(&apps, &home);
        let names: Vec<_> = files.iter().map(|file| file.name.as_str()).collect();
        assert_eq!(names, vec!["arm.FCStd", "shelf.kicad_pro", "notes.dxf", "town.blend"]);
        assert_eq!(files[0].app, "freecad");
        assert_eq!(files[3].app, "blender");
        let _ = fs::remove_dir_all(&home);
    }
}
