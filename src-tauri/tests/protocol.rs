use std::time::Duration;

use atla_office::connectors::cursor::{
    consume_sse, parse_agent_list, parse_run_id, parse_v0_conversation, CursorConnector,
};
use atla_office::connectors::echo::EchoConnector;
use atla_office::connectors::ollama::{consume_ollama_buffer, OllamaAccum, OllamaConnector};
use atla_office::connectors::relay::{
    normalize_auth_header, relay_body, reply_text, RelayConnector,
};
use atla_office::connectors::{BotConnector, DispatchOutcome, Outbound, TokenTx};
use atla_office::inbox::{self, InboxReply};
use atla_office::model::{parse_bots, StoredMessage, TranscriptLine};
use atla_office::secrets::{self, scrub};
use atla_office::todos;
use serde_json::{json, Value};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap()
}

fn outbound(bot: &str, text: &str) -> Outbound {
    Outbound {
        bot_id: bot.into(),
        bot_name: bot.into(),
        text: text.into(),
        message_id: "m-user".into(),
        messages: vec![json!({"role": "user", "content": text})],
    }
}

#[test]
fn seed_list_matches_the_office() {
    let value: Value =
        serde_json::from_str(include_str!("../../assets/defaults/bots.json")).unwrap();
    let (bots, warnings) = parse_bots(&value);
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(bots.len(), 11);
    assert_eq!(bots[0].name, "Atlas");
    assert_eq!(bots[0].connector, "relay-webhook");
    assert!(bots
        .iter()
        .any(|bot| bot.id == "local-atlas" && bot.connector == "local-ollama"));
    assert!(bots
        .iter()
        .any(|bot| bot.id == "devon" && bot.connector == "relay-webhook"));
    assert!(bots
        .iter()
        .any(|bot| bot.id == "atlas-site" && bot.connector == "cursor-agents"));
    assert!(bots
        .iter()
        .any(|bot| bot.id == "roleplay" && bot.connector == "cursor-agents"));
}

#[test]
fn load_office_uses_the_directory_it_is_given() {
    let dir = tempfile::tempdir().unwrap();
    let mut config: Value =
        serde_json::from_str(include_str!("../../assets/defaults/config.json")).unwrap();
    config["paths"]["todo"] = json!("todo.json");
    config["paths"]["today"] = json!("today.json");
    config["paths"]["personality"] = json!("personality.md");
    config["paths"]["inboxDir"] = json!("inbox");
    config["paths"]["historyDir"] = json!("history");
    std::fs::write(
        dir.path().join("config.json"),
        serde_json::to_string_pretty(&config).unwrap(),
    )
    .unwrap();
    let office = atla_office::config::load_office(dir.path());
    assert_eq!(office.bots.len(), 11);
    assert!(dir.path().join("bots.json").is_file());
    assert!(office.paths.inbox_dir.ends_with("inbox"));
    assert!(office.paths.history.ends_with("history"));
    assert!(!office.paths.todo.starts_with("/root") || office.paths.todo.ends_with("todo.json"));
}

#[test]
fn whiteboard_preserves_unknown_fields_and_merges_today() {
    let dir = tempfile::tempdir().unwrap();
    let todo = dir.path().join("todo.json");
    let today = dir.path().join("today.json");
    std::fs::write(&todo, include_str!("../../examples/todo.json")).unwrap();
    std::fs::write(&today, include_str!("../../examples/today.json")).unwrap();

    let view = todos::view(&todo, &today).unwrap();
    assert_eq!(
        view.focus.as_deref(),
        Some("Keep the office quiet and the board short.")
    );
    assert!(view
        .todos
        .iter()
        .any(|item| item.id == "today-1" && item.source == "today"));
    assert_eq!(view.todos.iter().filter(|item| item.id == "t2").count(), 1);
    assert!(view
        .todos
        .iter()
        .any(|item| item.id == "t2" && !item.done && item.source == "todo"));

    todos::toggle(&todo, &today, "t2").unwrap();
    let saved: Value = serde_json::from_str(&std::fs::read_to_string(&todo).unwrap()).unwrap();
    assert_eq!(saved["updatedFrom"], "sample");
    assert_eq!(saved["room"], "atla-office");
    let item = saved["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["id"] == "t2")
        .unwrap();
    assert_eq!(item["done"], true);
    assert_eq!(item["owner"], "products");
    assert_eq!(item["note"], "keep the unknown fields");

    todos::add(&todo, &today, "File the quiet hour").unwrap();
    let saved: Value = serde_json::from_str(&std::fs::read_to_string(&todo).unwrap()).unwrap();
    assert_eq!(saved["room"], "atla-office");
    assert!(saved["items"]
        .as_array()
        .unwrap()
        .iter()
        .any(|item| item["text"] == "File the quiet hour" && item["owner"].is_null()));

    let broken = "{not json";
    std::fs::write(&todo, broken).unwrap();
    let err = todos::toggle(&todo, &today, "t2").unwrap_err();
    assert!(err.contains("Nothing was written"), "{err}");
    assert_eq!(std::fs::read_to_string(&todo).unwrap(), broken);
}

#[test]
fn checking_a_today_item_copies_it_into_todo_json() {
    let dir = tempfile::tempdir().unwrap();
    let todo = dir.path().join("todo.json");
    let today = dir.path().join("today.json");
    std::fs::write(&today, include_str!("../../examples/today.json")).unwrap();
    todos::toggle(&todo, &today, "today-1").unwrap();
    let saved: Value = serde_json::from_str(&std::fs::read_to_string(&todo).unwrap()).unwrap();
    let item = saved["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["id"] == "today-1")
        .unwrap();
    assert_eq!(item["completed"], true);
    assert_eq!(item["from"], "atla-today");
    assert_eq!(item["title"], "Read overnight notes");
}

#[test]
fn inbox_directory_and_jsonl_follow_the_protocol() {
    let dir = tempfile::tempdir().unwrap();
    let inbox = dir.path().join("inbox");
    std::fs::create_dir_all(&inbox).unwrap();
    std::fs::write(
        inbox.join("one.json"),
        r#"{"bot":"atlas","text":"The plant is watered.","in_reply_to":"m-1","ts":"2026-11-12T08:00:00Z","note":"keep me"}"#,
    )
    .unwrap();
    std::fs::write(inbox.join("bad.json"), "{nope").unwrap();
    let pending = inbox::list_pending(&inbox).unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].reply.text, "The plant is watered.");
    assert_eq!(pending[0].reply.in_reply_to.as_deref(), Some("m-1"));
    assert!(inbox.join("unmatched").read_dir().unwrap().next().is_some());

    let jsonl = dir.path().join("replies.jsonl");
    std::fs::write(
        &jsonl,
        "{\"bot\":\"products\",\"text\":\"Shelf sketched.\",\"in_reply_to\":\"m-2\",\"ts\":\"2026-11-12T09:00:00Z\"}\n{\"bot\":\"",
    )
    .unwrap();
    let (first, offset) = inbox::read_jsonl_from(&jsonl, 0).unwrap();
    assert_eq!(first.len(), 1);
    assert!(offset < std::fs::metadata(&jsonl).unwrap().len());
    let (again, _) = inbox::read_jsonl_from(&jsonl, offset).unwrap();
    assert!(again.is_empty());
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .append(true)
        .open(&jsonl)
        .unwrap();
    file.write_all(
        b"creation\",\"text\":\"A new line.\",\"in_reply_to\":\"m-3\",\"ts\":\"2026-11-12T09:01:00Z\"}\n",
    )
    .unwrap();
    let (rest, _) = inbox::read_jsonl_from(&jsonl, offset).unwrap();
    assert_eq!(rest.len(), 1);
    assert_eq!(rest[0].as_ref().unwrap().text, "A new line.");
}

#[test]
fn ollama_chunks_can_split_mid_line() {
    let mut buf = String::new();
    let mut acc = OllamaAccum::default();
    buf.push_str("{\"message\":{\"content\":\"Hel");
    let tokens = consume_ollama_buffer(&mut buf, &mut acc).unwrap();
    assert!(tokens.is_empty());
    buf.push_str("lo\"},\"done\":false}\n{\"message\":{\"content\":\"!\"},\"done\":true}\n");
    let tokens = consume_ollama_buffer(&mut buf, &mut acc).unwrap();
    assert_eq!(tokens, vec!["Hello".to_string(), "!".to_string()]);
    assert!(acc.done);
    assert_eq!(acc.content, "Hello!");
}

#[test]
fn sse_blocks_become_events() {
    let mut buf = "event: assistant\ndata: {\"text\":\"On \"}\n\nevent: result\ndata: {\"text\":\"On it.\"}\n\n".to_string();
    let events = consume_sse(&mut buf);
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].event, "assistant");
    assert!(buf.is_empty());
    assert!(parse_run_id(&json!({"run": {"id": "run-1"}})).as_deref() == Some("run-1"));
    let agents = parse_agent_list(
        &json!({"items": [{"id": "bc-site", "name": "Atlas site", "status": "ACTIVE"}]}),
    );
    assert_eq!(agents[0].name, "Atlas site");
    let lines = parse_v0_conversation(&json!({"messages": [
        {"id": "msg_1", "type": "user_message", "text": "Hello"},
        {"id": "msg_2", "type": "assistant_message", "text": "Hi"}
    ]}));
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0].role, "user");
}

#[test]
fn auth_header_and_scrub_keep_secrets_out_of_errors() {
    assert_eq!(normalize_auth_header("desk-token"), "Bearer desk-token");
    assert_eq!(
        normalize_auth_header("Bearer desk-token"),
        "Bearer desk-token"
    );
    let body = relay_body("products", "hi", "m-1");
    assert_eq!(
        body,
        json!({"bot": "products", "text": "hi", "reply_to_id": "m-1"})
    );
    let secret = "cursor_test_key_9f3a";
    let cleaned = scrub(secret, &format!("Cursor returned 500. nope {secret}"));
    assert!(!cleaned.contains(secret));
    assert!(cleaned.contains("[redacted]"));
}

#[test]
fn env_secret_wins_without_touching_the_keyring() {
    let name = "ATLA_OFFICE_TEST_SECRET_PROTOCOL";
    std::env::set_var(name, "from-env");
    let value = secrets::resolve_secret(name, "this-account-is-not-looked-up");
    std::env::remove_var(name);
    assert_eq!(value.as_deref(), Some("from-env"));
}

#[tokio::test]
async fn relay_posts_the_protocol_body() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/hook"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;
    let connector = RelayConnector {
        url: format!("{}/hook", server.uri()),
        auth: Some("desk-token".into()),
        client: client(),
    };
    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
    let tokens = TokenTx::new(tx);
    let outcome = connector
        .dispatch(outbound("products", "Sketch the shelf"), &tokens)
        .await
        .unwrap();
    assert!(matches!(outcome, DispatchOutcome::Accepted));
    let requests = server.received_requests().await.unwrap();
    let body: Value = requests[0].body_json().unwrap();
    assert_eq!(
        body,
        json!({"bot": "products", "text": "Sketch the shelf", "reply_to_id": "m-user"})
    );
    let header = requests[0]
        .headers
        .get("authorization")
        .unwrap()
        .to_str()
        .unwrap();
    assert_eq!(header, "Bearer desk-token");
}

#[tokio::test]
async fn relay_mirrors_a_reply_in_the_response_body() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/hook"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "text": "The shelf sketch is up."
        })))
        .expect(1)
        .mount(&server)
        .await;
    let connector = RelayConnector {
        url: format!("{}/hook", server.uri()),
        auth: None,
        client: client(),
    };
    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
    let outcome = connector
        .dispatch(outbound("products", "Sketch the shelf"), &TokenTx::new(tx))
        .await
        .unwrap();
    assert!(matches!(outcome, DispatchOutcome::Reply(text) if text == "The shelf sketch is up."));
    assert_eq!(reply_text(""), None);
    assert_eq!(reply_text("ok"), None);
    assert_eq!(reply_text("{\"ok\":true}"), None);
}

#[test]
fn history_links_a_remote_transcript_without_duplicating() {
    let dir = tempfile::tempdir().unwrap();
    let path = atla_office::history::path_for(dir.path(), "atlas").unwrap();
    atla_office::history::append(
        &path,
        &StoredMessage {
            id: "m-user".into(),
            role: "user".into(),
            text: "Sketch the shelf".into(),
            ts: "2026-11-12T08:00:00Z".into(),
            read: true,
            in_reply_to: None,
            remote_id: None,
        },
    )
    .unwrap();
    atla_office::history::append(
        &path,
        &StoredMessage {
            id: "m-user:reply".into(),
            role: "assistant".into(),
            text: "The shelf sketch is up.".into(),
            ts: "2026-11-12T08:00:01Z".into(),
            read: false,
            in_reply_to: Some("m-user".into()),
            remote_id: None,
        },
    )
    .unwrap();
    let added = atla_office::history::mirror_lines(
        &path,
        &[
            TranscriptLine {
                remote_id: "remote-user".into(),
                role: "user".into(),
                text: "Sketch the shelf".into(),
                ts: None,
            },
            TranscriptLine {
                remote_id: "remote-assistant".into(),
                role: "assistant".into(),
                text: "The shelf sketch is up.".into(),
                ts: None,
            },
            TranscriptLine {
                remote_id: "remote-extra".into(),
                role: "assistant".into(),
                text: "I also moved the lamp.".into(),
                ts: None,
            },
        ],
        false,
    )
    .unwrap();
    assert_eq!(added.len(), 1);
    assert_eq!(added[0].text, "I also moved the lamp.");
    assert!(!added[0].read);
    let stored = atla_office::history::read_all(&path);
    assert_eq!(stored.len(), 3);
    assert_eq!(stored[0].remote_id.as_deref(), Some("remote-user"));
    assert_eq!(stored[1].remote_id.as_deref(), Some("remote-assistant"));
    assert!(atla_office::history::already_mirrored(
        &path,
        "assistant",
        "The shelf sketch is up.",
        None,
    ));
    assert!(!atla_office::history::already_mirrored(
        &path,
        "assistant",
        "A different reply.",
        Some("m-user"),
    ));
}

#[tokio::test]
async fn ollama_streams_tokens() {
    let server = MockServer::start().await;
    let body = concat!(
        "{\"message\":{\"role\":\"assistant\",\"content\":\"Hello\"},\"done\":false}\n",
        "{\"message\":{\"role\":\"assistant\",\"content\":\" there\"},\"done\":true}\n",
    );
    Mock::given(method("POST"))
        .and(path("/api/chat"))
        .respond_with(ResponseTemplate::new(200).set_body_string(body))
        .expect(1)
        .mount(&server)
        .await;
    let connector = OllamaConnector {
        base_url: server.uri(),
        model: "qwen3:8b".into(),
        echo_if_unavailable: false,
        client: client(),
    };
    let mut req = outbound("local-atlas", "ping");
    req.messages = vec![
        json!({"role": "system", "content": "Be brief."}),
        json!({"role": "user", "content": "ping"}),
    ];
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let tokens = TokenTx::new(tx);
    let outcome = connector.dispatch(req, &tokens).await.unwrap();
    drop(tokens);
    let mut streamed = String::new();
    while let Some(token) = rx.recv().await {
        streamed.push_str(&token);
    }
    assert_eq!(streamed, "Hello there");
    assert!(matches!(outcome, DispatchOutcome::Reply(text) if text == "Hello there"));
    let requests = server.received_requests().await.unwrap();
    let sent: Value = requests[0].body_json().unwrap();
    assert_eq!(sent["model"], "qwen3:8b");
    assert_eq!(sent["stream"], true);
    assert_eq!(sent["messages"][0]["content"], "Be brief.");
}

#[tokio::test]
async fn echo_streams_when_asked() {
    let connector = EchoConnector;
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let tokens = TokenTx::new(tx);
    let outcome = connector
        .dispatch(outbound("Local Atlas", "hello"), &tokens)
        .await
        .unwrap();
    drop(tokens);
    let mut streamed = String::new();
    while let Some(token) = rx.recv().await {
        streamed.push_str(&token);
    }
    let DispatchOutcome::Reply(text) = outcome else {
        panic!("echo should reply");
    };
    assert_eq!(streamed, text);
    assert!(text.contains("Echo mode is on"));
    assert!(text.contains("hello"));
}

#[tokio::test]
async fn cursor_lists_follows_up_and_reads_the_conversation() {
    let server = MockServer::start().await;
    let key = "cursor_test_key_9f3a";
    Mock::given(method("GET"))
        .and(path("/v1/agents"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "items": [
                {"id": "bc-site", "name": "Atlas site", "status": "ACTIVE", "url": "https://cursor.com/agents/bc-site"},
                {"id": "bc-play", "name": "Roleplay", "status": "IDLE", "url": ""}
            ]
        })))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/agents/bc-site/runs"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "run": {"id": "run-1", "status": "CREATING"}
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/agents/bc-site/runs/run-1/stream"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/event-stream")
                .set_body_string(
                    "event: assistant\ndata: {\"text\":\"On \"}\n\nevent: assistant\ndata: {\"text\":\"it.\"}\n\nevent: result\ndata: {\"status\":\"FINISHED\",\"text\":\"On it.\"}\n\nevent: done\ndata: {}\n\n",
                ),
        )
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v0/agents/bc-site/conversation"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "bc-site",
            "messages": [
                {"id": "msg_1", "type": "user_message", "text": "Add the mark"},
                {"id": "msg_2", "type": "assistant_message", "text": "The O is on the wall."}
            ]
        })))
        .mount(&server)
        .await;

    let connector = CursorConnector {
        base_url: server.uri(),
        api_key: key.into(),
        auth_style: "bearer".into(),
        agent_id: "bc-site".into(),
        client: client(),
    };
    let agents = connector.list_agents().await.unwrap();
    assert_eq!(agents.len(), 2);
    assert_eq!(agents[0].status, "ACTIVE");
    let lines = connector.transcript().await.unwrap();
    assert_eq!(lines[1].text, "The O is on the wall.");

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let tokens = TokenTx::new(tx);
    let outcome = connector
        .dispatch(outbound("atlas-site", "Put the mark on the wall"), &tokens)
        .await
        .unwrap();
    drop(tokens);
    let mut streamed = String::new();
    while let Some(token) = rx.recv().await {
        streamed.push_str(&token);
    }
    assert_eq!(streamed, "On it.");
    assert!(matches!(outcome, DispatchOutcome::Reply(text) if text == "On it."));

    let requests = server.received_requests().await.unwrap();
    let post = requests
        .iter()
        .find(|request| request.method == "POST")
        .unwrap();
    assert_eq!(
        post.headers.get("authorization").unwrap().to_str().unwrap(),
        format!("Bearer {key}")
    );
    let sent: Value = post.body_json().unwrap();
    assert_eq!(sent["prompt"]["text"], "Put the mark on the wall");
}

#[tokio::test]
async fn cursor_errors_do_not_echo_the_key() {
    let server = MockServer::start().await;
    let key = "cursor_test_key_9f3a";
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(500).set_body_string(format!("denied {key}")))
        .mount(&server)
        .await;
    let connector = CursorConnector {
        base_url: server.uri(),
        api_key: key.into(),
        auth_style: "bearer".into(),
        agent_id: "bc-site".into(),
        client: client(),
    };
    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
    let err = connector
        .dispatch(outbound("atlas-site", "hi"), &TokenTx::new(tx))
        .await
        .unwrap_err();
    assert!(!err.contains(key), "{err}");
    assert!(err.contains("[redacted]"), "{err}");
}

#[test]
fn history_round_trip_marks_unread() {
    let dir = tempfile::tempdir().unwrap();
    let path = atla_office::history::path_for(dir.path(), "atlas").unwrap();
    let message = StoredMessage {
        id: "m-1".into(),
        role: "assistant".into(),
        text: "Morning.".into(),
        ts: "2026-11-12T08:00:00Z".into(),
        read: false,
        in_reply_to: None,
        remote_id: None,
    };
    atla_office::history::append(&path, &message).unwrap();
    assert!(atla_office::history::has_unread(&path));
    atla_office::history::mark_read(&path).unwrap();
    assert!(!atla_office::history::has_unread(&path));
    let reply = InboxReply {
        bot_key: "atlas".into(),
        text: "Morning.".into(),
        in_reply_to: None,
        ts: Some("2026-11-12T08:00:00Z".into()),
    };
    let id = inbox::content_id("one.json", &reply);
    assert_eq!(id, inbox::content_id("one.json", &reply));
}
