use serde_json::Value;

use crate::secrets::scrub;

use super::{ensure_http_url, host_only, BotConnector, DispatchOutcome, Outbound, TokenTx};

pub struct RelayConnector {
    pub url: String,
    pub auth: Option<String>,
    pub client: reqwest::Client,
}

pub fn relay_body(bot: &str, text: &str, reply_to_id: &str) -> Value {
    serde_json::json!({
        "bot": bot,
        "text": text,
        "reply_to_id": reply_to_id,
    })
}

/// A value with no space is sent as a bearer token. `Bearer …` and `Basic …` are sent as written.
pub fn normalize_auth_header(value: &str) -> String {
    let value = value.trim();
    if value.is_empty() {
        String::new()
    } else if value.contains(' ') {
        value.to_string()
    } else {
        format!("Bearer {value}")
    }
}

impl BotConnector for RelayConnector {
    fn kind(&self) -> &'static str {
        "relay-webhook"
    }

    async fn dispatch(&self, req: Outbound, _tokens: &TokenTx) -> Result<DispatchOutcome, String> {
        ensure_http_url(&self.url)?;
        let body = relay_body(&req.bot_id, &req.text, &req.message_id);
        let mut builder = self.client.post(&self.url).json(&body);
        let raw_secret = self.auth.clone().unwrap_or_default();
        let header = normalize_auth_header(&raw_secret);
        if !header.is_empty() {
            builder = builder.header("Authorization", &header);
        }
        let response = builder
            .send()
            .await
            .map_err(|_| format!("The relay at {} did not answer.", host_only(&self.url)))?;
        if !response.status().is_success() {
            let status = response.status();
            let detail = response.text().await.unwrap_or_default();
            let snippet: String = detail.chars().take(160).collect();
            let message = scrub(&header, &format!("The relay returned {status}. {snippet}"));
            return Err(scrub(&raw_secret, &message));
        }
        Ok(DispatchOutcome::Accepted)
    }
}
