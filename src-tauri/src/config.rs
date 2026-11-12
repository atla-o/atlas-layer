use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::history;
use crate::inbox;
use crate::model::{parse_bots, Bot};
use crate::secrets::env_nonempty;
use crate::todos;

const SEED_BOTS: &str = include_str!("../../assets/defaults/bots.json");
const SEED_CONFIG: &str = include_str!("../../assets/defaults/config.json");

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub office: OfficeCfg,
    pub features: FeaturesCfg,
    pub paths: PathsCfg,
    pub ollama: OllamaCfg,
    pub relay: RelayCfg,
    pub cursor: CursorCfg,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct FeaturesCfg {
    pub office: bool,
    pub filter: bool,
    pub shared_compute: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct OfficeCfg {
    pub title: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct PathsCfg {
    pub todo: String,
    pub today: String,
    pub personality: String,
    pub inbox_dir: String,
    pub inbox_file: String,
    pub history_dir: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct OllamaCfg {
    pub base_url: String,
    pub model: String,
    pub echo_if_unavailable: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct RelayCfg {
    pub url: String,
    pub url_env: String,
    pub auth_env: String,
    pub auth_keyring_account: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct CursorCfg {
    pub base_url: String,
    pub api_key_env: String,
    pub api_key_keyring_account: String,
    pub auth: String,
    pub poll_seconds: u64,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            office: OfficeCfg::default(),
            features: FeaturesCfg::default(),
            paths: PathsCfg::default(),
            ollama: OllamaCfg::default(),
            relay: RelayCfg::default(),
            cursor: CursorCfg::default(),
        }
    }
}

impl Default for FeaturesCfg {
    fn default() -> Self {
        Self {
            office: true,
            filter: false,
            shared_compute: false,
        }
    }
}

impl Default for OfficeCfg {
    fn default() -> Self {
        Self {
            title: "Atlas Layer".into(),
        }
    }
}

impl Default for PathsCfg {
    fn default() -> Self {
        Self {
            todo: "~/Atla/AI/todo.json".into(),
            today: "~/Atla/AI/atla-today.json".into(),
            personality: "~/Atla/AI/personality.md".into(),
            inbox_dir: "~/Atla/AI/office-inbox".into(),
            inbox_file: String::new(),
            history_dir: String::new(),
        }
    }
}

impl Default for OllamaCfg {
    fn default() -> Self {
        Self {
            base_url: "http://127.0.0.1:11434".into(),
            model: "qwen3:8b".into(),
            echo_if_unavailable: false,
        }
    }
}

impl Default for RelayCfg {
    fn default() -> Self {
        Self {
            url: String::new(),
            url_env: "ATLAS_LAYER_WEBHOOK_URL".into(),
            auth_env: "ATLAS_LAYER_WEBHOOK_AUTH".into(),
            auth_keyring_account: "webhook-auth".into(),
        }
    }
}

impl Default for CursorCfg {
    fn default() -> Self {
        Self {
            base_url: "https://api.cursor.com".into(),
            api_key_env: "CURSOR_API_KEY".into(),
            api_key_keyring_account: "cursor-api-key".into(),
            auth: "bearer".into(),
            poll_seconds: 60,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ResolvedPaths {
    pub todo: PathBuf,
    pub today: PathBuf,
    pub personality: PathBuf,
    pub inbox_dir: PathBuf,
    pub inbox_file: Option<PathBuf>,
    pub history: PathBuf,
}

pub struct OfficeData {
    pub config_dir: PathBuf,
    pub config_path: PathBuf,
    pub config_root: Value,
    pub config_writable: bool,
    pub config: AppConfig,
    pub bots_path: PathBuf,
    pub bots_root: Value,
    pub bots_writable: bool,
    pub bots: Vec<Bot>,
    pub warnings: Vec<String>,
    pub statuses: HashMap<String, String>,
    pub inflight: HashSet<String>,
    pub focus: Option<String>,
    pub matched_agents: HashMap<String, String>,
    pub paths: ResolvedPaths,
    pub jsonl_offset: u64,
    pub last_todo_sig: String,
    pub last_board_note: String,
}

pub fn resolve_config_dir() -> PathBuf {
    if let Some(raw) =
        env_nonempty("ATLAS_LAYER_CONFIG").or_else(|| env_nonempty("ATLA_OFFICE_CONFIG"))
    {
        let path = PathBuf::from(raw);
        if path.is_file() {
            return path.parent().unwrap_or(&path).to_path_buf();
        }
        return path;
    }
    let base = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    let dir = base.join("atlas-layer");
    let _ = migrate_legacy_config(&base.join("atla-office"), &dir);
    dir
}

/// Copies `config.json` (and `bots.json`, when present) from an older
/// `atla-office` directory into `atlas-layer` once. An existing new
/// `config.json` is left alone, so a later edit is not overwritten.
/// Returns whether a config file was copied.
pub fn migrate_legacy_config(legacy_dir: &Path, new_dir: &Path) -> bool {
    let new_cfg = new_dir.join("config.json");
    let old_cfg = legacy_dir.join("config.json");
    if new_cfg.exists() || !old_cfg.is_file() {
        return false;
    }
    if fs::create_dir_all(new_dir).is_err() {
        return false;
    }
    if fs::copy(&old_cfg, &new_cfg).is_err() {
        return false;
    }
    let new_bots = new_dir.join("bots.json");
    let old_bots = legacy_dir.join("bots.json");
    if !new_bots.exists() && old_bots.is_file() {
        let _ = fs::copy(&old_bots, &new_bots);
    }
    true
}

/// Chat history stays in the old folder when that is the only copy, so a
/// rename of the app does not hide existing desks' messages.
pub fn default_history_dir(data_root: &Path) -> PathBuf {
    let fresh = data_root.join("atlas-layer").join("history");
    let legacy = data_root.join("atla-office").join("history");
    if !fresh.exists() && legacy.exists() {
        legacy
    } else {
        fresh
    }
}

pub fn load_office(config_dir: &Path) -> OfficeData {
    let _ = fs::create_dir_all(config_dir);
    let mut warnings = Vec::new();
    let config_path = config_dir.join("config.json");
    let (config, config_root, config_writable) = load_config(&config_path, &mut warnings);
    let paths = resolve_paths(config_dir, &config);
    let _ = fs::create_dir_all(&paths.history);
    let _ = fs::create_dir_all(&paths.inbox_dir);
    let bots_path = config_dir.join("bots.json");
    let (bots_root, bots, bots_writable, bot_warnings) = load_bots(&bots_path);
    warnings.extend(bot_warnings);

    let mut statuses = HashMap::new();
    for bot in &bots {
        let unread = history::path_for(&paths.history, &bot.id)
            .map(|path| history::has_unread(&path))
            .unwrap_or(false);
        statuses.insert(
            bot.id.clone(),
            if unread {
                "message".into()
            } else {
                "idle".into()
            },
        );
    }
    let jsonl_offset = paths
        .inbox_file
        .as_ref()
        .map(|file| inbox::load_offset(&paths.history, file))
        .unwrap_or(0);
    let last_todo_sig = todos::view(&paths.todo, &paths.today)
        .map(|view| view.signature())
        .unwrap_or_default();

    eprintln!(
        "[atlas-layer] config {} \u00b7 {} desks",
        config_dir.display(),
        bots.len()
    );

    OfficeData {
        config_dir: config_dir.to_path_buf(),
        config_path,
        config_root,
        config_writable,
        config,
        bots_path,
        bots_root,
        bots_writable,
        bots,
        warnings,
        statuses,
        inflight: HashSet::new(),
        focus: None,
        matched_agents: HashMap::new(),
        paths,
        jsonl_offset,
        last_todo_sig,
        last_board_note: String::new(),
    }
}

fn load_config(path: &Path, warnings: &mut Vec<String>) -> (AppConfig, Value, bool) {
    let fallback = || {
        let root: Value = serde_json::from_str(SEED_CONFIG).expect("embedded config");
        let config = serde_json::from_value(root.clone()).unwrap_or_default();
        (config, root)
    };
    if !path.exists() {
        let (config, root) = fallback();
        let mut body =
            serde_json::to_string_pretty(&root).unwrap_or_else(|_| SEED_CONFIG.to_string());
        if !body.ends_with('\n') {
            body.push('\n');
        }
        if let Err(err) = fs::write(path, body) {
            warnings.push(format!(
                "Could not write a starter config ({err}). Using defaults in memory."
            ));
            return (config, root, false);
        }
        return (config, root, true);
    }
    match fs::read_to_string(path) {
        Ok(text) => match serde_json::from_str::<Value>(&text) {
            Ok(root) => match serde_json::from_value::<AppConfig>(root.clone()) {
                Ok(config) => (config, root, true),
                Err(err) => {
                    warnings.push(format!(
                        "config.json could not be read ({err}). Using defaults, and the file was not changed."
                    ));
                    let (config, root) = fallback();
                    (config, root, false)
                }
            },
            Err(err) => {
                warnings.push(format!(
                    "config.json could not be read ({err}). Using defaults, and the file was not changed."
                ));
                let (config, root) = fallback();
                (config, root, false)
            }
        },
        Err(err) => {
            warnings.push(format!(
                "config.json could not be read ({err}). Using defaults."
            ));
            let (config, root) = fallback();
            (config, root, false)
        }
    }
}

pub fn feature_key(name: &str) -> Result<&'static str, String> {
    match name {
        "office" => Ok("office"),
        "filter" => Ok("filter"),
        "sharedCompute" | "shared-compute" | "compute" => Ok("sharedCompute"),
        other => Err(format!("Unknown feature {other}.")),
    }
}

/// Sets one switch on the raw config document. Other keys, including ones this
/// app does not know, stay where they are.
pub fn set_feature_flag(
    root: &mut Value,
    feature: &str,
    enabled: bool,
    current: &FeaturesCfg,
) -> Result<(), String> {
    let key = feature_key(feature)?;
    if !root.is_object() {
        *root = serde_json::json!({});
    }
    let obj = root.as_object_mut().expect("config object");
    let features = obj
        .entry("features")
        .or_insert_with(|| serde_json::json!({}));
    if !features.is_object() {
        *features = serde_json::json!({});
    }
    let map = features.as_object_mut().expect("features object");
    if !map.contains_key("office") {
        map.insert("office".into(), Value::Bool(current.office));
    }
    if !map.contains_key("filter") {
        map.insert("filter".into(), Value::Bool(current.filter));
    }
    if !map.contains_key("sharedCompute") {
        map.insert("sharedCompute".into(), Value::Bool(current.shared_compute));
    }
    map.insert(key.into(), Value::Bool(enabled));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_features_leave_the_office_on() {
        let config: AppConfig =
            serde_json::from_str(r#"{"office":{"title":"Atlas Layer"}}"#).unwrap();
        assert!(config.features.office);
        assert!(!config.features.filter);
        assert!(!config.features.shared_compute);
    }

    #[test]
    fn a_switch_keeps_unknown_config_fields() {
        let mut root = serde_json::json!({
            "division": "creation",
            "office": { "title": "Atlas Layer", "note": "keep" },
            "paths": { "todo": "todo.json" }
        });
        set_feature_flag(&mut root, "sharedCompute", true, &FeaturesCfg::default()).unwrap();
        assert_eq!(root["division"], "creation");
        assert_eq!(root["office"]["note"], "keep");
        assert_eq!(root["paths"]["todo"], "todo.json");
        assert_eq!(root["features"]["office"], true);
        assert_eq!(root["features"]["filter"], false);
        assert_eq!(root["features"]["sharedCompute"], true);
        let config: AppConfig = serde_json::from_value(root).unwrap();
        assert!(config.features.shared_compute);
        assert!(config.features.office);
        assert!(!config.features.filter);
    }

    #[test]
    fn legacy_config_carries_toggles_once() {
        let root = tempfile::tempdir().unwrap();
        let legacy = root.path().join("atla-office");
        let fresh = root.path().join("atlas-layer");
        fs::create_dir_all(&legacy).unwrap();
        fs::write(
            legacy.join("config.json"),
            r#"{
  "division": "creation",
  "features": { "office": false, "filter": true, "sharedCompute": true }
}
"#,
        )
        .unwrap();
        fs::write(legacy.join("bots.json"), r#"{"bots":[]}"#).unwrap();

        assert!(migrate_legacy_config(&legacy, &fresh));
        let carried: Value =
            serde_json::from_str(&fs::read_to_string(fresh.join("config.json")).unwrap()).unwrap();
        assert_eq!(carried["division"], "creation");
        assert_eq!(carried["features"]["office"], false);
        assert_eq!(carried["features"]["filter"], true);
        assert_eq!(carried["features"]["sharedCompute"], true);
        assert_eq!(
            fs::read_to_string(fresh.join("bots.json")).unwrap(),
            r#"{"bots":[]}"#
        );

        fs::write(
            legacy.join("config.json"),
            r#"{"features":{"office":true}}"#,
        )
        .unwrap();
        assert!(!migrate_legacy_config(&legacy, &fresh));
        let still: Value =
            serde_json::from_str(&fs::read_to_string(fresh.join("config.json")).unwrap()).unwrap();
        assert_eq!(still["features"]["office"], false);
    }

    #[test]
    fn history_stays_on_the_old_folder_until_a_new_one_exists() {
        let root = tempfile::tempdir().unwrap();
        let legacy = root.path().join("atla-office").join("history");
        fs::create_dir_all(&legacy).unwrap();
        assert_eq!(default_history_dir(root.path()), legacy);
        let fresh = root.path().join("atlas-layer").join("history");
        fs::create_dir_all(&fresh).unwrap();
        assert_eq!(default_history_dir(root.path()), fresh);
    }
}

pub fn save_config(path: &Path, root: &Value) -> Result<(), String> {
    let mut pretty = serde_json::to_string_pretty(root).map_err(|err| err.to_string())?;
    if !pretty.ends_with('\n') {
        pretty.push('\n');
    }
    crate::todos::atomic_write(path, &pretty)
}

fn load_bots(path: &Path) -> (Value, Vec<Bot>, bool, Vec<String>) {
    let seed: Value = serde_json::from_str(SEED_BOTS).expect("embedded bots");
    let (seed_bots, _) = parse_bots(&seed);
    if !path.exists() {
        let pretty = serde_json::to_string_pretty(&seed).unwrap_or_else(|_| SEED_BOTS.to_string());
        let mut body = pretty;
        if !body.ends_with('\n') {
            body.push('\n');
        }
        if let Err(err) = fs::write(path, body) {
            return (
                seed,
                seed_bots,
                false,
                vec![format!(
                    "Could not write bots.json ({err}). Showing the seed list."
                )],
            );
        }
        return (seed, seed_bots, true, Vec::new());
    }
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) => {
            return (
                seed,
                seed_bots,
                false,
                vec![format!(
                    "bots.json could not be read ({err}). Showing the seed list, and the file was not changed."
                )],
            );
        }
    };
    let value: Value = match serde_json::from_str(&text) {
        Ok(value) => value,
        Err(err) => {
            return (
                seed,
                seed_bots,
                false,
                vec![format!(
                    "bots.json is not JSON ({err}). Showing the seed list, and the file was not changed."
                )],
            );
        }
    };
    let (bots, mut warnings) = parse_bots(&value);
    if bots.is_empty() {
        warnings.push(
            "bots.json has no usable desks. Showing the seed list, and the file was not changed."
                .into(),
        );
        return (seed, seed_bots, false, warnings);
    }
    (value, bots, true, warnings)
}

pub fn resolve_paths(config_dir: &Path, config: &AppConfig) -> ResolvedPaths {
    let history = if config.paths.history_dir.trim().is_empty() {
        default_history_dir(&dirs::data_dir().unwrap_or_else(|| PathBuf::from(".")))
    } else {
        resolve_against(config_dir, &config.paths.history_dir)
    };
    let inbox_file = {
        let raw = config.paths.inbox_file.trim();
        if raw.is_empty() {
            None
        } else {
            Some(resolve_against(config_dir, raw))
        }
    };
    ResolvedPaths {
        todo: resolve_against(config_dir, &config.paths.todo),
        today: resolve_against(config_dir, &config.paths.today),
        personality: resolve_against(config_dir, &config.paths.personality),
        inbox_dir: resolve_against(config_dir, &config.paths.inbox_dir),
        inbox_file,
        history,
    }
}

pub fn resolve_against(config_dir: &Path, raw: &str) -> PathBuf {
    let expanded = expand_tilde(raw.trim());
    if expanded.as_os_str().is_empty() {
        return config_dir.to_path_buf();
    }
    if expanded.is_absolute() {
        expanded
    } else {
        config_dir.join(expanded)
    }
}

pub fn expand_tilde(raw: &str) -> PathBuf {
    if raw == "~" {
        return dirs::home_dir().unwrap_or_else(|| PathBuf::from(raw));
    }
    if let Some(rest) = raw.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest);
        }
    }
    PathBuf::from(raw)
}

pub fn ollama_base(config: &AppConfig) -> String {
    if let Some(host) = env_nonempty("OLLAMA_HOST") {
        if host.starts_with("http://") || host.starts_with("https://") {
            return host;
        }
        return format!("http://{host}");
    }
    config.ollama.base_url.clone()
}

pub fn relay_url(config: &AppConfig) -> String {
    if let Some(url) = env_nonempty(&config.relay.url_env) {
        return url;
    }
    config.relay.url.trim().to_string()
}

pub fn read_prompt(path: &Path) -> Option<String> {
    let text = fs::read_to_string(path).ok()?;
    let trimmed = text.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.chars().take(32_000).collect())
    }
}

pub fn effective_agent<'a>(data: &'a OfficeData, bot: &Bot) -> Option<String> {
    if let Some(pinned) = bot.pinned_agent() {
        return Some(pinned.to_string());
    }
    data.matched_agents.get(&bot.id).cloned()
}
