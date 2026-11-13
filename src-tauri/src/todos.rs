use std::fs;
use std::path::Path;

use serde_json::{Map, Value};

use crate::model::{new_id, now_iso, MetricDto, TodoDto};

#[derive(Debug)]
struct ListedFile {
    root: Value,
    key: Option<String>,
    items: Vec<Value>,
}

#[derive(Debug)]
pub struct TodoView {
    pub todos: Vec<TodoDto>,
    pub focus: Option<String>,
    pub metrics: Vec<MetricDto>,
    pub updated: Option<String>,
    pub warning: Option<String>,
}

impl TodoView {
    pub fn signature(&self) -> String {
        let mut sig = self.focus.clone().unwrap_or_default();
        for item in &self.todos {
            sig.push('\n');
            sig.push_str(&item.id);
            sig.push('\t');
            sig.push(if item.done { '1' } else { '0' });
            sig.push('\t');
            sig.push_str(&item.source);
            sig.push('\t');
            sig.push(if item.today { 'T' } else { 't' });
            sig.push('\t');
            sig.push_str(&item.text);
        }
        for metric in &self.metrics {
            sig.push('\n');
            sig.push(if metric.bad { '!' } else { '.' });
            sig.push('\t');
            sig.push_str(&metric.label);
            sig.push('\t');
            sig.push_str(&metric.value);
        }
        if let Some(updated) = &self.updated {
            sig.push('\n');
            sig.push_str(updated);
        }
        sig
    }
}

const LIST_KEYS: [&str; 4] = ["items", "todos", "tasks", "list"];
const TEXT_KEYS: [&str; 6] = ["text", "title", "content", "task", "name", "label"];
const DONE_KEYS: [&str; 4] = ["done", "completed", "checked", "complete"];

pub fn view(todo_path: &Path, today_path: &Path) -> Result<TodoView, String> {
    let todo = load_list(todo_path)?;
    let mut todos = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for (index, raw) in todo.items.iter().enumerate() {
        if let Some(item) = project(raw, index, "todo") {
            seen.insert(item.id.clone());
            todos.push(item);
        }
    }
    let mut focus = None;
    let mut metrics = Vec::new();
    let mut updated = None;
    let mut warning = None;
    if today_path.exists() {
        match fs::read_to_string(today_path) {
            Ok(text) if text.trim().is_empty() => {}
            Ok(text) => match serde_json::from_str::<Value>(&text) {
                Ok(root) => {
                    metrics = metrics_of(&root);
                    updated = updated_of(&root);
                    focus = if metrics.is_empty() {
                        focus_of(&root)
                    } else {
                        None
                    };
                    let mut extra = Vec::new();
                    collect_tasks(&root, &mut extra, 0);
                    for (index, raw) in extra.iter().enumerate() {
                        if let Some(item) = project(raw, index, "today") {
                            if seen.contains(&item.id) {
                                continue;
                            }
                            seen.insert(item.id.clone());
                            todos.push(item);
                        }
                    }
                }
                Err(err) => {
                    warning = Some(format!(
                        "The today cache is not JSON ({err}). The todo list was left as it is."
                    ));
                }
            },
            Err(err) => {
                warning = Some(format!("The today cache could not be read ({err})."));
            }
        }
    }
    todos.sort_by(|left, right| right.today.cmp(&left.today));
    Ok(TodoView {
        todos,
        focus,
        metrics,
        updated,
        warning,
    })
}

pub fn add(todo_path: &Path, today_path: &Path, text: &str) -> Result<TodoView, String> {
    let text = text.trim();
    if text.is_empty() {
        return Err("Write an item first.".into());
    }
    if text.chars().count() > 240 {
        return Err("That item is too long for the board.".into());
    }
    let mut todo = load_list(todo_path)?;
    todo.items.push(serde_json::json!({
        "id": new_id(),
        "text": text,
        "done": false,
        "createdAt": now_iso(),
    }));
    save(todo_path, &todo)?;
    view(todo_path, today_path)
}

pub fn toggle(todo_path: &Path, today_path: &Path, id: &str) -> Result<TodoView, String> {
    let mut todo = load_list(todo_path)?;
    if let Some(raw) = find_mut(&mut todo.items, id) {
        flip(raw);
        save(todo_path, &todo)?;
        return view(todo_path, today_path);
    }
    if let Some(raw) = find_today(today_path, id)? {
        let mut copy = raw;
        if !copy.is_object() {
            let text = copy.as_str().unwrap_or("").to_string();
            copy = serde_json::json!({ "text": text });
        }
        if let Some(obj) = copy.as_object_mut() {
            obj.insert("id".into(), Value::String(id.to_string()));
        }
        flip(&mut copy);
        todo.items.push(copy);
        save(todo_path, &todo)?;
        return view(todo_path, today_path);
    }
    Err(format!("No board item matches {id}."))
}

fn find_mut<'a>(items: &'a mut [Value], id: &str) -> Option<&'a mut Value> {
    for (index, raw) in items.iter_mut().enumerate() {
        if project(raw, index, "todo").is_some_and(|item| item.id == id) {
            return Some(raw);
        }
    }
    None
}

fn find_today(today_path: &Path, id: &str) -> Result<Option<Value>, String> {
    if !today_path.exists() {
        return Ok(None);
    }
    let text = fs::read_to_string(today_path).map_err(|err| err.to_string())?;
    let root: Value = serde_json::from_str(&text)
        .map_err(|err| format!("The today cache is not JSON ({err}). Nothing was written."))?;
    let mut extra = Vec::new();
    collect_tasks(&root, &mut extra, 0);
    for (index, raw) in extra.iter().enumerate() {
        if project(raw, index, "today").is_some_and(|item| item.id == id) {
            return Ok(Some(raw.clone()));
        }
    }
    Ok(None)
}

fn load_list(path: &Path) -> Result<ListedFile, String> {
    if !path.exists() {
        return Ok(ListedFile {
            root: serde_json::json!({}),
            key: Some("items".into()),
            items: Vec::new(),
        });
    }
    let text = fs::read_to_string(path).map_err(|err| {
        format!(
            "Could not read {}. Nothing was written. {err}",
            path.display()
        )
    })?;
    if text.trim().is_empty() {
        return Err(format!("{} is empty. Nothing was written.", path.display()));
    }
    let root: Value = serde_json::from_str(&text).map_err(|err| {
        format!(
            "{} is not JSON ({}). Nothing was written.",
            path.display(),
            err
        )
    })?;
    if let Some(items) = root.as_array().cloned() {
        return Ok(ListedFile {
            root,
            key: None,
            items,
        });
    }
    if let Some((key, items)) = LIST_KEYS.iter().find_map(|key| {
        root.get(*key)
            .and_then(|value| value.as_array())
            .map(|items| ((*key).to_string(), items.clone()))
    }) {
        return Ok(ListedFile {
            root,
            key: Some(key),
            items,
        });
    }
    if root.is_object() {
        Err(format!(
            "{} has no items, todos, tasks, or list array. Nothing was written.",
            path.display()
        ))
    } else {
        Err(format!(
            "{} must be a list or an object. Nothing was written.",
            path.display()
        ))
    }
}

fn save(path: &Path, file: &ListedFile) -> Result<(), String> {
    let array = Value::Array(file.items.clone());
    let root = match &file.key {
        None => array,
        Some(key) => {
            let mut root = if file.root.is_object() {
                file.root.clone()
            } else {
                serde_json::json!({})
            };
            root[key] = array;
            root
        }
    };
    let mut pretty = serde_json::to_string_pretty(&root).map_err(|err| err.to_string())?;
    pretty.push('\n');
    atomic_write(path, &pretty)
}

pub fn atomic_write(path: &Path, text: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).map_err(|err| err.to_string())?;
        }
    }
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, text).map_err(|err| err.to_string())?;
    fs::rename(&tmp, path).map_err(|err| err.to_string())?;
    Ok(())
}

fn project(raw: &Value, index: usize, source: &str) -> Option<TodoDto> {
    match raw {
        Value::String(text) => {
            let text = text.trim();
            if text.is_empty() {
                return None;
            }
            Some(TodoDto {
                id: format!("{source}-{index}"),
                text: text.to_string(),
                done: false,
                today: false,
                source: source.to_string(),
            })
        }
        Value::Object(map) => {
            let text = text_of(map)?;
            let (done, _) = done_of(map);
            Some(TodoDto {
                id: id_of(map, index, source),
                text,
                done,
                today: flag_of(map, "today"),
                source: source.to_string(),
            })
        }
        _ => None,
    }
}

fn text_of(map: &Map<String, Value>) -> Option<String> {
    for key in TEXT_KEYS {
        if let Some(text) = map.get(key).and_then(|v| v.as_str()) {
            let text = text.trim();
            if !text.is_empty() {
                return Some(text.to_string());
            }
        }
    }
    None
}

fn done_of(map: &Map<String, Value>) -> (bool, String) {
    for key in DONE_KEYS {
        if let Some(value) = map.get(key) {
            if let Some(flag) = value.as_bool() {
                return (flag, key.to_string());
            }
            if let Some(text) = value.as_str() {
                let flag = matches!(text, "true" | "done" | "yes" | "complete" | "completed");
                return (flag, key.to_string());
            }
        }
    }
    (false, "done".into())
}

fn id_of(map: &Map<String, Value>, index: usize, source: &str) -> String {
    for key in ["id", "uuid", "key"] {
        if let Some(text) = map.get(key).and_then(|v| v.as_str()) {
            let text = text.trim();
            if !text.is_empty() {
                return text.to_string();
            }
        }
        if let Some(number) = map.get(key).and_then(|v| v.as_i64()) {
            return number.to_string();
        }
    }
    format!("{source}-{index}")
}

fn flip(raw: &mut Value) {
    match raw {
        Value::String(text) => {
            let text = text.clone();
            *raw = serde_json::json!({ "text": text, "done": true });
        }
        Value::Object(map) => {
            let (done, key) = done_of(map);
            map.insert(key, Value::Bool(!done));
        }
        _ => {}
    }
}

fn flag_of(map: &Map<String, Value>, key: &str) -> bool {
    match map.get(key) {
        Some(Value::Bool(flag)) => *flag,
        Some(Value::String(text)) => matches!(text.as_str(), "true" | "yes"),
        _ => false,
    }
}

fn metrics_of(root: &Value) -> Vec<MetricDto> {
    let Some(items) = root.get("metrics").and_then(|value| value.as_array()) else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|item| {
            let map = item.as_object()?;
            let label = map
                .get("label")
                .and_then(|value| value.as_str())
                .unwrap_or("")
                .trim();
            let value = map
                .get("value")
                .and_then(|value| value.as_str())
                .unwrap_or("")
                .trim();
            if label.is_empty() || value.is_empty() {
                return None;
            }
            Some(MetricDto {
                label: label.to_string(),
                value: value.to_string(),
                bad: flag_of(map, "bad"),
            })
        })
        .collect()
}

fn updated_of(root: &Value) -> Option<String> {
    let text = root.get("updated").and_then(|value| value.as_str())?;
    let text = text.trim();
    if text.is_empty() {
        None
    } else {
        Some(text.to_string())
    }
}

fn focus_of(root: &Value) -> Option<String> {
    let obj = root.as_object()?;
    for key in ["focus", "headline", "intention"] {
        if let Some(text) = obj.get(key).and_then(|v| v.as_str()) {
            let text = text.trim();
            if !text.is_empty() {
                return Some(text.to_string());
            }
        }
    }
    None
}

fn is_task(value: &Value) -> bool {
    match value {
        Value::String(text) => !text.trim().is_empty(),
        Value::Object(map) => map.get("connector").is_none() && text_of(map).is_some(),
        _ => false,
    }
}

fn collect_tasks(value: &Value, out: &mut Vec<Value>, depth: usize) {
    if depth > 4 {
        return;
    }
    match value {
        Value::Array(items) => {
            let task_count = items.iter().filter(|item| is_task(item)).count();
            if task_count > 0 && task_count * 2 >= items.len() {
                for item in items {
                    if is_task(item) {
                        out.push(item.clone());
                    }
                }
            } else {
                for item in items {
                    collect_tasks(item, out, depth + 1);
                }
            }
        }
        Value::Object(map) => {
            for (key, child) in map {
                if key == "metrics" {
                    continue;
                }
                if matches!(
                    key.as_str(),
                    "focus" | "headline" | "intention" | "date" | "updated"
                ) && !child.is_array()
                    && !child.is_object()
                {
                    continue;
                }
                collect_tasks(child, out, depth + 1);
            }
        }
        _ => {}
    }
}
