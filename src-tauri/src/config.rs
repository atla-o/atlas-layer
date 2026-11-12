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
        "[atlas-layer] config {} \u{b7} {} desks",
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
