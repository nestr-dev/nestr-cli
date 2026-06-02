pub mod auth;
pub mod me;
pub mod profiles;
// Uncommented by later tasks as each module is created:
pub mod circles;
pub mod comments;
pub mod inbox;
pub mod labels;
pub mod nests;
pub mod notifications;
pub mod plan;
pub mod projects;
pub mod roles;
pub mod search;
pub mod work;
pub mod workspaces;

use anyhow::Result;

use crate::api_client::NestrClient;
use crate::config::{self, OutputFormat, ResolvedConfig};

/// The global flags every command shares (built once in `main`).
#[derive(Debug, Clone, Default)]
pub struct GlobalArgs {
    pub profile: Option<String>,
    pub api_key: Option<String>,
    pub host: Option<String>,
    pub output: Option<OutputFormat>,
    pub yes: bool,
    pub read_only: bool,
}

/// Resolve the active profile and build a ready client (with reactive refresh).
pub async fn resolve_client(g: &GlobalArgs) -> Result<(ResolvedConfig, NestrClient)> {
    let cfg = config::resolve(
        g.profile.as_deref(),
        g.api_key.as_deref(),
        g.host.as_deref(),
        g.output,
    )
    .await?;
    let client = NestrClient::with_refresh(cfg.api_base.clone(), &cfg.bearer, cfg.refresh.clone())?;
    Ok((cfg, client))
}
