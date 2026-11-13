use anyhow::Result;
use serde_json::Value;

use crate::api_client::{unwrap_data, NestrClient};
use crate::commands::{resolve_client, GlobalArgs};
use crate::render;

/// GET /users/me — returns the user object. The live API wraps it in
/// `{status, data}`, so unwrap defensively (unwrap_data is a no-op on a bare object).
pub async fn fetch_me(client: &NestrClient) -> crate::error::Result<Value> {
    let raw: Value = client.get("/users/me", &[]).await?;
    let (data, _, _) = unwrap_data(raw);
    Ok(data)
}

/// The caller's own user id, for a `me` given as `what` (e.g. `--user me`).
pub async fn my_id(client: &NestrClient, what: &str) -> crate::error::Result<String> {
    let me = fetch_me(client).await?;
    match me.get("_id").and_then(Value::as_str) {
        Some(id) if !id.is_empty() => Ok(id.to_string()),
        _ => Err(crate::error::NestrError::Validation(format!(
            "could not resolve `{what}`: /users/me returned no id"
        ))),
    }
}

/// One line of the caller's user labels in `ws`, or None when there are none.
pub fn user_labels_line(me: &Value, ws: &str) -> Option<String> {
    let labels: Vec<&str> = me["userLabels"][ws]["labels"]
        .as_array()?
        .iter()
        .filter_map(Value::as_str)
        .collect();
    (!labels.is_empty()).then(|| format!("labels: {}", labels.join(", ")))
}

/// `nestr me` — resolve the active profile, fetch identity, render.
pub async fn run(g: &GlobalArgs) -> Result<()> {
    let (cfg, client) = resolve_client(g).await?;
    let me = fetch_me(&client).await?;
    render::render_object(&me, cfg.output, |v| {
        let name = v
            .get("profile")
            .and_then(|p| p.get("fullName"))
            .and_then(|n| n.as_str())
            .unwrap_or("-");
        let username = v.get("username").and_then(|u| u.as_str()).unwrap_or("-");
        let id = v.get("_id").and_then(|i| i.as_str()).unwrap_or("-");
        println!(
            "{}  <{}>  [{id}]",
            render::clean_text(name),
            render::clean_text(username)
        );
        let ws = if cfg.workspace_id.is_empty() {
            "(none — full account)"
        } else {
            &cfg.workspace_id
        };
        println!("profile: {}  workspace: {ws}", cfg.profile_name);
        if let Some(line) = user_labels_line(v, &cfg.workspace_id) {
            println!("{}", render::clean_text(&line));
        }
    })?;
    Ok(())
}
