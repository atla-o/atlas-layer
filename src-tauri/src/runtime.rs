use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::sync::Mutex;

use crate::comms::{self, CommDto};
use crate::compute::{battery_discharging, LocalCompute, SpareEngine};
use crate::config::{self, effective_agent, load_office, ollama_base, relay_url, OfficeData};
use crate::connectors::cursor::{self, CursorConnector, RemoteAgent};
use crate::connectors::echo::EchoConnector;
use crate::connectors::local_window::LocalWindowConnector;
use crate::connectors::ollama::OllamaConnector;
use crate::connectors::relay::RelayConnector;
use crate::connectors::{self, Connector, DispatchOutcome, Outbound, TokenTx};
use crate::filter;
use crate::history;
use crate::inbox::{self, InboxReply};
use crate::model::{
    self, present_bot, safe_bot_id, Bot, BotDto, MetricDto, StoredMessage, TodoDto,
};
use crate::secrets::{self, echo_forced};
use crate::services::{BackgroundService, MockService, ServiceReport, ServiceRequest};
use crate::todos;

#[derive(Clone)]
pub struct AppState {
    pub data: Arc<Mutex<OfficeData>>,
    pub client: reqwest::Client,
    pub compute: Arc<Mutex<LocalCompute>>,
    pub filter_holds: Arc<AtomicU64>,
}

#[derive(Clone, Serialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum UiEvent {
    Token { bot_id: String, message_id: String, token: String },
    Done { bot_id: String, message_id: String, text: String },
    Error { bot_id: String, message_id: String, message: String },
    Accepted { bot_id: String, message_id: String },
    Status { bot_id: String, status: String },
    Todos { todos: Vec<TodoDto>, today_focus: Option<String>, metrics: Vec<MetricDto>, updated: Option<String> },
    Message { bot_id: String, message: StoredMessage },
    Warning { message: String },
}
