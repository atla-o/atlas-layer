use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use super::{BotConnector, DispatchOutcome, Outbound, TokenTx};

pub struct LocalWindowConnector {
    pub inbox_dir: PathBuf,
    pub launch_command: String,
    pub home: PathBuf,
}

pub fn prompt_name(message_id: &str) -> Result<String, String> {
    let id = message_id.trim();
    if id.is_empty()
        || id.len() > 80
        || id.contains("..")
        || !id
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
    {
        return Err("That message id cannot be used as a filename.".into());
    }
    Ok(format!("layer-{id}.md"))
}

/// Writes the message text where atlas-window looks. The `.tmp` name is not
/// `*.md`, so the window does not read a half-written file.
pub fn write_prompt(dir: &Path, message_id: &str, text: &str) -> Result<PathBuf, String> {
    let name = prompt_name(message_id)?;
    fs::create_dir_all(dir).map_err(|err| err.to_string())?;
    let dest = dir.join(&name);
    let tmp = dir.join(format!("{name}.tmp"));
    let mut body = text.trim_end().to_string();
    body.push('\n');
    fs::write(&tmp, body).map_err(|err| err.to_string())?;
    fs::rename(&tmp, &dest).map_err(|err| err.to_string())?;
    Ok(dest)
}

/// A custom `launchCommand` is used as-is. Otherwise try the desktop entry,
/// then `~/bin/atlas-window`.
pub fn launch_commands(launch_command: &str, home: &Path) -> Vec<Vec<String>> {
    let custom = launch_command.trim();
    if !custom.is_empty() {
        return vec![custom.split_whitespace().map(str::to_string).collect()];
    }
    vec![
        vec!["gtk-launch".into(), "atlas-window".into()],
        vec![home
            .join("bin")
            .join("atlas-window")
            .to_string_lossy()
            .into_owned()],
    ]
}

pub fn process_running(name: &str) -> bool {
    Command::new("pgrep")
        .arg("-x")
        .arg(name)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn spawn_detached(argv: &[String]) -> Result<(), String> {
    if argv.first().is_none_or(|program| program.is_empty()) {
        return Err("The launch command is empty.".into());
    }
    let status = Command::new("setsid")
        .arg("-f")
        .args(argv)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|err| format!("Could not start {}: {err}", argv[0]))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("Could not start {} ({status}).", argv[0]))
    }
}

pub fn wake_with(
    commands: &[Vec<String>],
    running: bool,
    mut spawn: impl FnMut(&[String]) -> Result<(), String>,
) -> Result<bool, String> {
    if running {
        return Ok(false);
    }
    let mut last = "Could not start atlas-window.".to_string();
    for command in commands {
        match spawn(command) {
            Ok(()) => return Ok(true),
            Err(err) => last = err,
        }
    }
    Err(last)
}

pub fn wake(launch_command: &str, home: &Path) -> Result<(), String> {
    wake_with(
        &launch_commands(launch_command, home),
        process_running("atlas-window"),
        spawn_detached,
    )
    .map(|_| ())
}

impl BotConnector for LocalWindowConnector {
    fn kind(&self) -> &'static str {
        "local-window"
    }

    async fn dispatch(&self, req: Outbound, _tokens: &TokenTx) -> Result<DispatchOutcome, String> {
        write_prompt(&self.inbox_dir, &req.message_id, &req.text)?;
        wake(&self.launch_command, &self.home)?;
        Ok(DispatchOutcome::Accepted)
    }
}
