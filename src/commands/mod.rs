pub mod auth;
pub mod me;
pub mod profiles;
// Uncommented by later tasks as each module is created:
pub mod agents;
pub mod circles;
pub mod comments;
pub mod connectors;
pub mod dm;
pub mod export;
pub mod files;
pub mod groups;
pub mod inbox;
pub mod insights;
pub mod labels;
pub mod links;
pub mod nests;
pub mod notifications;
pub mod plan;
pub mod projects;
pub mod queues;
pub mod recurrence;
pub mod roles;
pub mod search;
pub mod tensions;
pub mod users;
pub mod webhooks;
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
    pub workspace: Option<String>,
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
        g.workspace.as_deref(),
        g.output,
    )
    .await?;
    let client = NestrClient::with_refresh(cfg.api_base.clone(), &cfg.bearer, cfg.refresh.clone())?
        .print_notices(cfg.output);
    Ok((cfg, client))
}

/// `--unread` / `--read`: the read-marker filter the posts, DM and queue routes share.
/// Neither flag means no filter.
#[derive(clap::Args, Debug, Clone, Default)]
pub struct ReadFilter {
    /// Only what you have not read yet.
    #[arg(long, conflicts_with = "read")]
    pub unread: bool,
    /// Only what you have already read.
    #[arg(long)]
    pub read: bool,
}

impl ReadFilter {
    /// The `unread` query value, if a filter was asked for.
    pub fn param(&self) -> Option<&'static str> {
        match (self.unread, self.read) {
            (true, _) => Some("true"),
            (_, true) => Some("false"),
            _ => None,
        }
    }
}
