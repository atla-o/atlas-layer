use futures_util::StreamExt;
use serde_json::Value;

use super::echo::EchoConnector;
use super::{ensure_http_url, host_only, BotConnector, DispatchOutcome, Outbound, TokenTx};

pub struct OllamaConnector {
    pub base_url: String,
    pub model: String,
    pub echo_if_unavailable: bool,
    pub client: reqwest::Client,
}

#[derive(Default)]
pub struct OllamaAccum {
    pub content: String,
    pub thinking: String,
    pub done: bool,
}

pub fn consume_ollama_buffer(
    buf: &mut String,
    acc: &mut OllamaAccum,
) -> Result<Vec<String>, String> {
    let mut tokens = Vec::new();
    while let Some(nl) = buf.find('\n') {
        let line = buf[..nl].trim().to_string();
        buf.drain(..=nl);
        if line.is_empty() {
            continue;
        }
        let parsed: Value = serde_json::from_str(&line)
            .map_err(|_| "Ollama sent a line that wasn't JSON".to_string())?;
        if let Some(text) = parsed.pointer("/message/thinking").and_then(|v| v.as_str()) {
            acc.thinking.push_str(text);
        }
        if let Some(text) = parsed.pointer("/message/content").and_then(|v| v.as_str()) {
            if !text.is_empty() {
                acc.content.push_str(text);
                tokens.push(text.to_string());
            }
        }
        if parsed.get("done").and_then(|v| v.as_bool()) == Some(true) {
            acc.done = true;
        }
    }
    Ok(tokens)
}

impl BotConnector for OllamaConnector {
    fn kind(&self) -> &'static str {
        "local-ollama"
    }

    async fn dispatch(&self, req: Outbound, tokens: &TokenTx) -> Result<DispatchOutcome, String> {
        let url = format!("{}/api/chat", self.base_url.trim_end_matches('/'));
        ensure_http_url(&self.base_url)?;
        let body = serde_json::json!({
            "model": self.model,
            "messages": req.messages,
            "stream": true,
        });
        let response = match self.client.post(&url).json(&body).send().await {
            Ok(response) => response,
            Err(err) => {
                if self.echo_if_unavailable && (err.is_connect() || err.is_timeout()) {
                    let reply = EchoConnector::stream(&req.bot_name, &req.text, tokens).await;
                    return Ok(DispatchOutcome::Reply(reply));
                }
                let why = if err.is_connect() {
                    "connection refused"
                } else if err.is_timeout() {
                    "timed out"
                } else {
                    "request failed"
                };
                return Err(format!("Ollama at {} {why}.", host_only(&self.base_url)));
            }
        };
        if !response.status().is_success() {
            return Err(format!("Ollama returned {}.", response.status()));
        }
        let mut stream = response.bytes_stream();
        let mut buf = String::new();
        let mut acc = OllamaAccum::default();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|_| "The Ollama stream broke.".to_string())?;
            let text = String::from_utf8_lossy(&chunk).replace("\r\n", "\n");
            buf.push_str(&text);
            for token in consume_ollama_buffer(&mut buf, &mut acc)? {
                tokens.push(&token);
            }
        }
        if !buf.trim().is_empty() {
            buf.push('\n');
            for token in consume_ollama_buffer(&mut buf, &mut acc)? {
                tokens.push(&token);
            }
        }
        if acc.content.is_empty() && !acc.thinking.is_empty() {
            tokens.push(&acc.thinking);
            acc.content.clone_from(&acc.thinking);
        }
        if acc.content.is_empty() {
            return Err("Ollama returned an empty reply.".into());
        }
        Ok(DispatchOutcome::Reply(acc.content))
    }
}
