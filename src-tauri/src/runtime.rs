use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::sync::Mutex;

use crate::comms::{self, CommDto};
use crate::stations::{self, StationDto};
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
    stations: Vec<StationDto>,
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
        stations: stations::stations(&comms::current_host()),
    })
}

#[tauri::command]
fn station_detail(id: String) -> Result<stations::StationDetail, String> {
    let home = dirs::home_dir().ok_or_else(|| "There is no home directory.".to_string())?;
    stations::detail(&comms::current_host(), &id, &home)
}

#[tauri::command]
fn launch_tool(app_id: String, file: Option<String>) -> Result<(), String> {
    let home = dirs::home_dir().ok_or_else(|| "There is no home directory.".to_string())?;
    stations::launch(&comms::current_host(), &app_id, file.as_deref(), &home)
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

#[tauri::command]
async fn open_bot(
    app: AppHandle,
    state: State<'_, AppState>,
    bot_id: String,
) -> Result<OpenedChat, String> {
    let bot_id = safe_bot_id(&bot_id).ok_or("Unknown desk")?.to_string();
    let (history_path, kind, agent_hint, status) = {
        let mut data = state.data.lock().await;
        if !data.config.features.office {
            return Err("Office is off. Turn it on in Settings to open a desk.".into());
        }
        let bot = data
            .bots
            .iter()
            .find(|bot| bot.id == bot_id)
            .cloned()
            .ok_or("Unknown desk")?;
        let history_path = history::path_for(&data.paths.history, &bot.id)?;
        data.focus = Some(bot.id.clone());
        if !data.inflight.contains(&bot.id) {
            data.statuses.insert(bot.id.clone(), "idle".into());
        }
        let status = data
            .statuses
            .get(&bot.id)
            .cloned()
            .unwrap_or_else(|| "idle".into());
        let agent = effective_agent(&data, &bot).unwrap_or_default();
        (history_path, bot.connector.clone(), agent, status)
    };
    let _ = history::mark_read(&history_path);
    emit(
        &app,
        &UiEvent::Status {
            bot_id: bot_id.clone(),
            status,
        },
    );

    let mut warning = None;
    let mut agents = Vec::new();
    if kind == "cursor-agents" {
        match cursor_from_state(&state, &agent_hint).await {
            Ok(mut connector) => {
                match connector.list_agents().await {
                    Ok(list) => {
                        apply_agent_list(&app, &state, &list).await;
                        agents = list;
                    }
                    Err(err) => warning = Some(err),
                }
                let refreshed = {
                    let data = state.data.lock().await;
                    data.bots
                        .iter()
                        .find(|bot| bot.id == bot_id)
                        .and_then(|bot| effective_agent(&data, bot))
                        .unwrap_or_default()
                };
                if refreshed.is_empty() {
                    if warning.is_none() {
                        warning = Some(
                            "This desk is not tied to an agent yet. Pick one from the list.".into(),
                        );
                    }
                } else {
                    connector.agent_id = refreshed;
                    match connector.transcript().await {
                        Ok(lines) => {
                            let _ = history::import_remote(&history_path, &lines);
                        }
                        Err(err) => warning = Some(err),
                    }
                }
            }
            Err(err) => warning = Some(err),
        }
    }
    let agent_id = {
        let data = state.data.lock().await;
        data.bots
            .iter()
            .find(|bot| bot.id == bot_id)
            .and_then(|bot| effective_agent(&data, bot))
            .unwrap_or_default()
    };
    Ok(OpenedChat {
        messages: history::read_all(&history_path),
        warning,
        agents,
        agent_id,
    })
}

#[tauri::command]
async fn close_bot(state: State<'_, AppState>, bot_id: String) -> Result<(), String> {
    let bot_id = safe_bot_id(&bot_id).unwrap_or("");
    let mut data = state.data.lock().await;
    if data.focus.as_deref() == Some(bot_id) {
        data.focus = None;
    }
    Ok(())
}

#[tauri::command]
async fn send_message(
    app: AppHandle,
    state: State<'_, AppState>,
    bot_id: String,
    text: String,
) -> Result<StoredMessage, String> {
    let text = text.trim().to_string();
    if text.is_empty() {
        return Err("Write a message first.".into());
    }
    if text.chars().count() > 8_000 {
        return Err("That message is too long for the desk.".into());
    }
    note_activity(&state).await;
    if state.data.lock().await.config.features.filter && filter::blocked(&text).is_some() {
        state.filter_holds.fetch_add(1, Ordering::Relaxed);
        return Err("The local filter held that message. Nothing was sent.".into());
    }
    let bot_id = safe_bot_id(&bot_id).ok_or("Unknown desk")?.to_string();
    let (bot, history_path) = {
        let data = state.data.lock().await;
        if !data.config.features.office {
            return Err("Office is off. Turn it on in Settings to use the desks.".into());
        }
        let bot = data
            .bots
            .iter()
            .find(|bot| bot.id == bot_id)
            .cloned()
            .ok_or("Unknown desk")?;
        if data.inflight.contains(&bot.id) {
            return Err("This desk is still working on the last message.".into());
        }
        let history_path = history::path_for(&data.paths.history, &bot.id)?;
        (bot, history_path)
    };
    let user = StoredMessage {
        id: model::new_id(),
        role: "user".into(),
        text,
        ts: model::now_iso(),
        read: true,
        in_reply_to: None,
        remote_id: None,
    };
    history::append(&history_path, &user)?;
    {
        let mut data = state.data.lock().await;
        data.inflight.insert(bot.id.clone());
        data.statuses.insert(bot.id.clone(), "working".into());
    }
    emit(
        &app,
        &UiEvent::Status {
            bot_id: bot.id.clone(),
            status: "working".into(),
        },
    );
    let app_task = app.clone();
    let state_task = state.inner().clone();
    let user_task = user.clone();
    let bot_task = bot.clone();
    tauri::async_runtime::spawn(async move {
        let result = run_dispatch(&app_task, &state_task, &bot_task, &user_task).await;
        settle(&app_task, &state_task, &bot_task.id, &user_task.id, result).await;
        if bot_task.connector == "cursor-agents" {
            mirror_cursor_desks(&app_task, &state_task, Some(&bot_task.id)).await;
        }
    });
    Ok(user)
}

#[tauri::command]
async fn add_todo(
    app: AppHandle,
    state: State<'_, AppState>,
    text: String,
) -> Result<OfficeSnapshot, String> {
    let (todo, today) = {
        let data = state.data.lock().await;
        (data.paths.todo.clone(), data.paths.today.clone())
    };
    let view = todos::add(&todo, &today, &text)?;
    publish_board(&app, &state, &view).await;
    note_activity(&state).await;
    let services = services_of(&state).await;
    let mut data = state.data.lock().await;
    snapshot(&mut data, services)
}

#[tauri::command]
async fn toggle_todo(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> Result<OfficeSnapshot, String> {
    let (todo, today) = {
        let data = state.data.lock().await;
        (data.paths.todo.clone(), data.paths.today.clone())
    };
    let view = todos::toggle(&todo, &today, &id)?;
    publish_board(&app, &state, &view).await;
    note_activity(&state).await;
    let services = services_of(&state).await;
    let mut data = state.data.lock().await;
    snapshot(&mut data, services)
}

#[tauri::command]
async fn set_feature(
    state: State<'_, AppState>,
    feature: String,
    enabled: bool,
) -> Result<OfficeSnapshot, String> {
    let mut data = state.data.lock().await;
    if !data.config_writable {
        return Err("config.json could not be read earlier, so it was not changed.".into());
    }
    let key = config::feature_key(&feature)?;
    let mut root = data.config_root.clone();
    config::set_feature_flag(&mut root, key, enabled, &data.config.features)?;
    if key != "office" {
        let mut service = MockService::from_flags(
            data.config.features.filter,
            data.config.features.shared_compute,
        );
        service.handle(ServiceRequest::SetEnabled {
            feature: key.into(),
            enabled,
        })?;
    }
    config::save_config(&data.config_path, &root)?;
    data.config_root = root;
    match key {
        "office" => data.config.features.office = enabled,
        "filter" => data.config.features.filter = enabled,
        "sharedCompute" => data.config.features.shared_compute = enabled,
        _ => {}
    }
    let filter_on = data.config.features.filter;
    let compute_on = data.config.features.shared_compute;
    drop(data);
    if key == "sharedCompute" {
        state.compute.lock().await.set_enabled(enabled);
    }
    note_activity(&state).await;
    let mut services = services_of(&state).await;
    services.filter = filter_report(filter_on, state.filter_holds.load(Ordering::Relaxed));
    if !compute_on {
        services.shared_compute.enabled = false;
    }
    let mut data = state.data.lock().await;
    snapshot(&mut data, services)
}
