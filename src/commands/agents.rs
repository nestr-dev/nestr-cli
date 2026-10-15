use anyhow::Result;
use clap::{Subcommand, ValueEnum};
use serde_json::{json, Map, Value};

use crate::api_client::{unwrap_data, NestrClient};
use crate::commands::{resolve_client, GlobalArgs};
use crate::error::NestrError;
use crate::render;
use crate::safety;

#[derive(Subcommand)]
pub enum AgentsCmd {
    /// List the workspace's agents.
    List,
    /// Get an agent by its user id.
    Get { agent_id: String },
    /// Create an agent (admin).
    Create {
        /// The agent's own name (e.g. "Collab"), not the name of the work it does.
        #[arg(long)]
        name: String,
        /// What the agent is like: its character, not its instructions.
        #[arg(long)]
        description: Option<String>,
        /// Create an assistant: an agent that can never be assigned to a role.
        #[arg(long)]
        assistant: bool,
        /// Runtime wiring as JSON, e.g. '{"runtimeCallbackUrl":"https://…","tokenTtlSeconds":300}'.
        /// Omit for an agent that runs on Nestr's own runtime.
        #[arg(long, value_name = "JSON")]
        config: Option<String>,
    },
    /// Run an agent now on a nest (needs a user or OAuth login, not an API key).
    Run {
        agent_id: String,
        /// The role, project or task the run is pinned to.
        #[arg(long)]
        nest: String,
        /// What this run is for. Omit for a plain "advance this item" run.
        #[arg(long)]
        message: Option<String>,
    },
    /// What connectors an agent can use, and why not when it can't.
    Connectors { agent_id: String },
    /// How an agent operates on a nest: reactive or autonomous on a cadence.
    Mode {
        #[command(subcommand)]
        cmd: ModeCmd,
    },
}

#[derive(Subcommand)]
pub enum ModeCmd {
    /// Show an agent's mode on a nest, and where it comes from.
    Get {
        agent_id: String,
        #[arg(long)]
        nest: String,
    },
    /// Set an agent's mode on a nest (needs a user or OAuth login, not an API key).
    Set {
        agent_id: String,
        /// The role, project or task this setting is for.
        #[arg(long)]
        nest: String,
        /// reactive acts when triggered; autonomous works on a cadence.
        #[arg(long, value_enum)]
        mode: AgentMode,
        /// Required with autonomous: "continuous", or an RRULE such as
        /// FREQ=WEEKLY;BYDAY=MO,WE,FR (hourly, daily and weekly are shorthand).
        #[arg(long, required_if_eq("mode", "autonomous"))]
        recheck: Option<String>,
        /// ISO date-time the schedule starts at; also sets the time of day it runs.
        /// A value with no offset is read as local time in --tz. Defaults to now.
        #[arg(long, requires = "recheck")]
        start: Option<String>,
        /// IANA time zone the schedule keeps its local time in (default UTC).
        #[arg(long, requires = "recheck")]
        tz: Option<String>,
    },
    /// Clear a nest's own setting so the agent inherits again.
    Clear {
        agent_id: String,
        #[arg(long)]
        nest: String,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum AgentMode {
    Reactive,
    Autonomous,
}

/// Parse a JSON-object flag value, naming the flag in the error.
pub fn parse_json_object(flag: &str, raw: &str) -> crate::error::Result<Value> {
    match serde_json::from_str::<Value>(raw) {
        Ok(v @ Value::Object(_)) => Ok(v),
        _ => Err(NestrError::Validation(format!(
            "{flag} expects a JSON object, got '{raw}'"
        ))),
    }
}

/// Assemble the `POST /workspaces/:ws/agents` body. Optional fields are omitted.
pub fn create_body(
    name: String,
    description: Option<String>,
    assistant: bool,
    config: Option<Value>,
) -> Value {
    let mut body = Map::new();
    body.insert("name".into(), name.into());
    if let Some(d) = description {
        body.insert("description".into(), d.into());
    }
    if assistant {
        body.insert("roleAssignable".into(), false.into());
    }
    if let Some(c) = config {
        body.insert("agentConfig".into(), c);
    }
    Value::Object(body)
}

/// Assemble the `PUT /nests/:id/agents/:userId/mode` body. `None` clears the
/// nest's own setting. The cadence only travels with autonomous: the server
/// ignores it on reactive, and sending it there reads as a schedule never set.
pub fn mode_body(
    mode: Option<AgentMode>,
    recheck: Option<String>,
    start: Option<String>,
    tz: Option<String>,
) -> crate::error::Result<Value> {
    match mode {
        None => Ok(json!({ "mode": null })),
        Some(AgentMode::Reactive) => {
            if recheck.is_some() || start.is_some() || tz.is_some() {
                return Err(NestrError::Validation(
                    "--recheck, --start and --tz only apply to --mode autonomous".into(),
                ));
            }
            Ok(json!({ "mode": "reactive" }))
        }
        Some(AgentMode::Autonomous) => {
            let Some(recheck) = recheck else {
                return Err(NestrError::Validation(
                    "--mode autonomous needs --recheck (continuous, or an RRULE such as FREQ=DAILY)"
                        .into(),
                ));
            };
            let mut body = Map::new();
            body.insert("mode".into(), "autonomous".into());
            body.insert("recheck".into(), recheck.into());
            if let Some(s) = start {
                body.insert("start".into(), s.into());
            }
            if let Some(t) = tz {
                body.insert("tzid".into(), t.into());
            }
            Ok(Value::Object(body))
        }
    }
}

pub async fn fetch_list(client: &NestrClient, ws: &str) -> crate::error::Result<Value> {
    let raw: Value = client.get(&format!("/workspaces/{ws}/agents"), &[]).await?;
    Ok(unwrap_data(raw).0)
}

pub async fn fetch_get(client: &NestrClient, ws: &str, agent: &str) -> crate::error::Result<Value> {
    let raw: Value = client
        .get(&format!("/workspaces/{ws}/agents/{agent}"), &[])
        .await?;
    Ok(unwrap_data(raw).0)
}

pub async fn create_agent(
    client: &NestrClient,
    ws: &str,
    body: &Value,
) -> crate::error::Result<Value> {
    let raw: Value = client
        .post(&format!("/workspaces/{ws}/agents"), body)
        .await?;
    Ok(unwrap_data(raw).0)
}

pub async fn run_agent(
    client: &NestrClient,
    ws: &str,
    agent: &str,
    nest: &str,
    message: Option<&str>,
) -> crate::error::Result<Value> {
    let mut body = Map::new();
    body.insert("nestId".into(), nest.into());
    if let Some(m) = message {
        body.insert("message".into(), m.into());
    }
    let raw: Value = client
        .post(
            &format!("/workspaces/{ws}/agents/{agent}/run"),
            &Value::Object(body),
        )
        .await?;
    Ok(unwrap_data(raw).0)
}

pub async fn fetch_reach(
    client: &NestrClient,
    ws: &str,
    agent: &str,
) -> crate::error::Result<Value> {
    let raw: Value = client
        .get(&format!("/workspaces/{ws}/agents/{agent}/connectors"), &[])
        .await?;
    Ok(unwrap_data(raw).0)
}

pub async fn fetch_mode(
    client: &NestrClient,
    nest: &str,
    agent: &str,
) -> crate::error::Result<Value> {
    let raw: Value = client
        .get(&format!("/nests/{nest}/agents/{agent}/mode"), &[])
        .await?;
    Ok(unwrap_data(raw).0)
}

pub async fn set_mode(
    client: &NestrClient,
    nest: &str,
    agent: &str,
    body: &Value,
) -> crate::error::Result<Value> {
    let raw: Value = client
        .put(&format!("/nests/{nest}/agents/{agent}/mode"), body)
        .await?;
    Ok(unwrap_data(raw).0)
}

pub async fn run(cmd: AgentsCmd, g: &GlobalArgs) -> Result<()> {
    let (cfg, client) = resolve_client(g).await?;
    match cmd {
        AgentsCmd::List => {
            let ws = cfg.require_workspace()?;
            let data = fetch_list(&client, ws).await?;
            render::output_agents(&data, cfg.output)?;
        }
        AgentsCmd::Get { agent_id } => {
            let ws = cfg.require_workspace()?;
            let data = fetch_get(&client, ws, &agent_id).await?;
            render::agent_detail(&data, cfg.output)?;
        }
        AgentsCmd::Create {
            name,
            description,
            assistant,
            config,
        } => {
            safety::enforce_read_only(g.read_only, "agents create")?;
            let ws = cfg.require_workspace()?;
            let config = config
                .as_deref()
                .map(|c| parse_json_object("--config", c))
                .transpose()?;
            let body = create_body(name, description, assistant, config);
            let data = create_agent(&client, ws, &body).await?;
            render::agent_detail(&data, cfg.output)?;
        }
        AgentsCmd::Run {
            agent_id,
            nest,
            message,
        } => {
            safety::enforce_read_only(g.read_only, "agents run")?;
            let ws = cfg.require_workspace()?;
            let data = run_agent(&client, ws, &agent_id, &nest, message.as_deref()).await?;
            render::agent_run(&data, cfg.output)?;
        }
        AgentsCmd::Connectors { agent_id } => {
            let ws = cfg.require_workspace()?;
            let data = fetch_reach(&client, ws, &agent_id).await?;
            render::output_agent_reach(&data, cfg.output)?;
        }
        // The mode routes are nest-scoped, so they need no workspace.
        AgentsCmd::Mode { cmd } => match cmd {
            ModeCmd::Get { agent_id, nest } => {
                let data = fetch_mode(&client, &nest, &agent_id).await?;
                render::agent_mode(&data, cfg.output)?;
            }
            ModeCmd::Set {
                agent_id,
                nest,
                mode,
                recheck,
                start,
                tz,
            } => {
                safety::enforce_read_only(g.read_only, "agents mode set")?;
                let body = mode_body(Some(mode), recheck, start, tz)?;
                let data = set_mode(&client, &nest, &agent_id, &body).await?;
                render::agent_mode(&data, cfg.output)?;
            }
            ModeCmd::Clear { agent_id, nest } => {
                safety::enforce_read_only(g.read_only, "agents mode clear")?;
                let body = mode_body(None, None, None, None)?;
                let data = set_mode(&client, &nest, &agent_id, &body).await?;
                render::agent_mode(&data, cfg.output)?;
            }
        },
    }
    Ok(())
}
