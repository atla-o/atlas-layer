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
    Token {
        bot_id: String,
        message_id: String,
        token: String,
    },
    Done {
        bot_id: String,
        message_id: String,
        text: String,
    },
    Error {
        bot_id: String,
        message_id: String,
        message: String,
    },
    Accepted {
        bot_id: String,
        message_id: String,
    },
    Status {
        bot_id: String,
        status: String,
    },
    Todos {
        todos: Vec<TodoDto>,
        today_focus: Option<String>,
        metrics: Vec<MetricDto>,
        updated: Option<String>,
    },
    Message {
        bot_id: String,
        message: StoredMessage,
    },
    Warning {
        message: String,
    },
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PathDto {
    todo: String,
    today: String,
    inbox: String,
    history: String,
    config: String,
    personality: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct OfficeSnapshot {
    title: String,
    bots: Vec<BotDto>,
    todos: Vec<TodoDto>,
    today_focus: Option<String>,
    metrics: Vec<MetricDto>,
    updated: Option<String>,
    statuses: std::collections::HashMap<String, String>,
    paths: PathDto,
    warnings: Vec<String>,
    preview: bool,
    features: config::FeaturesCfg,
    services: ServicesDto,
    comms: Vec<CommDto>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ServicesDto {
    filter: ServiceReport,
    shared_compute: ServiceReport,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct OpenedChat {
    messages: Vec<StoredMessage>,
    warning: Option<String>,
    agents: Vec<RemoteAgent>,
    agent_id: String,
}

pub fn boot() -> AppState {
    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(600))
        .user_agent("atlas-layer/0.1")
        .build()
        .expect("http client");
    let office = load_office(&config::resolve_config_dir());
    let compute_path = office.paths.history.join("compute.txt");
    let compute = LocalCompute::load(&compute_path, office.config.features.shared_compute);
    AppState {
        data: Arc::new(Mutex::new(office)),
        client,
        compute: Arc::new(Mutex::new(compute)),
        filter_holds: Arc::new(AtomicU64::new(0)),
    }
}

pub fn start_background(app: AppHandle, state: AppState) {
    start_watcher(app.clone(), state.clone());
    start_poll(app.clone(), state.clone());
    start_compute(state.clone());
    start_live(app, state);
}

fn emit(app: &AppHandle, event: &UiEvent) {
    let _ = app.emit("office-event", event);
}

#[tauri::command]
async fn get_office_state(state: State<'_, AppState>) -> Result<OfficeSnapshot, String> {
    note_activity(&state).await;
    let services = services_of(&state).await;
    let mut data = state.data.lock().await;
    snapshot(&mut data, services)
}

fn snapshot(data: &mut OfficeData, services: ServicesDto) -> Result<OfficeSnapshot, String> {
    let mut warnings = data.warnings.clone();
    let (todos, today_focus, metrics, updated) =
        match todos::view(&data.paths.todo, &data.paths.today) {
            Ok(view) => {
                data.last_todo_sig = view.signature();
                if let Some(note) = view.warning.clone() {
                    warnings.push(note);
                }
                (view.todos, view.focus, view.metrics, view.updated)
            }
            Err(err) => {
                warnings.push(err);
                (Vec::new(), None, Vec::new(), None)
            }
        };
    let bots = data
        .bots
        .iter()
        .enumerate()
        .map(|(index, bot)| {
            let matched = data.matched_agents.get(&bot.id).map(String::as_str);
            present_bot(bot, index, matched)
        })
        .collect();
    let inbox = data
        .paths
        .inbox_file
        .clone()
        .unwrap_or_else(|| data.paths.inbox_dir.clone());
    Ok(OfficeSnapshot {
        title: data.config.office.title.clone(),
        bots,
        todos,
        today_focus,
        metrics,
        updated,
        statuses: data.statuses.clone(),
        paths: PathDto {
            todo: data.paths.todo.display().to_string(),
            today: data.paths.today.display().to_string(),
            inbox: inbox.display().to_string(),
            history: data.paths.history.display().to_string(),
            config: data.config_dir.display().to_string(),
            personality: data.paths.personality.display().to_string(),
        },
        warnings,
        preview: false,
        features: data.config.features.clone(),
        services,
        comms: comms::current_objects(),
    })
}

#[tauri::command]
async fn open_comm(app: AppHandle, id: String) -> Result<(), String> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    let handle = app.clone();
    app.run_on_main_thread(move || {
        let _ = tx.send(comms::open(&handle, &id));
    })
    .map_err(|err| err.to_string())?;
    rx.await.map_err(|err| err.to_string())?
}

async fn note_activity(state: &AppState) {
    state.compute.lock().await.note_activity();
}

async fn services_of(state: &AppState) -> ServicesDto {
    let holds = state.filter_holds.load(Ordering::Relaxed);
    let filter_on = state.data.lock().await.config.features.filter;
    let shared_compute = state.compute.lock().await.report();
    ServicesDto {
        filter: filter_report(filter_on, holds),
        shared_compute,
    }
}

fn filter_report(enabled: bool, holds: u64) -> ServiceReport {
    ServiceReport {
        feature: "filter".into(),
        enabled,
        phase: if enabled {
            "scanning".into()
        } else {
            "off".into()
        },
        detail: if enabled {
            "On. A local word list checks desk text on this machine. Nothing is uploaded, and turning this off stops scanning."
        } else {
            "Off. Desk text is not scanned. No hosts file or DNS setting was changed."
        }
        .into(),
        running: if enabled {
            "Local list".into()
        } else {
            "Nothing".into()
        },
        contributed: format!("{holds} held"),
        earned: "0".into(),
    }
}

async fn gate_text(state: &AppState, text: String) -> String {
    let enabled = state.data.lock().await.config.features.filter;
    if enabled && filter::blocked(&text).is_some() {
        state.filter_holds.fetch_add(1, Ordering::Relaxed);
        filter::held_text().to_string()
    } else {
        text
    }
}
