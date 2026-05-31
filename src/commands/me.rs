use anyhow::Result;
use serde_json::Value;

use crate::api_client::NestrClient;
use crate::config::{self, OutputFormat};
use crate::render;

/// GET /users/me — returns the raw user object.
pub async fn fetch_me(client: &NestrClient) -> crate::error::Result<Value> {
    client.get("/users/me", &[]).await
}

/// `nestr me` — resolve the active profile, fetch identity, render.
pub async fn run(
    profile: Option<&str>,
    api_key: Option<&str>,
    host: Option<&str>,
    output: Option<OutputFormat>,
) -> Result<()> {
    let cfg = config::resolve(profile, api_key, host, output).await?;
    let client = NestrClient::new(cfg.api_base.clone(), &cfg.bearer)?;
    let me = fetch_me(&client).await?;
    render::render_object(&me, cfg.output, |v| {
        let name = v
            .get("profile")
            .and_then(|p| p.get("fullName"))
            .and_then(|n| n.as_str())
            .unwrap_or("-");
        let username = v.get("username").and_then(|u| u.as_str()).unwrap_or("-");
        let id = v.get("_id").and_then(|i| i.as_str()).unwrap_or("-");
        println!("{name}  <{username}>  [{id}]");
        println!(
            "profile: {}  workspace: {}",
            cfg.profile_name, cfg.workspace_id
        );
    })?;
    Ok(())
}
