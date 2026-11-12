use serde_json::Value;

use crate::model::StoredMessage;

pub mod cursor;
pub mod echo;
pub mod ollama;
pub mod relay;

#[derive(Debug)]
pub enum DispatchOutcome {
    Reply(String),
    Accepted,
}

#[derive(Clone)]
pub struct TokenTx {
    tx: tokio::sync::mpsc::UnboundedSender<String>,
}

impl TokenTx {
    pub fn new(tx: tokio::sync::mpsc::UnboundedSender<String>) -> Self {
        Self { tx }
    }

    pub fn push(&self, token: &str) {
        if !token.is_empty() {
            let _ = self.tx.send(token.to_string());
        }
    }
}

pub struct Outbound {
    pub bot_id: String,
    pub bot_name: String,
    pub text: String,
    pub message_id: String,
    pub messages: Vec<Value>,
}

/// One adapter per desk. The runtime picks an adapter from the bot's connector field.
pub trait BotConnector: Send + Sync {
    fn kind(&self) -> &'static str;
    #[allow(async_fn_in_trait)]
    async fn dispatch(&self, req: Outbound, tokens: &TokenTx) -> Result<DispatchOutcome, String>;
}

pub enum Connector {
    Ollama(ollama::OllamaConnector),
    Relay(relay::RelayConnector),
    Cursor(cursor::CursorConnector),
    Echo(echo::EchoConnector),
}

impl Connector {
    pub async fn dispatch(
        &self,
        req: Outbound,
        tokens: &TokenTx,
    ) -> Result<DispatchOutcome, String> {
        match self {
            Self::Ollama(connector) => connector.dispatch(req, tokens).await,
            Self::Relay(connector) => connector.dispatch(req, tokens).await,
            Self::Cursor(connector) => connector.dispatch(req, tokens).await,
            Self::Echo(connector) => connector.dispatch(req, tokens).await,
        }
    }
}

pub fn build_ollama_messages(
    system: Option<&str>,
    history: &[StoredMessage],
    limit: usize,
) -> Vec<Value> {
    let mut messages = Vec::new();
    if let Some(prompt) = system {
        messages.push(serde_json::json!({"role": "system", "content": prompt}));
    }
    let relevant: Vec<&StoredMessage> = history
        .iter()
        .filter(|message| message.role == "user" || message.role == "assistant")
        .collect();
    let start = relevant.len().saturating_sub(limit);
    for message in &relevant[start..] {
        messages.push(serde_json::json!({
            "role": message.role,
            "content": message.text,
        }));
    }
    messages
}

pub fn ensure_http_url(url: &str) -> Result<(), String> {
    let parsed = reqwest::Url::parse(url).map_err(|_| "That URL is not valid.".to_string())?;
    match parsed.scheme() {
        "http" | "https" => Ok(()),
        _ => Err("Only http and https URLs are allowed.".into()),
    }
}

pub fn host_only(url: &str) -> String {
    reqwest::Url::parse(url)
        .ok()
        .and_then(|parsed| parsed.host_str().map(str::to_string))
        .unwrap_or_else(|| "the configured host".into())
}
