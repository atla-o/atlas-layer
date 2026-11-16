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
            let text = gate_text(state, text).await;
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
            open_comm,
        ])
        .run(tauri::generate_context!())
        .expect("atla office failed to start");
}
