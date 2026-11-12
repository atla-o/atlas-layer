use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::model::{safe_bot_id, StoredMessage, TranscriptLine};

static HISTORY_IO: Mutex<()> = Mutex::new(());

fn lock() -> std::sync::MutexGuard<'static, ()> {
    HISTORY_IO.lock().unwrap_or_else(|err| err.into_inner())
}

pub fn path_for(dir: &Path, bot_id: &str) -> Result<PathBuf, String> {
    let id = safe_bot_id(bot_id).ok_or_else(|| "Unknown bot".to_string())?;
    Ok(dir.join(format!("{id}.jsonl")))
}

pub fn read_all(path: &Path) -> Vec<StoredMessage> {
    let _guard = lock();
    read_all_locked(path)
}

fn read_all_locked(path: &Path) -> Vec<StoredMessage> {
    let Ok(text) = fs::read_to_string(path) else {
        return Vec::new();
    };
    text.lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                None
            } else {
                serde_json::from_str(trimmed).ok()
            }
        })
        .collect()
}

pub fn has_unread(path: &Path) -> bool {
    read_all(path)
        .iter()
        .any(|message| message.role == "assistant" && !message.read)
}

pub fn append(path: &Path, message: &StoredMessage) -> Result<(), String> {
    let _guard = lock();
    append_locked(path, message)
}

fn append_locked(path: &Path, message: &StoredMessage) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|err| err.to_string())?;
    let line = serde_json::to_string(message).map_err(|err| err.to_string())?;
    writeln!(file, "{line}").map_err(|err| err.to_string())?;
    Ok(())
}

/// Appends unless a message with the same id is already stored.
pub fn append_unique(path: &Path, message: &StoredMessage) -> Result<bool, String> {
    let _guard = lock();
    if read_all_locked(path)
        .iter()
        .any(|existing| existing.id == message.id)
    {
        return Ok(false);
    }
    append_locked(path, message)?;
    Ok(true)
}

pub fn mark_read(path: &Path) -> Result<(), String> {
    let _guard = lock();
    if !path.exists() {
        return Ok(());
    }
    let mut messages = read_all_locked(path);
    let mut changed = false;
    for message in &mut messages {
        if !message.read {
            message.read = true;
            changed = true;
        }
    }
    if !changed {
        return Ok(());
    }
    rewrite_locked(path, &messages)
}

pub fn import_remote(path: &Path, lines: &[TranscriptLine]) -> Result<usize, String> {
    let _guard = lock();
    let existing = read_all_locked(path);
    let mut known: std::collections::HashSet<String> = existing
        .iter()
        .filter_map(|message| message.remote_id.clone())
        .collect();
    let mut added = 0;
    for line in lines {
        if line.text.trim().is_empty() || !known.insert(line.remote_id.clone()) {
            continue;
        }
        let message = StoredMessage {
            id: format!("remote-{}", line.remote_id),
            role: line.role.clone(),
            text: line.text.clone(),
            ts: line.ts.clone().unwrap_or_else(crate::model::now_iso),
            read: true,
            in_reply_to: None,
            remote_id: Some(line.remote_id.clone()),
        };
        append_locked(path, &message)?;
        added += 1;
    }
    Ok(added)
}

fn rewrite_locked(path: &Path, messages: &[StoredMessage]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    let tmp = path.with_extension("jsonl.tmp");
    let mut body = String::new();
    for message in messages {
        body.push_str(&serde_json::to_string(message).map_err(|err| err.to_string())?);
        body.push('\n');
    }
    fs::write(&tmp, body).map_err(|err| err.to_string())?;
    fs::rename(&tmp, path).map_err(|err| err.to_string())?;
    Ok(())
}
