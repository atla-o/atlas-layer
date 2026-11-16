//! Devo's productivity home, read from `dashboard.json`.
//! Atlas keeps the file updated. This app only reads it.

use std::fs;
use std::path::Path;

use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Bill {
    pub id: String,
    pub name: String,
    pub amount: String,
    pub due: String,
    pub paid: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WaitingReply {
    pub id: String,
    pub from: String,
    pub text: String,
    pub since: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Decision {
    pub id: String,
    pub title: String,
    pub detail: String,
    pub due: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Dashboard {
    pub bills: Vec<Bill>,
    pub replies: Vec<WaitingReply>,
    pub decisions: Vec<Decision>,
    pub updated: Option<String>,
    pub warning: Option<String>,
}

impl Dashboard {
    pub fn empty() -> Self {
        Self {
            bills: Vec::new(),
            replies: Vec::new(),
            decisions: Vec::new(),
            updated: None,
            warning: None,
        }
    }

    pub fn signature(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }
}

pub fn load(path: &Path) -> Dashboard {
    if !path.exists() {
        return Dashboard::empty();
    }
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) => {
            let mut dash = Dashboard::empty();
            dash.warning = Some(format!("The dashboard could not be read ({err})."));
            return dash;
        }
    };
    if text.trim().is_empty() {
        return Dashboard::empty();
    }
    let root: Value = match serde_json::from_str(&text) {
        Ok(value) => value,
        Err(err) => {
            let mut dash = Dashboard::empty();
            dash.warning = Some(format!(
                "The dashboard is not JSON ({err}). It was left as it is."
            ));
            return dash;
        }
    };
    let mut bills = list_of(&root, &["bills", "invoices"])
        .iter()
        .enumerate()
        .filter_map(|(index, item)| {
            let map = item.as_object()?;
            let name = first_str(map, &["name", "title", "payee", "label"])?;
            Some(Bill {
                id: first_str(map, &["id"]).unwrap_or_else(|| format!("bill-{index}")),
                amount: first_str(map, &["amount", "value", "total"]).unwrap_or_else(|| "—".into()),
                due: first_str(map, &["due", "dueDate", "date"]).unwrap_or_default(),
                paid: flag(map, &["paid", "done"]),
                name,
            })
        })
        .collect::<Vec<_>>();
    bills.sort_by(|left, right| left.paid.cmp(&right.paid).then(left.due.cmp(&right.due)));
    let replies = list_of(&root, &["replies", "waiting", "inbox"])
        .iter()
        .enumerate()
        .filter_map(|(index, item)| {
            let map = item.as_object()?;
            let text = first_str(map, &["text", "title", "subject"])?;
            Some(WaitingReply {
                id: first_str(map, &["id"]).unwrap_or_else(|| format!("reply-{index}")),
                from: first_str(map, &["from", "who", "name"]).unwrap_or_else(|| "Someone".into()),
                since: first_str(map, &["since", "waitingSince", "due", "date"])
                    .unwrap_or_default(),
                text,
            })
        })
        .collect();
    let decisions = list_of(&root, &["decisions", "owed"])
        .iter()
        .enumerate()
        .filter_map(|(index, item)| {
            let map = item.as_object()?;
            let title = first_str(map, &["title", "text", "name"])?;
            Some(Decision {
                id: first_str(map, &["id"]).unwrap_or_else(|| format!("decision-{index}")),
                detail: first_str(map, &["detail", "note", "why"]).unwrap_or_default(),
                due: first_str(map, &["due", "dueDate", "date"]).unwrap_or_default(),
                title,
            })
        })
        .collect();
    Dashboard {
        bills,
        replies,
        decisions,
        updated: root
            .get("updated")
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|text| !text.is_empty())
            .map(str::to_string),
        warning: None,
    }
}

fn list_of<'a>(root: &'a Value, keys: &[&str]) -> Vec<&'a Value> {
    for key in keys {
        if let Some(items) = root.get(*key).and_then(|value| value.as_array()) {
            return items.iter().collect();
        }
    }
    Vec::new()
}

fn first_str(map: &serde_json::Map<String, Value>, keys: &[&str]) -> Option<String> {
    for key in keys {
        if let Some(text) = map.get(*key).and_then(|value| value.as_str()) {
            let text = text.trim();
            if !text.is_empty() {
                return Some(text.to_string());
            }
        }
    }
    None
}

fn flag(map: &serde_json::Map<String, Value>, keys: &[&str]) -> bool {
    for key in keys {
        match map.get(*key) {
            Some(Value::Bool(value)) => return *value,
            Some(Value::String(text)) if matches!(text.as_str(), "true" | "yes" | "paid") => {
                return true
            }
            _ => {}
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::load;

    #[test]
    fn reads_bills_replies_and_decisions_and_ignores_unknown_fields() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("dashboard.json");
        std::fs::write(
            &path,
            r#"{
              "updated": "Mon Nov 16 2026, 4:00 AM PT",
              "note": "kept outside the app",
              "bills": [
                {"id":"b2","name":"Later","amount":"$2","due":"2026-12-01","paid":false,"vendor":"x"},
                {"id":"b1","name":"Agent","amount":"$50","due":"2026-11-20","paid":false}
              ],
              "replies": [{"from":"Atlas","text":"Need a yes","since":"2026-11-15","mood":"calm"}],
              "decisions": [{"title":"Ship the home panel","detail":"Replace the old page","due":"2026-11-16"}]
            }"#,
        )
        .unwrap();
        let dash = load(&path);
        assert!(dash.warning.is_none());
        assert_eq!(dash.bills[0].name, "Agent");
        assert_eq!(dash.bills[1].due, "2026-12-01");
        assert_eq!(dash.replies[0].from, "Atlas");
        assert_eq!(dash.decisions[0].title, "Ship the home panel");
        assert_eq!(dash.updated.as_deref(), Some("Mon Nov 16 2026, 4:00 AM PT"));
        assert!(load(&dir.path().join("missing.json")).bills.is_empty());
    }
}
