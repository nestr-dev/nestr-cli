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
        label_codes(&self.labels).join(",")
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

/// Extract display titles from an `accountabilities`/`domains` array, whose items
/// are either `{_id, title}` objects or bare strings.
pub fn view_titles(items: &[Value]) -> Vec<String> {
    items
        .iter()
        .filter_map(|i| match i {
            Value::String(s) => Some(s.clone()),
            Value::Object(_) => i.get("title").and_then(|v| v.as_str()).map(str::to_string),
            _ => None,
        })
        .collect()
}

/// A role or a circle: a Nest plus `accountabilities[]` and `domains[]`.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct RoleView {
    #[serde(default, rename = "_id")]
    pub id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub purpose: Option<String>,
    #[serde(default)]
    pub labels: Vec<Value>,
    #[serde(default, rename = "parentId")]
    pub parent_id: Option<String>,
    #[serde(default)]
    pub accountabilities: Vec<Value>,
    #[serde(default)]
    pub domains: Vec<Value>,
}

impl RoleView {
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
    pub fn acc_titles(&self) -> Vec<String> {
        view_titles(&self.accountabilities)
    }
    pub fn domain_titles(&self) -> Vec<String> {
        view_titles(&self.domains)
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct UserProfile {
    #[serde(default, rename = "fullName")]
    pub full_name: Option<String>,
    #[serde(default)]
    pub email: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct UserView {
    #[serde(default, rename = "_id")]
    pub id: String,
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub profile: Option<UserProfile>,
    #[serde(default)]
    pub bot: Option<bool>,
}

impl UserView {
    pub fn full_name(&self) -> String {
        self.profile
            .as_ref()
            .and_then(|p| p.full_name.clone())
            .unwrap_or_default()
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct GroupView {
    #[serde(default, rename = "_id")]
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct AppView {
    #[serde(default, rename = "_id")]
    pub id: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub enabled: bool,
}

/// Extract label identifiers (handles `["code"]` and `[{code|title|_id}]`), skipping
/// anything that yields no string. The single source of truth for reading a nest's labels.
pub fn label_codes(labels: &[Value]) -> Vec<String> {
    labels
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
        .collect()
}

/// Join label identifiers for display (handles `["code"]` and `[{code|title|_id}]`).
pub fn join_labels(labels: &[Value]) -> String {
    label_codes(labels).join(",")
}

/// A tension: a Nest with a computed `status`.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct TensionView {
    #[serde(default, rename = "_id")]
    pub id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub labels: Vec<Value>,
    #[serde(default, rename = "parentId")]
    pub parent_id: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub completed: Option<bool>,
}

/// A proposal part: contains one or more proposal items.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct PartView {
    #[serde(default, rename = "_id")]
    pub id: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub items: Vec<Value>,
}

/// One line of a computed diff for a proposal part.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ChangeView {
    #[serde(default, rename = "nestId")]
    pub nest_id: Option<String>,
    #[serde(default)]
    pub variable: String,
    #[serde(default, rename = "newValue")]
    pub new_value: Value,
    #[serde(default, rename = "oldValue")]
    pub old_value: Value,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct StatusResponse {
    #[serde(default, rename = "userId")]
    pub user_id: Option<String>,
    #[serde(default)]
    pub response: Option<String>,
    #[serde(default, rename = "votedAt")]
    pub voted_at: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct StatusView {
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub responses: Vec<StatusResponse>,
    #[serde(default)]
    pub autoapprove: Option<String>,
}

/// An accountability/domain child of a proposal part.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ChildView {
    #[serde(default, rename = "_id")]
    pub id: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub labels: Vec<Value>,
    #[serde(default, rename = "linkId")]
    pub link_id: Option<String>,
}

/// A graph-link edge: the linked nest, annotated with its relation + direction.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct LinkView {
    #[serde(default, rename = "_id")]
    pub id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub labels: Vec<Value>,
    #[serde(default)]
    pub relation: Option<String>,
    #[serde(default)]
    pub direction: Option<String>,
}

/// A workspace webhook subscription.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct WebhookView {
    #[serde(default, rename = "_id")]
    pub id: String,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default, rename = "type")]
    pub type_: Option<String>,
    #[serde(default)]
    pub event: Option<String>,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default, rename = "ancestorId")]
    pub ancestor_id: Option<String>,
    #[serde(default, rename = "createdAt")]
    pub created_at: Option<String>,
    #[serde(default, rename = "triggerCount")]
    pub trigger_count: Option<u64>,
    #[serde(default, rename = "errorCount")]
    pub error_count: Option<u64>,
}

/// An organizational-health metric (insight).
#[derive(Debug, Clone, Default, Deserialize)]
pub struct InsightView {
    #[serde(default, rename = "nestId")]
    pub nest_id: Option<String>,
    #[serde(default, rename = "type")]
    pub type_: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default, rename = "currentValue")]
    pub current_value: Option<f64>,
    #[serde(default, rename = "compareValue")]
    pub compare_value: Option<f64>,
    #[serde(default, rename = "dataType")]
    pub data_type: Option<String>,
    #[serde(default)]
    pub goal: Option<String>,
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

    #[test]
    fn role_view_extracts_accountability_and_domain_titles() {
        let r: RoleView = serde_json::from_value(json!({
            "_id": "r1", "title": "Facilitator", "labels": ["role"],
            "accountabilities": [{"_id": "a1", "title": "Run meetings"}, "Keep records"],
            "domains": [{"_id": "d1", "title": "The agenda"}]
        }))
        .unwrap();
        assert_eq!(r.acc_titles(), vec!["Run meetings", "Keep records"]);
        assert_eq!(r.domain_titles(), vec!["The agenda"]);
        assert_eq!(r.labels_str(), "role");
    }

    #[test]
    fn user_view_pulls_full_name_and_tolerates_missing_profile() {
        let u: UserView = serde_json::from_value(
            json!({"_id": "u1", "username": "a@b.c", "profile": {"fullName": "A B"}}),
        )
        .unwrap();
        assert_eq!(u.full_name(), "A B");
        let u2: UserView = serde_json::from_value(json!({"_id": "u2"})).unwrap();
        assert_eq!(u2.full_name(), "");
    }

    #[test]
    fn app_view_defaults_enabled_false() {
        let a: AppView = serde_json::from_value(json!({"_id": "okr", "title": "OKR"})).unwrap();
        assert!(!a.enabled);
    }

    #[test]
    fn tension_view_reads_status_and_tolerates_missing() {
        let t: TensionView = serde_json::from_value(
            json!({"_id":"t1","title":"Gap","labels":["circleplus-prepared-tension"],"status":"proposed"}),
        )
        .unwrap();
        assert_eq!(t.id, "t1");
        assert_eq!(t.status.as_deref(), Some("proposed"));
        let t2: TensionView = serde_json::from_value(json!({"_id":"t2","title":"x"})).unwrap();
        assert!(t2.status.is_none());
        assert_eq!(join_labels(&t.labels), "circleplus-prepared-tension");
    }

    #[test]
    fn change_view_tolerates_null_old_new() {
        let c: ChangeView = serde_json::from_value(
            json!({"variable":"role.title","newValue":"Treasurer","oldValue":null}),
        )
        .unwrap();
        assert_eq!(c.variable, "role.title");
        assert_eq!(c.new_value, json!("Treasurer"));
        assert!(c.old_value.is_null());
    }

    #[test]
    fn link_view_reads_relation_and_direction() {
        let l: LinkView = serde_json::from_value(json!({
            "_id":"n1","title":"Weekly meeting","labels":["meeting"],
            "relation":"meeting","direction":"outgoing"
        }))
        .unwrap();
        assert_eq!(l.id, "n1");
        assert_eq!(l.relation.as_deref(), Some("meeting"));
        assert_eq!(l.direction.as_deref(), Some("outgoing"));
    }

    #[test]
    fn insight_view_reads_numbers_and_renames_type() {
        let i: InsightView = serde_json::from_value(json!({
            "type":"role_count","title":"Roles","currentValue":12,"compareValue":10.0,"goal":"high"
        }))
        .unwrap();
        assert_eq!(i.type_.as_deref(), Some("role_count"));
        assert_eq!(i.current_value, Some(12.0));
        assert_eq!(i.compare_value, Some(10.0));
        let empty: InsightView = serde_json::from_value(json!({})).unwrap();
        assert!(empty.current_value.is_none());
    }

    #[test]
    fn webhook_view_renames_type_and_counts() {
        let w: WebhookView = serde_json::from_value(json!({
            "_id":"wh1","url":"https://x.test/hook","type":"nest","event":"create","triggerCount":3
        }))
        .unwrap();
        assert_eq!(w.id, "wh1");
        assert_eq!(w.type_.as_deref(), Some("nest"));
        assert_eq!(w.trigger_count, Some(3));
        let empty: WebhookView = serde_json::from_value(json!({})).unwrap();
        assert!(empty.url.is_none());
    }

    #[test]
    fn status_view_reads_responses() {
        let s: StatusView = serde_json::from_value(json!({
            "status":"proposed",
            "responses":[{"userId":"u1","response":"accepted","votedAt":"2026-06-04T00:00:00Z"}]
        }))
        .unwrap();
        assert_eq!(s.status.as_deref(), Some("proposed"));
        assert_eq!(s.responses.len(), 1);
        assert_eq!(s.responses[0].user_id.as_deref(), Some("u1"));
    }
}
