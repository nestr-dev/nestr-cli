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
        println!(
            "profile: {}  workspace: {}",
            cfg.profile_name, cfg.workspace_id
        );
    })?;
    Ok(())
}
