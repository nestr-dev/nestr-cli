//! The workspace connector catalog and its bindings (connections).
//!
//! No command here takes or prints a credential. The catalog and the bindings hold
//! none (the server refuses a credential-looking header in `--config`), and the
//! secret is captured out-of-band: `connections link` returns a URL a person opens
//! to sign in or enter a key. That URL carries no authority of its own.

use anyhow::Result;
use clap::{Subcommand, ValueEnum};
use serde_json::{Map, Value};

use crate::api_client::{unwrap_data, NestrClient};
use crate::commands::agents::parse_json_object;
use crate::commands::{resolve_client, GlobalArgs};
use crate::error::NestrError;
use crate::render;
use crate::safety;

#[derive(Subcommand)]
pub enum ConnectorsCmd {
    /// List the workspace's connector catalog.
    List,
    /// List the connector templates this deployment can add in one click (admin).
    Templates,
    /// Register a connector in the catalog (admin). Prefer --template.
    Register {
        /// Template id from `nestr connectors templates`. Fills in the vendor's
        /// endpoint, transport, auth strategy and OAuth client.
        #[arg(long, conflicts_with_all = ["type_", "config", "capabilities", "exposure", "auth_strategy"])]
        template: Option<String>,
        /// Unique name in the catalog (defaults to the template's name).
        #[arg(long)]
        name: Option<String>,
        #[command(flatten)]
        spec: ConnectorSpec,
    },
    /// Update a connector in the catalog (admin).
    Update {
        connector_id: String,
        #[arg(long)]
        name: Option<String>,
        #[command(flatten)]
        spec: ConnectorSpec,
        /// Switch the connector on.
        #[arg(long, conflicts_with = "disable")]
        enable: bool,
        /// Switch the connector off (pauses it without removing bindings).
        #[arg(long)]
        disable: bool,
    },
    /// Remove a connector from the catalog (admin). Bindings that name it stop resolving.
    Remove { connector_id: String },
    /// Bind a connector to an owner so it can use it (admin).
    Bind {
        connector_id: String,
        /// Who gets access. `role` is usually right: the connector's domain is
        /// found or created under the role.
        #[arg(long, value_enum)]
        owner_type: OwnerType,
        /// role: role nest id. role-domain: domain nest id. workspace: workspace id.
        /// user: the person's user id. agent: the agent's user id.
        #[arg(long)]
        owner: String,
    },
    /// Connections: which connector is bound to which owner, and who connected it.
    Connections {
        #[command(subcommand)]
        cmd: ConnectionsCmd,
    },
}

#[derive(Subcommand)]
pub enum ConnectionsCmd {
    /// List the workspace's connections. Never shows a secret.
    List {
        /// Include removed (disabled) connections.
        #[arg(long)]
        include_disabled: bool,
    },
    /// Get a link a person opens to connect an account to a connection.
    Link { connection_id: String },
    /// Remove a connection and revoke every credential on it (admin).
    Remove { connection_id: String },
    /// Revoke your own credential on a connection. The connection stays.
    Revoke { connection_id: String },
}

/// Transport fields shared by `register` (without a template) and `update`.
#[derive(clap::Args)]
pub struct ConnectorSpec {
    /// Transport: mcp and api need a url in --config, cli needs a command.
    #[arg(long = "type", value_parser = ["mcp", "cli", "api"])]
    pub type_: Option<String>,
    /// Transport config as JSON, e.g. '{"url":"https://…"}'. No secrets.
    #[arg(long, value_name = "JSON")]
    pub config: Option<String>,
    /// Capability descriptor as JSON, e.g. '{"discover":true}'.
    #[arg(long, value_name = "JSON")]
    pub capabilities: Option<String>,
    /// Which owners may bind, as JSON, e.g. '{"domainGated":true,"userAgent":false}'.
    #[arg(long, value_name = "JSON")]
    pub exposure: Option<String>,
    /// How a person connects: a one-time secret, or OAuth 2.
    #[arg(long, value_parser = ["secret", "oauth2"])]
    pub auth_strategy: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum OwnerType {
    Role,
    RoleDomain,
    Workspace,
    User,
    Agent,
}

impl OwnerType {
    pub fn as_str(self) -> &'static str {
        match self {
            OwnerType::Role => "role",
            OwnerType::RoleDomain => "role-domain",
            OwnerType::Workspace => "workspace",
            OwnerType::User => "user",
            OwnerType::Agent => "agent",
        }
    }
}

/// Fold a spec (plus name/enabled) into a request body. Optional fields are omitted.
pub fn spec_body(
    name: Option<String>,
    spec: ConnectorSpec,
    enabled: Option<bool>,
) -> crate::error::Result<Map<String, Value>> {
    let mut body = Map::new();
    if let Some(t) = spec.type_ {
        body.insert("type".into(), t.into());
    }
    if let Some(n) = name {
        body.insert("name".into(), n.into());
    }
    for (key, flag, raw) in [
        ("config", "--config", spec.config),
        ("capabilities", "--capabilities", spec.capabilities),
        ("exposure", "--exposure", spec.exposure),
    ] {
        if let Some(raw) = raw {
            body.insert(key.into(), parse_json_object(flag, &raw)?);
        }
    }
    if let Some(a) = spec.auth_strategy {
        body.insert("authStrategy".into(), a.into());
    }
    if let Some(e) = enabled {
        body.insert("enabled".into(), e.into());
    }
    Ok(body)
}

/// Assemble the register body: a template (with an optional name), or a
/// hand-described transport, which needs both a type and a name.
pub fn register_body(
    template: Option<String>,
    name: Option<String>,
    spec: ConnectorSpec,
) -> crate::error::Result<Value> {
    if let Some(t) = template {
        let mut body = Map::new();
        body.insert("templateId".into(), t.into());
        if let Some(n) = name {
            body.insert("name".into(), n.into());
        }
        return Ok(Value::Object(body));
    }
    if spec.type_.is_none() || name.is_none() {
        return Err(NestrError::Validation(
            "without --template, both --type and --name are required. Check `nestr connectors templates` first: a known vendor almost always has one.".into(),
        ));
    }
    Ok(Value::Object(spec_body(name, spec, None)?))
}

pub fn bind_body(connector_id: &str, owner_type: OwnerType, owner: &str) -> Value {
    serde_json::json!({
        "connectorId": connector_id,
        "owner": { "type": owner_type.as_str(), "id": owner },
    })
}

pub async fn fetch_list(client: &NestrClient, ws: &str) -> crate::error::Result<Value> {
    let raw: Value = client
        .get(&format!("/workspaces/{ws}/connectors"), &[])
        .await?;
    Ok(unwrap_data(raw).0)
}

/// Returns the templates and the hint labels the server sends beside them.
pub async fn fetch_templates(
    client: &NestrClient,
    ws: &str,
) -> crate::error::Result<(Value, Vec<String>)> {
    let raw: Value = client
        .get(&format!("/workspaces/{ws}/connector-templates"), &[])
        .await?;
    let hints = render::hint_labels(&raw);
    Ok((unwrap_data(raw).0, hints))
}

/// Returns the registered connector and any hint (e.g. "a template exists for this url").
pub async fn register_connector(
    client: &NestrClient,
    ws: &str,
    body: &Value,
) -> crate::error::Result<(Value, Vec<String>)> {
    let raw: Value = client
        .post(&format!("/workspaces/{ws}/connectors"), body)
        .await?;
    let hints = render::hint_labels(&raw);
    Ok((unwrap_data(raw).0, hints))
}

pub async fn update_connector(
    client: &NestrClient,
    ws: &str,
    id: &str,
    body: &Value,
) -> crate::error::Result<Value> {
    let raw: Value = client
        .patch(&format!("/workspaces/{ws}/connectors/{id}"), body)
        .await?;
    Ok(unwrap_data(raw).0)
}

pub async fn remove_connector(
    client: &NestrClient,
    ws: &str,
    id: &str,
) -> crate::error::Result<Value> {
    let raw: Value = client
        .delete(&format!("/workspaces/{ws}/connectors/{id}"))
        .await?;
    Ok(unwrap_data(raw).0)
}

pub async fn bind_connector(
    client: &NestrClient,
    ws: &str,
    body: &Value,
) -> crate::error::Result<Value> {
    let raw: Value = client
        .post(&format!("/workspaces/{ws}/connections"), body)
        .await?;
    Ok(unwrap_data(raw).0)
}

pub async fn fetch_connections(
    client: &NestrClient,
    ws: &str,
    include_disabled: bool,
) -> crate::error::Result<Value> {
    let params: &[(&str, &str)] = if include_disabled {
        &[("includeDisabled", "true")]
    } else {
        &[]
    };
    let raw: Value = client
        .get(&format!("/workspaces/{ws}/connections/list"), params)
        .await?;
    Ok(unwrap_data(raw).0)
}

/// Returns `{_id, revokedCount}`.
pub async fn remove_connection(
    client: &NestrClient,
    ws: &str,
    id: &str,
) -> crate::error::Result<Value> {
    let raw: Value = client
        .delete(&format!("/workspaces/{ws}/connections/{id}"))
        .await?;
    Ok(unwrap_data(raw).0)
}

/// Returns `{connectionId, url}`.
pub async fn connect_link(client: &NestrClient, ws: &str, id: &str) -> crate::error::Result<Value> {
    let raw: Value = client
        .post(
            &format!("/workspaces/{ws}/connections/{id}/connect-link"),
            &Value::Object(Map::new()),
        )
        .await?;
    Ok(unwrap_data(raw).0)
}

/// Returns `{_id}`, or `{revoked, vaultCleanupFailed, message}` when the
/// credential was revoked but the vault copy could not be deleted.
pub async fn revoke_credential(
    client: &NestrClient,
    ws: &str,
    id: &str,
) -> crate::error::Result<Value> {
    let raw: Value = client
        .delete(&format!("/workspaces/{ws}/connections/{id}/authorizations"))
        .await?;
    Ok(unwrap_data(raw).0)
}

fn print_text_or_json(data: &Value, output: crate::config::OutputFormat, text: &str) -> Result<()> {
    match output {
        crate::config::OutputFormat::Json => render::print_json(data)?,
        crate::config::OutputFormat::Text => println!("{text}"),
    }
    Ok(())
}

pub async fn run(cmd: ConnectorsCmd, g: &GlobalArgs) -> Result<()> {
    let (cfg, client) = resolve_client(g).await?;
    let ws = cfg.require_workspace()?.to_string();
    match cmd {
        ConnectorsCmd::List => {
            let data = fetch_list(&client, &ws).await?;
            render::output_connectors(&data, cfg.output)?;
        }
        ConnectorsCmd::Templates => {
            let (data, hints) = fetch_templates(&client, &ws).await?;
            render::output_connector_templates(&data, cfg.output)?;
            render::print_hints(&hints);
        }
        ConnectorsCmd::Register {
            template,
            name,
            spec,
        } => {
            safety::enforce_read_only(g.read_only, "connectors register")?;
            let body = register_body(template, name, spec)?;
            let (data, hints) = register_connector(&client, &ws, &body).await?;
            render::connector_detail(&data, cfg.output)?;
            render::print_hints(&hints);
        }
        ConnectorsCmd::Update {
            connector_id,
            name,
            spec,
            enable,
            disable,
        } => {
            safety::enforce_read_only(g.read_only, "connectors update")?;
            let enabled = match (enable, disable) {
                (true, _) => Some(true),
                (_, true) => Some(false),
                _ => None,
            };
            let body = spec_body(name, spec, enabled)?;
            if body.is_empty() {
                anyhow::bail!("Nothing to update: pass at least one field to change.");
            }
            let data = update_connector(&client, &ws, &connector_id, &Value::Object(body)).await?;
            render::connector_detail(&data, cfg.output)?;
        }
        ConnectorsCmd::Remove { connector_id } => {
            safety::enforce_read_only(g.read_only, "connectors remove")?;
            safety::confirm_destructive(
                &format!(
                    "Remove connector '{connector_id}'? Bindings that name it stop resolving."
                ),
                g.yes,
            )?;
            let data = remove_connector(&client, &ws, &connector_id).await?;
            print_text_or_json(
                &data,
                cfg.output,
                &format!("Connector removed ({connector_id})."),
            )?;
        }
        ConnectorsCmd::Bind {
            connector_id,
            owner_type,
            owner,
        } => {
            safety::enforce_read_only(g.read_only, "connectors bind")?;
            let body = bind_body(&connector_id, owner_type, &owner);
            let data = bind_connector(&client, &ws, &body).await?;
            render::connection_bound(&data, cfg.output)?;
        }
        ConnectorsCmd::Connections { cmd } => {
            match cmd {
                ConnectionsCmd::List { include_disabled } => {
                    let data = fetch_connections(&client, &ws, include_disabled).await?;
                    render::output_connections(&data, cfg.output)?;
                }
                // A POST, but it changes nothing: the server only builds a URL, and
                // whoever opens it is re-checked. So it is a read, and --read-only
                // doesn't block it.
                ConnectionsCmd::Link { connection_id } => {
                    let data = connect_link(&client, &ws, &connection_id).await?;
                    render::connect_link(&data, cfg.output)?;
                }
                ConnectionsCmd::Remove { connection_id } => {
                    safety::enforce_read_only(g.read_only, "connectors connections remove")?;
                    safety::confirm_destructive(
                        &format!(
                        "Remove connection '{connection_id}' and revoke every credential on it?"
                    ),
                        g.yes,
                    )?;
                    let data = remove_connection(&client, &ws, &connection_id).await?;
                    let revoked = data
                        .get("revokedCount")
                        .and_then(Value::as_u64)
                        .unwrap_or(0);
                    print_text_or_json(
                    &data,
                    cfg.output,
                    &format!("Connection removed ({connection_id}); {revoked} credential(s) revoked."),
                )?;
                }
                ConnectionsCmd::Revoke { connection_id } => {
                    safety::enforce_read_only(g.read_only, "connectors connections revoke")?;
                    safety::confirm_destructive(
                        &format!("Revoke your credential on connection '{connection_id}'?"),
                        g.yes,
                    )?;
                    let data = revoke_credential(&client, &ws, &connection_id).await?;
                    if data.get("vaultCleanupFailed").and_then(Value::as_bool) == Some(true) {
                        let msg = data.get("message").and_then(Value::as_str).unwrap_or("");
                        eprintln!(
                        "warning: the credential is revoked, but its stored copy could not be deleted: {}",
                        render::clean_text(msg)
                    );
                    }
                    print_text_or_json(
                    &data,
                    cfg.output,
                    "Credential revoked. The connection stays; connect again to restore access.",
                )?;
                }
            }
        }
    }
    Ok(())
}
