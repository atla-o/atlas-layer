use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const ACCENTS: [&str; 8] = [
    "#c45c3e", "#d08a3a", "#8d5a3a", "#c47b86", "#6e5a4a", "#7f9474", "#b86a3a", "#4a3b32",
];
pub const HAIRS: [&str; 4] = ["#241c17", "#5c3a28", "#3a2428", "#3e5340"];

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Bot {
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub role: String,
    pub connector: String,
    #[serde(default)]
    pub accent: String,
    #[serde(default)]
    pub hair: String,
    #[serde(default)]
    pub accessory: String,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub agent_id: Option<String>,
    #[serde(default)]
    pub system_prompt: Option<String>,
    #[serde(default)]
    pub links: DeskLinks,
}

/// Optional things on the desk that open a page or a file. Empty fields are not clickable.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct DeskLinks {
    pub papers: Option<String>,
    pub drive: Option<String>,
    pub mug: Option<String>,
    pub computer: Option<String>,
}

impl Bot {
    pub fn kind(&self) -> Option<&'static str> {
        match self.connector.as_str() {
            "local-ollama" => Some("local-ollama"),
            "local-window" => Some("local-window"),
            "relay-webhook" => Some("relay-webhook"),
            "cursor-agents" => Some("cursor-agents"),
            "echo" => Some("echo"),
            _ => None,
        }
    }

    pub fn pinned_agent(&self) -> Option<&str> {
        self.agent_id
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BotDto {
    pub id: String,
    pub name: String,
    pub role: String,
    pub connector: String,
    pub accent: String,
    pub hair: String,
    pub accessory: String,
    pub model: Option<String>,
    pub agent_id: String,
    pub agent_pinned: bool,
    pub links: DeskLinks,
}

pub fn present_bot(bot: &Bot, index: usize, matched: Option<&str>) -> BotDto {
    let accent = if bot.accent.trim().is_empty() {
        ACCENTS[index % ACCENTS.len()].to_string()
    } else {
        bot.accent.clone()
    };
    let hair = if bot.hair.trim().is_empty() {
        HAIRS[index % HAIRS.len()].to_string()
    } else {
        bot.hair.clone()
    };
    let pinned = bot.pinned_agent().map(|s| s.to_string());
    let agent_id = pinned
        .clone()
        .or_else(|| matched.map(|s| s.to_string()))
        .unwrap_or_default();
    BotDto {
        id: bot.id.clone(),
        name: if bot.name.trim().is_empty() {
            bot.id.clone()
        } else {
            bot.name.clone()
        },
        role: if bot.role.trim().is_empty() {
            "Desk".into()
        } else {
            bot.role.clone()
        },
        connector: bot.connector.clone(),
        accent,
        hair,
        accessory: if bot.accessory.trim().is_empty() {
            "none".into()
        } else {
            bot.accessory.clone()
        },
        model: bot.model.clone().filter(|m| !m.trim().is_empty()),
        agent_id,
        agent_pinned: pinned.is_some(),
        links: tidy_links(&bot.links),
    }
}

fn tidy_links(links: &DeskLinks) -> DeskLinks {
    DeskLinks {
        papers: nonempty(links.papers.as_deref()),
        drive: nonempty(links.drive.as_deref()),
        mug: nonempty(links.mug.as_deref()),
        computer: nonempty(links.computer.as_deref()),
    }
}

fn nonempty(raw: Option<&str>) -> Option<String> {
    raw.map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_string)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredMessage {
    pub id: String,
    pub role: String,
    pub text: String,
    pub ts: String,
    #[serde(default)]
    pub read: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub in_reply_to: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TodoDto {
    pub id: String,
    pub text: String,
    pub done: bool,
    pub source: String,
    pub today: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MetricDto {
    pub label: String,
    pub value: String,
    pub bad: bool,
}

#[derive(Debug, Clone)]
pub struct TranscriptLine {
    pub remote_id: String,
    pub role: String,
    pub text: String,
    pub ts: Option<String>,
}

pub fn safe_bot_id(id: &str) -> Option<&str> {
    let id = id.trim();
    if id.is_empty() || id.len() > 64 {
        return None;
    }
    if id
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        Some(id)
    } else {
        None
    }
}

pub fn safe_agent_id(id: &str) -> Option<&str> {
    let id = id.trim();
    if id.is_empty() || id.len() > 128 {
        return None;
    }
    if id
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        Some(id)
    } else {
        None
    }
}

pub fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

pub fn new_id() -> String {
    format!("m-{}", uuid::Uuid::new_v4())
}

pub fn parse_bots(root: &Value) -> (Vec<Bot>, Vec<String>) {
    let Some(arr) = root
        .get("bots")
        .and_then(|v| v.as_array())
        .or_else(|| root.as_array())
    else {
        return (Vec::new(), vec!["bots.json needs a bots array.".into()]);
    };
    let mut bots = Vec::new();
    let mut warnings = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for (i, item) in arr.iter().enumerate() {
        let mut bot: Bot = match serde_json::from_value(item.clone()) {
            Ok(bot) => bot,
            Err(err) => {
                warnings.push(format!("Skipped bot {i}: {err}"));
                continue;
            }
        };
        bot.id = bot.id.trim().to_string();
        if safe_bot_id(&bot.id).is_none() {
            warnings.push(format!(
                "Skipped bot {i}: id must be letters, numbers, hyphens, or underscores."
            ));
            continue;
        }
        if !seen.insert(bot.id.clone()) {
            warnings.push(format!("Skipped duplicate bot {}.", bot.id));
            continue;
        }
        if bot.kind().is_none() {
            warnings.push(format!(
                "Skipped {}: unknown connector {}.",
                bot.id, bot.connector
            ));
            continue;
        }
        if bot.name.trim().is_empty() {
            bot.name = bot.id.clone();
        }
        if bot.role.trim().is_empty() {
            bot.role = "Desk".into();
        }
        if bot.pinned_agent().is_none() {
            bot.agent_id = None;
        }
        bots.push(bot);
    }
    (bots, warnings)
}

pub fn connector_label(kind: &str) -> &'static str {
    match kind {
        "local-ollama" => "Local model",
        "local-window" => "Local window",
        "relay-webhook" => "Cloud relay",
        "cursor-agents" => "Cursor agent",
        "echo" => "Echo",
        _ => "Desk",
    }
}
