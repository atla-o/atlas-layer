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
        if name.starts_with('.') || !name.ends_with(".json") || is_live_mirror(name) {
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

/// `atlas-*.json` (any `{desk}-*.json`) stays in the inbox. Atlas updates those
/// files in place. Each file is either one message or `{ "messages": [ ... ] }`.
pub fn is_live_mirror(name: &str) -> bool {
    let Some(stem) = name.strip_suffix(".json") else {
        return false;
    };
    match stem.split_once('-') {
        Some((bot, rest)) => !bot.is_empty() && !rest.is_empty() && !bot.contains('.'),
        None => false,
    }
}

#[derive(Debug, Clone)]
pub struct LiveLine {
    pub bot_key: String,
    pub message: crate::model::StoredMessage,
}

pub fn read_live_mirrors(dir: &Path) -> Vec<LiveLine> {
    if !dir.exists() {
        return Vec::new();
    }
    let mut paths: Vec<PathBuf> = fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(is_live_mirror)
        })
        .collect();
    paths.sort();
    let mut lines = Vec::new();
    for path in paths {
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };
        let Ok(value) = serde_json::from_str::<Value>(&text) else {
            continue;
        };
        let bot = value
            .get("bot")
            .and_then(|item| item.as_str())
            .map(str::trim)
            .filter(|text| !text.is_empty())
            .map(str::to_string)
            .or_else(|| {
                name.split_once('-')
                    .map(|(bot, _)| bot.to_string())
            });
        let Some(bot_key) = bot else { continue };
        if let Some(messages) = value.get("messages").and_then(|item| item.as_array()) {
            for (index, item) in messages.iter().enumerate() {
                if let Some(message) = live_message(item, name, index) {
                    lines.push(LiveLine {
                        bot_key: bot_key.clone(),
                        message,
                    });
                }
            }
        } else if let Some(message) = live_message(&value, name, 0) {
            lines.push(LiveLine { bot_key, message });
        }
    }
    lines
}

fn live_message(value: &Value, file: &str, index: usize) -> Option<crate::model::StoredMessage> {
    let text = value
        .get("text")
        .and_then(|item| item.as_str())
        .map(str::trim)
        .filter(|text| !text.is_empty())?;
    let role = value
        .get("role")
        .and_then(|item| item.as_str())
        .map(str::trim)
        .filter(|role| !role.is_empty())
        .unwrap_or("assistant");
    let id = value
        .get("id")
        .and_then(|item| item.as_str())
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| format!("mirror-{file}-{index}"));
    let ts = value
        .get("ts")
        .and_then(|item| item.as_str())
        .map(str::to_string)
        .unwrap_or_else(crate::model::now_iso);
    Some(crate::model::StoredMessage {
        id,
        role: role.to_string(),
        text: text.to_string(),
        ts,
        read: false,
        in_reply_to: value
            .get("in_reply_to")
            .and_then(|item| item.as_str())
            .map(str::trim)
            .filter(|id| !id.is_empty())
            .map(str::to_string),
        remote_id: None,
    })
}

/// One JSON object per line: `{ "role", "text", "id", "ts" }`. The local window's own log.
pub fn read_window_log(path: &Path) -> Vec<crate::model::StoredMessage> {
    let Ok(text) = fs::read_to_string(path) else {
        return Vec::new();
    };
    text.lines()
        .enumerate()
        .filter_map(|(index, line)| {
            let value = serde_json::from_str::<Value>(line.trim()).ok()?;
            live_message(&value, "window", index)
        })
        .collect()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hyphenated_json_is_a_live_mirror() {
        assert!(is_live_mirror("atlas-grok.json"));
        assert!(is_live_mirror("products-1.json"));
        assert!(!is_live_mirror("one.json"));
        assert!(!is_live_mirror("atlas.json"));
        assert!(!is_live_mirror(".atlas-grok.json"));
        assert!(!is_live_mirror("atlas-grok.md"));
    }

    #[test]
    fn live_mirrors_keep_file_order_and_message_order() {
        let dir = std::env::temp_dir().join(format!("atlas-live-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("atlas-b.json"),
            r#"{"bot":"Atlas","messages":[{"id":"b1","role":"user","text":"second file"},{"id":"b2","role":"assistant","text":"still second","in_reply_to":"b1"}]}"#, 
        )
        .unwrap();
        fs::write(
            dir.join("atlas-a.json"),
            r#"{"text":"first file","id":"a1","role":"user"}"#, 
        )
        .unwrap();
        fs::write(dir.join("one.json"), r#"{"bot":"atlas","text":"archived later"}"#).unwrap();
        let lines = read_live_mirrors(&dir);
        let texts: Vec<_> = lines.iter().map(|line| line.message.text.as_str()).collect();
        assert_eq!(texts, ["first file", "second file", "still second"]);
        assert_eq!(lines[1].message.id, "b1");
        assert_eq!(lines[2].message.in_reply_to.as_deref(), Some("b1"));
        assert!(lines.iter().all(|line| line.bot_key.eq_ignore_ascii_case("atlas")));
        let pending = list_pending(&dir).unwrap();
        assert_eq!(pending.len(), 1);
        assert!(pending[0].path.ends_with("one.json"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn window_log_reads_every_line() {
        let dir = std::env::temp_dir().join(format!("atlas-window-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("local-atlas.jsonl");
        fs::write(
            &path,
            "{\"role\":\"user\",\"text\":\"hello\",\"id\":\"w1\"}\nnot json\n{\"role\":\"assistant\",\"text\":\"hi\",\"id\":\"w2\",\"in_reply_to\":\"w1\"}\n",
        )
        .unwrap();
        let lines = read_window_log(&path);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].text, "hello");
        assert_eq!(lines[1].in_reply_to.as_deref(), Some("w1"));
        let _ = fs::remove_dir_all(&dir);
    }
}
