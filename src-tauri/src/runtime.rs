use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::sync::Mutex;

use crate::config::{self, effective_agent, load_office, ollama_base, relay_url, OfficeData};
use crate::connectors::cursor::{self, CursorConnector, RemoteAgent};
use crate::connectors::echo::EchoConnector;
use crate::connectors::local_window::LocalWindowConnector;
use crate::connectors::ollama::OllamaConnector;
use crate::connectors::relay::RelayConnector;
use crate::connectors::{self, Connector, DispatchOutcome, Outbound, TokenTx};
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
    AppState {
        data: Arc::new(Mutex::new(office)),
        client,
    }
}

pub fn start_background(app: AppHandle, state: AppState) {
    start_watcher(app.clone(), state.clone());
    start_poll(app, state);
}

fn emit(app: &AppHandle, event: &UiEvent) {
    let _ = app.emit("office-event", event);
}

#[tauri::command]
async fn get_office_state(state: State<'_, AppState>) -> Result<OfficeSnapshot, String> {
    let mut data = state.data.lock().await;
    snapshot(&mut data)
}

fn snapshot(data: &mut OfficeData) -> Result<OfficeSnapshot, String> {
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
        services: service_readout(&data.config.features),
    })
}

fn service_readout(features: &config::FeaturesCfg) -> ServicesDto {
    let (filter, shared_compute) =
        MockService::from_flags(features.filter, features.shared_compute).pair();
    ServicesDto {
        filter,
        shared_compute,
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
    let mut data = state.data.lock().await;
    snapshot(&mut data)
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
    let mut data = state.data.lock().await;
    snapshot(&mut data)
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
    snapshot(&mut data)
}

#[tauri::command]
async fn refresh_cursor_agents(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Vec<RemoteAgent>, String> {
    let connector = cursor_from_state(&state, "").await?;
    let agents = connector.list_agents().await?;
    apply_agent_list(&app, &state, &agents).await;
    Ok(agents)
}

#[tauri::command]
async fn bind_cursor_agent(
    state: State<'_, AppState>,
    bot_id: String,
    agent_id: String,
) -> Result<BotDto, String> {
    let bot_id = safe_bot_id(&bot_id).ok_or("Unknown desk")?.to_string();
    let agent_id = model::safe_agent_id(&agent_id)
        .ok_or("That agent id does not look right.")?
        .to_string();
    let mut data = state.data.lock().await;
    if !data.bots_writable {
        return Err("bots.json could not be read earlier, so it was not changed.".into());
    }
    if !data.bots.iter().any(|bot| bot.id == bot_id) {
        return Err("Unknown desk".into());
    }
    let bots_path = data.bots_path.clone();
    write_agent_id(&bots_path, &mut data.bots_root, &bot_id, &agent_id)?;
    if let Some(bot) = data.bots.iter_mut().find(|bot| bot.id == bot_id) {
        bot.agent_id = Some(agent_id.clone());
    }
    data.matched_agents.insert(bot_id.clone(), agent_id);
    let index = data
        .bots
        .iter()
        .position(|bot| bot.id == bot_id)
        .unwrap_or(0);
    let bot = &data.bots[index];
    let matched = data.matched_agents.get(&bot.id).map(String::as_str);
    Ok(present_bot(bot, index, matched))
}

fn write_agent_id(
    path: &std::path::Path,
    root: &mut serde_json::Value,
    bot_id: &str,
    agent_id: &str,
) -> Result<(), String> {
    let list = if root.get("bots").is_some() {
        root.get_mut("bots")
            .and_then(|value| value.as_array_mut())
            .ok_or("bots.json has no bots array")?
    } else {
        root.as_array_mut().ok_or("bots.json has no bots array")?
    };
    let mut found = false;
    for item in list.iter_mut() {
        if item.get("id").and_then(|value| value.as_str()) == Some(bot_id) {
            item["agentId"] = serde_json::json!(agent_id);
            found = true;
        }
    }
    if !found {
        return Err("That desk is not in bots.json.".into());
    }
    let mut pretty = serde_json::to_string_pretty(root).map_err(|err| err.to_string())?;
    pretty.push('\n');
    todos::atomic_write(path, &pretty)
}

async fn publish_board(app: &AppHandle, state: &AppState, view: &todos::TodoView) {
    {
        let mut data = state.data.lock().await;
        data.last_todo_sig = view.signature();
    }
    emit(
        app,
        &UiEvent::Todos {
            todos: view.todos.clone(),
            today_focus: view.focus.clone(),
            metrics: view.metrics.clone(),
            updated: view.updated.clone(),
        },
    );
}

async fn run_dispatch(
    app: &AppHandle,
    state: &AppState,
    bot: &Bot,
    user: &StoredMessage,
) -> Result<DispatchOutcome, String> {
    let connector = build_connector(state, bot).await?;
    let outbound = build_outbound(state, bot, user).await?;
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let tokens = TokenTx::new(tx);
    let reply_id = format!("{}:reply", user.id);
    let app_tok = app.clone();
    let bot_id = bot.id.clone();
    let reader = tokio::spawn(async move {
        while let Some(token) = rx.recv().await {
            emit(
                &app_tok,
                &UiEvent::Token {
                    bot_id: bot_id.clone(),
                    message_id: reply_id.clone(),
                    token,
                },
            );
        }
    });
    let result = connector.dispatch(outbound, &tokens).await;
    drop(tokens);
    let _ = reader.await;
    result
}

async fn build_outbound(
    state: &AppState,
    bot: &Bot,
    user: &StoredMessage,
) -> Result<Outbound, String> {
    let (history_path, personality, system_override) = {
        let data = state.data.lock().await;
        (
            history::path_for(&data.paths.history, &bot.id)?,
            data.paths.personality.clone(),
            bot.system_prompt.clone(),
        )
    };
    let history = history::read_all(&history_path);
    let system = system_override.or_else(|| config::read_prompt(&personality));
    let messages = if bot.connector == "local-ollama" || bot.connector == "echo" {
        connectors::build_ollama_messages(system.as_deref(), &history, 20)
    } else {
        Vec::new()
    };
    Ok(Outbound {
        bot_id: bot.id.clone(),
        bot_name: bot.name.clone(),
        text: user.text.clone(),
        message_id: user.id.clone(),
        messages,
    })
}

async fn build_connector(state: &AppState, bot: &Bot) -> Result<Connector, String> {
    match bot.connector.as_str() {
        "local-window" => {
            let (inbox_dir, launch_command) = {
                let data = state.data.lock().await;
                let inbox_dir = if data.config.local_window.inbox_dir.trim().is_empty() {
                    config::expand_tilde("~/Atla/AI/inbox")
                } else {
                    config::resolve_against(&data.config_dir, &data.config.local_window.inbox_dir)
                };
                (inbox_dir, data.config.local_window.launch_command.clone())
            };
            Ok(Connector::LocalWindow(LocalWindowConnector {
                inbox_dir,
                launch_command,
                home: dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")),
            }))
        }
        "local-ollama" => {
            let (base_url, model, echo) = {
                let data = state.data.lock().await;
                let model = bot
                    .model
                    .clone()
                    .filter(|model| !model.trim().is_empty())
                    .unwrap_or_else(|| data.config.ollama.model.clone());
                (
                    ollama_base(&data.config),
                    model,
                    data.config.ollama.echo_if_unavailable || echo_forced(),
                )
            };
            Ok(Connector::Ollama(OllamaConnector {
                base_url,
                model,
                echo_if_unavailable: echo,
                client: state.client.clone(),
            }))
        }
        "echo" => Ok(Connector::Echo(EchoConnector)),
        "relay-webhook" => {
            let (url, url_env, env_name, account) = {
                let data = state.data.lock().await;
                (
                    relay_url(&data.config),
                    data.config.relay.url_env.clone(),
                    data.config.relay.auth_env.clone(),
                    data.config.relay.auth_keyring_account.clone(),
                )
            };
            if url.is_empty() {
                return Err(format!(
                    "Set relay.url or {url_env} before messaging this desk."
                ));
            }
            connectors::ensure_http_url(&url)?;
            let auth =
                tokio::task::spawn_blocking(move || secrets::resolve_secret(&env_name, &account))
                    .await
                    .unwrap_or(None);
            Ok(Connector::Relay(RelayConnector {
                url,
                auth,
                client: state.client.clone(),
            }))
        }
        "cursor-agents" => {
            let agent = {
                let data = state.data.lock().await;
                effective_agent(&data, bot).unwrap_or_default()
            };
            let connector = cursor_from_state(state, &agent).await?;
            if connector.agent_id.is_empty() {
                return Err("Choose a Cursor agent for this desk. The list is in the chat.".into());
            }
            Ok(Connector::Cursor(connector))
        }
        other => Err(format!("This desk has no connector ({other}).")),
    }
}

async fn cursor_from_state(state: &AppState, agent_id: &str) -> Result<CursorConnector, String> {
    let (base, env_name, account, style) = {
        let data = state.data.lock().await;
        (
            data.config.cursor.base_url.clone(),
            data.config.cursor.api_key_env.clone(),
            data.config.cursor.api_key_keyring_account.clone(),
            data.config.cursor.auth.clone(),
        )
    };
    let key = tokio::task::spawn_blocking(move || secrets::resolve_secret(&env_name, &account))
        .await
        .unwrap_or(None)
        .unwrap_or_default();
    if key.is_empty() {
        return Err("Set CURSOR_API_KEY, or store it with secret-tool (service atlas-layer, account cursor-api-key). A key saved earlier under service atla-office still works. The key is never written into config.".into());
    }
    Ok(CursorConnector {
        base_url: base,
        api_key: key,
        auth_style: style,
        agent_id: agent_id.to_string(),
        client: state.client.clone(),
    })
}

async fn settle(
    app: &AppHandle,
    state: &AppState,
    bot_id: &str,
    user_id: &str,
    result: Result<DispatchOutcome, String>,
) {
    let history_path = {
        let data = state.data.lock().await;
        history::path_for(&data.paths.history, bot_id).ok()
    };
    match result {
        Ok(DispatchOutcome::Reply(text)) => {
            let focused = {
                let data = state.data.lock().await;
                data.focus.as_deref() == Some(bot_id)
            };
            let reply = StoredMessage {
                id: format!("{user_id}:reply"),
                role: "assistant".into(),
                text: text.clone(),
                ts: model::now_iso(),
                read: focused,
                in_reply_to: Some(user_id.to_string()),
                remote_id: None,
            };
            if let Some(path) = &history_path {
                let _ = history::append(path, &reply);
            }
            let status =
                finish_status(state, bot_id, if focused { "idle" } else { "message" }).await;
            emit(
                app,
                &UiEvent::Done {
                    bot_id: bot_id.into(),
                    message_id: reply.id,
                    text,
                },
            );
            emit(
                app,
                &UiEvent::Status {
                    bot_id: bot_id.into(),
                    status,
                },
            );
        }
        Ok(DispatchOutcome::Accepted) => {
            let status = finish_status(state, bot_id, "idle").await;
            emit(
                app,
                &UiEvent::Accepted {
                    bot_id: bot_id.into(),
                    message_id: user_id.into(),
                },
            );
            emit(
                app,
                &UiEvent::Status {
                    bot_id: bot_id.into(),
                    status,
                },
            );
        }
        Err(message) => {
            let note = StoredMessage {
                id: model::new_id(),
                role: "system".into(),
                text: message.clone(),
                ts: model::now_iso(),
                read: true,
                in_reply_to: Some(user_id.to_string()),
                remote_id: None,
            };
            if let Some(path) = &history_path {
                let _ = history::append(path, &note);
            }
            let status = finish_status(state, bot_id, "idle").await;
            emit(
                app,
                &UiEvent::Error {
                    bot_id: bot_id.into(),
                    message_id: user_id.into(),
                    message,
                },
            );
            emit(
                app,
                &UiEvent::Status {
                    bot_id: bot_id.into(),
                    status,
                },
            );
        }
    }
}

async fn finish_status(state: &AppState, bot_id: &str, when_clear: &str) -> String {
    let mut data = state.data.lock().await;
    data.inflight.remove(bot_id);
    let focused = data.focus.as_deref() == Some(bot_id);
    let current = data
        .statuses
        .get(bot_id)
        .cloned()
        .unwrap_or_else(|| "idle".into());
    let next = if current == "message" && !focused {
        "message".to_string()
    } else if focused {
        "idle".to_string()
    } else {
        when_clear.to_string()
    };
    data.statuses.insert(bot_id.to_string(), next.clone());
    next
}

async fn apply_agent_list(app: &AppHandle, state: &AppState, agents: &[RemoteAgent]) {
    let mut changed: Vec<(String, String)> = Vec::new();
    {
        let mut data = state.data.lock().await;
        for bot in data.bots.clone() {
            if bot.connector != "cursor-agents" || bot.pinned_agent().is_some() {
                continue;
            }
            let matches: Vec<&RemoteAgent> = agents
                .iter()
                .filter(|agent| agent.name.eq_ignore_ascii_case(&bot.name))
                .collect();
            if matches.len() == 1 {
                data.matched_agents
                    .insert(bot.id.clone(), matches[0].id.clone());
            }
        }
        for bot in data.bots.clone() {
            if bot.connector != "cursor-agents" {
                continue;
            }
            let Some(agent_id) = effective_agent(&data, &bot) else {
                continue;
            };
            let Some(agent) = agents.iter().find(|agent| agent.id == agent_id) else {
                continue;
            };
            if data.inflight.contains(&bot.id) {
                continue;
            }
            if data.statuses.get(&bot.id).map(String::as_str) == Some("message")
                && data.focus.as_deref() != Some(bot.id.as_str())
            {
                continue;
            }
            let mapped = cursor::map_agent_status(&agent.status);
            if data.focus.as_deref() == Some(bot.id.as_str()) && mapped == "idle" {
                continue;
            }
            if data.statuses.get(&bot.id).map(String::as_str) != Some(mapped) {
                data.statuses.insert(bot.id.clone(), mapped.into());
                changed.push((bot.id.clone(), mapped.into()));
            }
        }
    }
    for (bot_id, status) in changed {
        emit(app, &UiEvent::Status { bot_id, status });
    }
}

/// Pulls each Cursor agent's transcript into that desk's history and shows
/// any line the desk has not already stored. Skips a desk that is mid-send
/// so the reply just streamed is not written twice.
async fn mirror_cursor_desks(app: &AppHandle, state: &AppState, only_bot: Option<&str>) {
    let desks: Vec<(String, String, std::path::PathBuf, bool)> = {
        let data = state.data.lock().await;
        if !data.config.features.office {
            return;
        }
        let mut desks = Vec::new();
        for bot in &data.bots {
            if bot.connector != "cursor-agents" {
                continue;
            }
            if only_bot.is_some_and(|id| id != bot.id) {
                continue;
            }
            if only_bot.is_none() && data.inflight.contains(&bot.id) {
                continue;
            }
            let Some(agent_id) = effective_agent(&data, bot) else {
                continue;
            };
            let Ok(history_path) = history::path_for(&data.paths.history, &bot.id) else {
                continue;
            };
            desks.push((
                bot.id.clone(),
                agent_id,
                history_path,
                data.focus.as_deref() == Some(bot.id.as_str()),
            ));
        }
        desks
    };
    for (bot_id, agent_id, history_path, focused) in desks {
        let Ok(connector) = cursor_from_state(state, &agent_id).await else {
            continue;
        };
        let Ok(lines) = connector.transcript().await else {
            continue;
        };
        let Ok(added) = history::mirror_lines(&history_path, &lines, focused) else {
            continue;
        };
        let mut unseen = false;
        for message in added {
            if message.role == "assistant" && !message.read {
                unseen = true;
            }
            emit(
                app,
                &UiEvent::Message {
                    bot_id: bot_id.clone(),
                    message,
                },
            );
        }
        if unseen {
            let mut data = state.data.lock().await;
            if !data.inflight.contains(&bot_id) && data.focus.as_deref() != Some(bot_id.as_str()) {
                data.statuses.insert(bot_id.clone(), "message".into());
                drop(data);
                emit(
                    app,
                    &UiEvent::Status {
                        bot_id,
                        status: "message".into(),
                    },
                );
            }
        }
    }
}

fn start_poll(app: AppHandle, state: AppState) {
    tauri::async_runtime::spawn(async move {
        let seconds = {
            let data = state.data.lock().await;
            let configured = data.config.cursor.poll_seconds;
            if configured == 0 {
                0
            } else {
                configured.max(15)
            }
        };
        if seconds == 0 {
            return;
        }
        let mut interval = tokio::time::interval(Duration::from_secs(seconds));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            interval.tick().await;
            let has_cursor = {
                let data = state.data.lock().await;
                data.config.features.office
                    && data.bots.iter().any(|bot| bot.connector == "cursor-agents")
            };
            if !has_cursor {
                continue;
            }
            if let Ok(connector) = cursor_from_state(&state, "").await {
                if let Ok(agents) = connector.list_agents().await {
                    apply_agent_list(&app, &state, &agents).await;
                }
            }
            mirror_cursor_desks(&app, &state, None).await;
        }
    });
}

fn start_watcher(app: AppHandle, state: AppState) {
    let _ = std::thread::Builder::new()
        .name("atla-watch".into())
        .spawn(move || watch_loop(app, state));
}

fn watch_loop(app: AppHandle, state: AppState) {
    let (tx, rx) = std::sync::mpsc::channel();
    let mut debouncer = match notify_debouncer_mini::new_debouncer(
        Duration::from_millis(300),
        move |result: notify_debouncer_mini::DebounceEventResult| {
            if result.is_ok() {
                let _ = tx.send(());
            }
        },
    ) {
        Ok(debouncer) => debouncer,
        Err(err) => {
            eprintln!("[atlas-layer] file watch is off ({err})");
            return;
        }
    };
    let mut watching: Vec<PathBuf> = Vec::new();
    loop {
        let paths = {
            let data = state.data.blocking_lock();
            let mut paths = vec![
                data.paths.todo.clone(),
                data.paths.today.clone(),
                data.paths.inbox_dir.clone(),
            ];
            if let Some(file) = &data.paths.inbox_file {
                paths.push(file.clone());
            }
            paths
        };
        for path in paths {
            let target = if path.is_dir() {
                path
            } else {
                path.parent().map(PathBuf::from).unwrap_or(path)
            };
            if target.exists() && !watching.iter().any(|watched| watched == &target) {
                if debouncer
                    .watcher()
                    .watch(&target, notify::RecursiveMode::NonRecursive)
                    .is_ok()
                {
                    watching.push(target);
                }
            }
        }
        match rx.recv_timeout(Duration::from_secs(5)) {
            Ok(()) | Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                let office_on = state.data.blocking_lock().config.features.office;
                if office_on {
                    reload_board(&app, &state);
                    ingest(&app, &state);
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
}

fn reload_board(app: &AppHandle, state: &AppState) {
    let (todo, today) = {
        let data = state.data.blocking_lock();
        (data.paths.todo.clone(), data.paths.today.clone())
    };
    match todos::view(&todo, &today) {
        Ok(view) => {
            let signature = view.signature();
            let mut data = state.data.blocking_lock();
            if data.last_todo_sig == signature {
                return;
            }
            data.last_todo_sig = signature;
            drop(data);
            emit(
                app,
                &UiEvent::Todos {
                    todos: view.todos,
                    today_focus: view.focus,
                    metrics: view.metrics,
                    updated: view.updated,
                },
            );
            if let Some(message) = view.warning {
                note_once(app, state, &message);
            }
        }
        Err(err) => note_once(app, state, &err),
    }
}

fn note_once(app: &AppHandle, state: &AppState, message: &str) {
    let mut data = state.data.blocking_lock();
    if data.last_board_note == message {
        return;
    }
    data.last_board_note = message.to_string();
    drop(data);
    emit(
        app,
        &UiEvent::Warning {
            message: message.to_string(),
        },
    );
}

fn ingest(app: &AppHandle, state: &AppState) {
    let (dir, file, offset, history) = {
        let data = state.data.blocking_lock();
        (
            data.paths.inbox_dir.clone(),
            data.paths.inbox_file.clone(),
            data.jsonl_offset,
            data.paths.history.clone(),
        )
    };
    if let Some(file) = file {
        match inbox::read_jsonl_from(&file, offset) {
            Ok((items, new_offset)) => {
                {
                    let mut data = state.data.blocking_lock();
                    data.jsonl_offset = new_offset;
                }
                if new_offset != offset {
                    inbox::store_offset(&history, &file, new_offset);
                }
                for (index, item) in items.into_iter().enumerate() {
                    let source = format!("jsonl-{offset}-{index}");
                    match item {
                        Ok(reply) => deliver(app, state, &reply, &source),
                        Err(err) => eprintln!("[atlas-layer] inbox line skipped: {err}"),
                    }
                }
            }
            Err(err) => eprintln!("[atlas-layer] inbox file: {err}"),
        }
    }
    match inbox::list_pending(&dir) {
        Ok(pending) => {
            for item in pending {
                let source = item
                    .path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("reply.json")
                    .to_string();
                let known = {
                    let data = state.data.blocking_lock();
                    data.bots.iter().any(|bot| {
                        bot.id == item.reply.bot_key
                            || bot.name.eq_ignore_ascii_case(&item.reply.bot_key)
                    })
                };
                if !known {
                    eprintln!(
                        "[atlas-layer] inbox reply for an unknown desk ({})",
                        item.reply.bot_key.chars().take(40).collect::<String>()
                    );
                    let _ = inbox::archive(&item.path, &dir, "unmatched");
                    continue;
                }
                deliver(app, state, &item.reply, &source);
                let _ = inbox::archive(&item.path, &dir, "processed");
            }
        }
        Err(err) => eprintln!("[atlas-layer] inbox dir: {err}"),
    }
}

fn deliver(app: &AppHandle, state: &AppState, reply: &InboxReply, source: &str) {
    let (bot_id, history_path, focused) = {
        let data = state.data.blocking_lock();
        let Some(bot_id) = data.bots.iter().find_map(|bot| {
            if bot.id == reply.bot_key || bot.name.eq_ignore_ascii_case(&reply.bot_key) {
                Some(bot.id.clone())
            } else {
                None
            }
        }) else {
            eprintln!(
                "[atlas-layer] inbox reply for an unknown desk ({})",
                reply.bot_key.chars().take(40).collect::<String>()
            );
            return;
        };
        let Ok(history_path) = history::path_for(&data.paths.history, &bot_id) else {
            return;
        };
        let focused = data.focus.as_deref() == Some(bot_id.as_str());
        (bot_id, history_path, focused)
    };
    if history::already_mirrored(
        &history_path,
        "assistant",
        &reply.text,
        reply.in_reply_to.as_deref(),
    ) {
        return;
    }
    let message = StoredMessage {
        id: inbox::content_id(source, reply),
        role: "assistant".into(),
        text: reply.text.clone(),
        ts: reply.ts.clone().unwrap_or_else(model::now_iso),
        read: focused,
        in_reply_to: reply.in_reply_to.clone(),
        remote_id: None,
    };
    let stored = match history::append_unique(&history_path, &message) {
        Ok(stored) => stored,
        Err(err) => {
            eprintln!("[atlas-layer] could not store an inbox reply ({err})");
            return;
        }
    };
    if !stored {
        return;
    }
    let status = {
        let mut data = state.data.blocking_lock();
        let status = if focused {
            data.statuses
                .get(&bot_id)
                .cloned()
                .filter(|status| status == "working")
                .unwrap_or_else(|| "idle".into())
        } else if data.inflight.contains(&bot_id) {
            "message".into()
        } else {
            "message".into()
        };
        data.statuses.insert(bot_id.clone(), status.clone());
        status
    };
    emit(
        app,
        &UiEvent::Message {
            bot_id: bot_id.clone(),
            message: message.clone(),
        },
    );
    emit(app, &UiEvent::Status { bot_id, status });
}

#[cfg(test)]
mod event_shape {
    use super::UiEvent;

    #[test]
    fn events_use_camel_case_fields() {
        let event = UiEvent::Status {
            bot_id: "atlas".into(),
            status: "message".into(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert_eq!(
            json,
            r#"{"type":"status","botId":"atlas","status":"message"}"#
        );
    }
}

pub fn run() {
    let state = boot();
    tauri::Builder::default()
        .manage(state)
        .setup(|app| {
            let state = app.state::<AppState>().inner().clone();
            start_background(app.handle().clone(), state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_office_state,
            open_bot,
            close_bot,
            send_message,
            add_todo,
            toggle_todo,
            refresh_cursor_agents,
            bind_cursor_agent,
            set_feature,
        ])
        .run(tauri::generate_context!())
        .expect("atla office failed to start");
}
