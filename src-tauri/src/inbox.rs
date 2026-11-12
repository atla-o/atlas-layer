use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InboxReply {
    pub bot_key: String,
    pub text: String,
    pub in_reply_to: Option<String>,
    pub ts: Option<String>,
}

#[derive(Debug)]
pub struct PendingFile {
    pub path: PathBuf,
    pub reply: InboxReply,
}

pub fn parse_reply(value: &Value) -> Result<InboxReply, String> {
    let bot = value
        .get("bot")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim();
    if bot.is_empty() {
        return Err("inbox reply is missing bot".into());
    }
    let text = value
        .get("text")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim();
    if text.is_empty() {
        return Err("inbox reply is missing text".into());
    }
    let in_reply_to = value
        .get("in_reply_to")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    let ts = value
        .get("ts")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    Ok(InboxReply {
        bot_key: bot.to_string(),
        text: text.to_string(),
        in_reply_to,
        ts,
    })
}

pub fn list_pending(dir: &Path) -> Result<Vec<PendingFile>, String> {
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut paths: Vec<PathBuf> = fs::read_dir(dir)
        .map_err(|err| err.to_string())?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.is_file())
        .collect();
    paths.sort();
    let mut pending = Vec::new();
    for path in paths {
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if name.starts_with('.') || !name.ends_with(".json") {
            continue;
        }
        let text = fs::read_to_string(&path).map_err(|err| err.to_string())?;
        let parsed = serde_json::from_str::<Value>(&text)
            .ok()
            .and_then(|value| parse_reply(&value).ok());
        match parsed {
            Some(reply) => pending.push(PendingFile { path, reply }),
            None => {
                let _ = archive(&path, dir, "unmatched");
            }
        }
    }
    Ok(pending)
}

pub fn archive(path: &Path, inbox_dir: &Path, folder: &str) -> Result<(), String> {
    let dest_dir = inbox_dir.join(folder);
    fs::create_dir_all(&dest_dir).map_err(|err| err.to_string())?;
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("reply.json");
    let dest = dest_dir.join(format!(
        "{}-{name}",
        crate::model::now_iso().replace(':', "")
    ));
    fs::rename(path, &dest).map_err(|err| err.to_string())?;
    Ok(())
}

/// Reads complete JSONL lines after `offset`. A trailing partial line stays unconsumed.
pub fn read_jsonl_from(
    path: &Path,
    offset: u64,
) -> Result<(Vec<Result<InboxReply, String>>, u64), String> {
    if !path.exists() {
        return Ok((Vec::new(), 0));
    }
    let mut file = File::open(path).map_err(|err| err.to_string())?;
    let len = file.metadata().map_err(|err| err.to_string())?.len();
    let start = if offset > len { 0 } else { offset };
    file.seek(SeekFrom::Start(start))
        .map_err(|err| err.to_string())?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)
        .map_err(|err| err.to_string())?;
    let Some(last_nl) = bytes.iter().rposition(|byte| *byte == b'\n') else {
        return Ok((Vec::new(), start));
    };
    let end = last_nl + 1;
    let text = String::from_utf8_lossy(&bytes[..end]);
    let mut out = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        match serde_json::from_str::<Value>(trimmed) {
            Ok(value) => out.push(parse_reply(&value)),
            Err(_) => out.push(Err("inbox line is not JSON".into())),
        }
    }
    Ok((out, start + end as u64))
}

pub fn content_id(source: &str, reply: &InboxReply) -> String {
    let raw = format!(
        "{source}|{}|{}|{}|{}",
        reply.bot_key,
        reply.text,
        reply.in_reply_to.as_deref().unwrap_or(""),
        reply.ts.as_deref().unwrap_or("")
    );
    format!("inbox-{:x}", fnv64(raw.as_bytes()))
}

fn fnv64(bytes: &[u8]) -> u64 {
    let mut hash = 1469598103934665603u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(1099511628211);
    }
    hash
}

pub fn load_offset(history_dir: &Path, inbox_file: &Path) -> u64 {
    let path = history_dir.join("inbox-offset.txt");
    let Ok(text) = fs::read_to_string(path) else {
        return 0;
    };
    let mut parts = text.splitn(2, char::is_whitespace);
    let offset: u64 = parts.next().unwrap_or("0").trim().parse().unwrap_or(0);
    let stored = parts.next().unwrap_or("").trim();
    if stored == inbox_file.display().to_string() {
        offset
    } else {
        0
    }
}

pub fn store_offset(history_dir: &Path, inbox_file: &Path, offset: u64) {
    let _ = fs::create_dir_all(history_dir);
    let line = format!("{offset} {}\n", inbox_file.display());
    let _ = fs::write(history_dir.join("inbox-offset.txt"), line);
}
