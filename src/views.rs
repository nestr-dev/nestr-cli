//! Deserialize-only "view" structs backing the text renderers. These tolerate
//! Nestr's polymorphic payloads (unknown fields ignored, missing fields default).
//! NEVER used for `-o json` output — that path prints the raw `Value`.

use serde::Deserialize;
use serde_json::Value;

/// The compact field set the CLI renders for any Nest-shaped object
/// (search hits, children, projects, inbox items, daily-plan items).
#[derive(Debug, Clone, Default, Deserialize)]
pub struct CompactNest {
    #[serde(default, rename = "_id")]
    pub id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub purpose: Option<String>,
    #[serde(default)]
    pub completed: Option<bool>,
    #[serde(default)]
    pub labels: Vec<Value>,
    #[serde(default)]
    pub due: Option<String>,
    #[serde(default, rename = "parentId")]
    pub parent_id: Option<String>,
}

impl CompactNest {
    /// Best-effort join of label identifiers: handles `["code"]` and `[{code|title|_id}]`.
    pub fn labels_str(&self) -> String {
        self.labels
            .iter()
            .filter_map(|l| match l {
                Value::String(s) => Some(s.clone()),
                Value::Object(_) => l
                    .get("code")
                    .or_else(|| l.get("title"))
                    .or_else(|| l.get("_id"))
                    .and_then(|v| v.as_str())
                    .map(str::to_string),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join(",")
    }
}

/// A comment (a Nest with `type:"comment"`). Body may live under `body` or `title`.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct PostView {
    #[serde(default, rename = "_id")]
    pub id: String,
    #[serde(default)]
    pub body: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default, rename = "createdBy")]
    pub created_by: Option<String>,
    #[serde(default, rename = "createdAt")]
    pub created_at: Option<String>,
}

impl PostView {
    pub fn text(&self) -> String {
        self.body
            .clone()
            .or_else(|| self.title.clone())
            .unwrap_or_default()
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct NotificationView {
    #[serde(default, rename = "_id")]
    pub id: String,
    #[serde(default)]
    pub group: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default, rename = "actorName")]
    pub actor_name: Option<String>,
    #[serde(default, rename = "createdAt")]
    pub created_at: Option<String>,
    #[serde(default, rename = "isRead")]
    pub is_read: bool,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct LabelView {
    #[serde(default, rename = "_id")]
    pub id: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub icon: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn compact_nest_tolerates_unknown_and_missing_fields() {
        let n: CompactNest =
            serde_json::from_value(json!({"_id":"x","title":"T","extra":"ignored"})).unwrap();
        assert_eq!(n.id, "x");
        assert_eq!(n.title, "T");
        assert!(n.due.is_none());
        assert_eq!(n.labels_str(), "");
    }

    #[test]
    fn labels_str_handles_strings_and_objects() {
        let n: CompactNest = serde_json::from_value(
            json!({"labels":["now", {"code":"project"}, {"title":"Goal"}, 42]}),
        )
        .unwrap();
        assert_eq!(n.labels_str(), "now,project,Goal");
    }

    #[test]
    fn post_text_prefers_body_then_title() {
        let p: PostView = serde_json::from_value(json!({"title":"t"})).unwrap();
        assert_eq!(p.text(), "t");
        let p: PostView = serde_json::from_value(json!({"body":"b","title":"t"})).unwrap();
        assert_eq!(p.text(), "b");
    }
}
