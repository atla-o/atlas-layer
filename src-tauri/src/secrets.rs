use std::io::Read;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

pub const SERVICE: &str = "atlas-layer";
const LEGACY_SERVICE: &str = "atla-office";

pub fn env_nonempty(name: &str) -> Option<String> {
    let raw = std::env::var(name).ok()?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

/// Env wins. The keyring is only consulted when the env var is empty.
/// The looked-up value is returned to the caller and must not be logged.
pub fn resolve_secret(env_var: &str, keyring_account: &str) -> Option<String> {
    if let Some(value) = env_nonempty(env_var) {
        return Some(value);
    }
    let account = keyring_account.trim();
    if account.is_empty() {
        return None;
    }
    keyring_lookup(SERVICE, account).or_else(|| keyring_lookup(LEGACY_SERVICE, account))
}

pub fn scrub(secret: &str, text: &str) -> String {
    if secret.len() < 6 {
        return text.to_string();
    }
    text.replace(secret, "[redacted]")
}

fn keyring_lookup(service: &str, account: &str) -> Option<String> {
    let mut child = Command::new("secret-tool")
        .args(["lookup", "service", service, "account", account])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let started = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if started.elapsed() > Duration::from_secs(2) => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(40)),
            Err(_) => return None,
        }
    };
    if !status.success() {
        return None;
    }
    let mut out = String::new();
    child.stdout.as_mut()?.read_to_string(&mut out).ok()?;
    let trimmed = out.trim_end_matches(['\n', '\r']);
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

pub fn echo_forced() -> bool {
    ["ATLAS_LAYER_ECHO", "ATLA_OFFICE_ECHO"]
        .into_iter()
        .any(|name| {
            matches!(
                std::env::var(name).ok().as_deref(),
                Some("1") | Some("true") | Some("yes")
            )
        })
}
