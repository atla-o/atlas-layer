use futures_util::StreamExt;
use serde::Serialize;
use serde_json::Value;

use crate::model::{safe_agent_id, TranscriptLine};
use crate::secrets::scrub;

use super::{ensure_http_url, BotConnector, DispatchOutcome, Outbound, TokenTx};

pub struct CursorConnector {
    pub base_url: String,
    pub api_key: String,
    pub auth_style: String,
    pub agent_id: String,
    pub client: reqwest::Client,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteAgent {
    pub id: String,
    pub name: String,
    pub status: String,
    pub url: String,
}

#[derive(Debug)]
pub struct SseEvent {
    pub event: String,
    pub data: String,
}

pub fn map_agent_status(status: &str) -> &'static str {
    match status {
        "ACTIVE" | "RUNNING" | "CREATING" | "WAITING_FOR_BACKGROUND_WORK" => "working",
        _ => "idle",
    }
}

pub fn parse_agent_list(value: &Value) -> Vec<RemoteAgent> {
    let Some(items) = value
        .get("items")
        .or_else(|| value.get("agents"))
        .and_then(|v| v.as_array())
    else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|item| {
            let id = item.get("id")?.as_str()?.trim();
            if safe_agent_id(id).is_none() {
                return None;
            }
            Some(RemoteAgent {
                id: id.to_string(),
                name: item
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Agent")
                    .to_string(),
                status: item
                    .get("status")
                    .and_then(|v| v.as_str())
                    .unwrap_or("IDLE")
                    .to_string(),
                url: item
                    .get("url")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
            })
        })
        .collect()
}

pub fn parse_run_id(value: &Value) -> Option<String> {
    value
        .pointer("/run/id")
        .and_then(|v| v.as_str())
        .or_else(|| value.get("runId").and_then(|v| v.as_str()))
        .or_else(|| value.get("id").and_then(|v| v.as_str()))
        .map(str::trim)
        .filter(|id| safe_agent_id(id).is_some())
        .map(str::to_string)
}

pub fn parse_v0_conversation(value: &Value) -> Vec<TranscriptLine> {
    let Some(messages) = value.get("messages").and_then(|v| v.as_array()) else {
        return Vec::new();
    };
    let mut lines = Vec::new();
    for (index, message) in messages.iter().enumerate() {
        let text = message
            .get("text")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim();
        if text.is_empty() {
            continue;
        }
        let kind = message.get("type").and_then(|v| v.as_str()).unwrap_or("");
        let role = if kind.contains("user") {
            "user"
        } else {
            "assistant"
        };
        let remote_id = message
            .get("id")
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .unwrap_or_else(|| format!("v0-{index}"));
        lines.push(TranscriptLine {
            remote_id,
            role: role.into(),
            text: text.to_string(),
            ts: None,
        });
    }
    lines
}

pub fn consume_sse(buf: &mut String) -> Vec<SseEvent> {
    let mut events = Vec::new();
    while let Some(end) = buf.find("\n\n") {
        let block = buf[..end].to_string();
        buf.drain(..end + 2);
        let mut event = String::new();
        let mut data = String::new();
        for line in block.lines() {
            if let Some(rest) = line.strip_prefix("event:") {
                event = rest.trim().to_string();
            } else if let Some(rest) = line.strip_prefix("data:") {
                if !data.is_empty() {
                    data.push('\n');
                }
                data.push_str(rest.trim_start());
            }
        }
        if event.is_empty() && data.is_empty() {
            continue;
        }
        events.push(SseEvent { event, data });
    }
    events
}

fn auth_value(style: &str, key: &str) -> String {
    let key = key.trim().strip_prefix("Bearer ").unwrap_or(key.trim());
    if style.eq_ignore_ascii_case("basic") {
        format!("Basic {}", b64(format!("{key}:").as_bytes()))
    } else {
        format!("Bearer {key}")
    }
}

fn b64(bytes: &[u8]) -> String {
    const TABLE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    let mut index = 0;
    while index + 3 <= bytes.len() {
        let n = (u32::from(bytes[index]) << 16)
            | (u32::from(bytes[index + 1]) << 8)
            | u32::from(bytes[index + 2]);
        out.push(TABLE[((n >> 18) & 63) as usize] as char);
        out.push(TABLE[((n >> 12) & 63) as usize] as char);
        out.push(TABLE[((n >> 6) & 63) as usize] as char);
        out.push(TABLE[(n & 63) as usize] as char);
        index += 3;
    }
    let rest = bytes.len() - index;
    if rest == 1 {
        let n = u32::from(bytes[index]) << 16;
        out.push(TABLE[((n >> 18) & 63) as usize] as char);
        out.push(TABLE[((n >> 12) & 63) as usize] as char);
        out.push('=');
        out.push('=');
    } else if rest == 2 {
        let n = (u32::from(bytes[index]) << 16) | (u32::from(bytes[index + 1]) << 8);
        out.push(TABLE[((n >> 18) & 63) as usize] as char);
        out.push(TABLE[((n >> 12) & 63) as usize] as char);
        out.push(TABLE[((n >> 6) & 63) as usize] as char);
        out.push('=');
    }
    out
}

fn query_escape(value: &str) -> String {
    let mut out = String::new();
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char);
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

impl CursorConnector {
    fn root(&self) -> Result<String, String> {
        ensure_http_url(&self.base_url)?;
        Ok(self.base_url.trim_end_matches('/').to_string())
    }

    fn authed(&self, builder: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        builder.header("Authorization", auth_value(&self.auth_style, &self.api_key))
    }

    async fn read_error(&self, response: reqwest::Response) -> String {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        let snippet: String = body.chars().take(180).collect();
        scrub(
            &self.api_key,
            &format!("Cursor returned {status}. {snippet}"),
        )
    }

    pub async fn list_agents(&self) -> Result<Vec<RemoteAgent>, String> {
        let root = self.root()?;
        let mut cursor_q: Option<String> = None;
        let mut all = Vec::new();
        for _ in 0..3 {
            let mut url = format!("{root}/v1/agents?limit=100");
            if let Some(cursor) = &cursor_q {
                url.push_str("&cursor=");
                url.push_str(&query_escape(cursor));
            }
            let response = self
                .authed(self.client.get(&url))
                .send()
                .await
                .map_err(|_| "The Cursor API did not answer.".to_string())?;
            if !response.status().is_success() {
                return Err(self.read_error(response).await);
            }
            let body: Value = response
                .json()
                .await
                .map_err(|_| "The agent list was not JSON.".to_string())?;
            all.extend(parse_agent_list(&body));
            cursor_q = body
                .get("nextCursor")
                .and_then(|v| v.as_str())
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string);
            if cursor_q.is_none() {
                break;
            }
        }
        Ok(all)
    }

    pub async fn transcript(&self) -> Result<Vec<TranscriptLine>, String> {
        let root = self.root()?;
        let agent = safe_agent_id(&self.agent_id).ok_or("Choose an agent for this desk first.")?;
        let url = format!("{root}/v0/agents/{agent}/conversation");
        let response = self
            .authed(self.client.get(&url))
            .send()
            .await
            .map_err(|_| "The Cursor API did not answer.".to_string())?;
        if response.status().is_success() {
            if let Ok(body) = response.json::<Value>().await {
                let lines = parse_v0_conversation(&body);
                if !lines.is_empty() {
                    return Ok(lines);
                }
            }
        }
        self.latest_run_line().await
    }

    async fn latest_run_line(&self) -> Result<Vec<TranscriptLine>, String> {
        let root = self.root()?;
        let agent = safe_agent_id(&self.agent_id).ok_or("Choose an agent for this desk first.")?;
        let url = format!("{root}/v1/agents/{agent}");
        let response = self
            .authed(self.client.get(&url))
            .send()
            .await
            .map_err(|_| "The Cursor API did not answer.".to_string())?;
        if !response.status().is_success() {
            return Err(self.read_error(response).await);
        }
        let body: Value = response
            .json()
            .await
            .map_err(|_| "The agent record was not JSON.".to_string())?;
        let Some(run_id) = body
            .get("latestRunId")
            .and_then(|v| v.as_str())
            .and_then(safe_agent_id)
        else {
            return Ok(Vec::new());
        };
        let run_url = format!("{root}/v1/agents/{agent}/runs/{run_id}");
        let response = self
            .authed(self.client.get(&run_url))
            .send()
            .await
            .map_err(|_| "The Cursor API did not answer.".to_string())?;
        if !response.status().is_success() {
            return Err(self.read_error(response).await);
        }
        let run: Value = response
            .json()
            .await
            .map_err(|_| "The run record was not JSON.".to_string())?;
        let Some(text) = run.get("result").and_then(|v| v.as_str()).map(str::trim) else {
            return Ok(Vec::new());
        };
        if text.is_empty() {
            return Ok(Vec::new());
        }
        Ok(vec![TranscriptLine {
            remote_id: format!("run-{run_id}"),
            role: "assistant".into(),
            text: text.to_string(),
            ts: run
                .get("updatedAt")
                .and_then(|v| v.as_str())
                .map(str::to_string),
        }])
    }

    async fn follow_up(&self, text: &str, tokens: &TokenTx) -> Result<String, String> {
        let root = self.root()?;
        let agent = safe_agent_id(&self.agent_id).ok_or("Choose an agent for this desk first.")?;
        let url = format!("{root}/v1/agents/{agent}/runs");
        let response = self
            .authed(self.client.post(&url))
            .json(&serde_json::json!({ "prompt": { "text": text } }))
            .send()
            .await
            .map_err(|_| "The Cursor API did not answer.".to_string())?;
        if !response.status().is_success() {
            return Err(self.read_error(response).await);
        }
        let body: Value = response
            .json()
            .await
            .map_err(|_| "The follow-up response was not JSON.".to_string())?;
        let run_id = parse_run_id(&body).ok_or("The follow-up response had no run id.")?;
        match self.stream_run(agent, &run_id, tokens).await {
            Ok(text) => Ok(text),
            Err(err)
                if err.starts_with("The agent stream did not open")
                    || err.starts_with("The agent stream broke")
                    || err.starts_with("Cursor returned") =>
            {
                self.poll_run(agent, &run_id).await
            }
            Err(err) => Err(err),
        }
    }

    async fn stream_run(
        &self,
        agent: &str,
        run_id: &str,
        tokens: &TokenTx,
    ) -> Result<String, String> {
        let root = self.root()?;
        let url = format!("{root}/v1/agents/{agent}/runs/{run_id}/stream");
        let response = self
            .authed(self.client.get(&url))
            .header("Accept", "text/event-stream")
            .send()
            .await
            .map_err(|_| "The agent stream did not open.".to_string())?;
        if !response.status().is_success() {
            return Err(self.read_error(response).await);
        }
        let mut stream = response.bytes_stream();
        let mut buf = String::new();
        let mut deltas = String::new();
        let mut result_text: Option<String> = None;
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|_| "The agent stream broke.".to_string())?;
            buf.push_str(&String::from_utf8_lossy(&chunk).replace("\r\n", "\n"));
            for event in consume_sse(&mut buf) {
                match event.event.as_str() {
                    "assistant" => {
                        if let Ok(value) = serde_json::from_str::<Value>(&event.data) {
                            if let Some(text) = value.get("text").and_then(|v| v.as_str()) {
                                deltas.push_str(text);
                                tokens.push(text);
                            }
                        }
                    }
                    "result" => {
                        if let Ok(value) = serde_json::from_str::<Value>(&event.data) {
                            if let Some(text) = value.get("text").and_then(|v| v.as_str()) {
                                result_text = Some(text.to_string());
                            }
                            if value.get("status").and_then(|v| v.as_str()) == Some("ERROR")
                                && result_text.as_deref().unwrap_or("").is_empty()
                            {
                                return Err("The agent run ended with an error.".into());
                            }
                        }
                    }
                    "error" => {
                        let message = serde_json::from_str::<Value>(&event.data)
                            .ok()
                            .and_then(|value| {
                                value
                                    .get("message")
                                    .and_then(|v| v.as_str())
                                    .map(str::to_string)
                            })
                            .unwrap_or_else(|| "The agent stream failed.".into());
                        return Err(scrub(&self.api_key, &message));
                    }
                    _ => {}
                }
            }
        }
        let text = result_text.unwrap_or(deltas);
        if text.trim().is_empty() {
            return Err("The agent stream ended without a reply.".into());
        }
        Ok(text)
    }

    async fn poll_run(&self, agent: &str, run_id: &str) -> Result<String, String> {
        let root = self.root()?;
        let url = format!("{root}/v1/agents/{agent}/runs/{run_id}");
        for _ in 0..15 {
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            let response = match self.authed(self.client.get(&url)).send().await {
                Ok(response) => response,
                Err(_) => continue,
            };
            if !response.status().is_success() {
                continue;
            }
            let Ok(body) = response.json::<Value>().await else {
                continue;
            };
            let status = body.get("status").and_then(|v| v.as_str()).unwrap_or("");
            if matches!(status, "FINISHED" | "ERROR" | "CANCELLED" | "EXPIRED") {
                if let Some(text) = body.get("result").and_then(|v| v.as_str()) {
                    if !text.trim().is_empty() {
                        return Ok(text.to_string());
                    }
                }
                if status == "FINISHED" {
                    return Err("The run finished without a reply.".into());
                }
                return Err(format!("The run ended as {status}."));
            }
        }
        Err("The run is still going. Open this desk again in a moment to pull the result.".into())
    }
}

impl BotConnector for CursorConnector {
    fn kind(&self) -> &'static str {
        "cursor-agents"
    }

    async fn dispatch(&self, req: Outbound, tokens: &TokenTx) -> Result<DispatchOutcome, String> {
        if self.api_key.trim().is_empty() {
            return Err("Set CURSOR_API_KEY, or store it in the keyring under service atlas-layer and account cursor-api-key. It is never written into config.".into());
        }
        let text = self.follow_up(&req.text, tokens).await?;
        Ok(DispatchOutcome::Reply(text))
    }
}
