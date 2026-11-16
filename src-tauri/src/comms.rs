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
    let mail_title = "Atlas Layer \u{2014} Mail";
    let phone_title = "Atlas Layer \u{2014} Phone";
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
