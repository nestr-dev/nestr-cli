use clap::{Parser, Subcommand};

use nestr_cli::commands::{
    auth, circles, comments, export, groups, inbox, insights, labels, links, me, nests,
    notifications, plan, profiles, projects, roles, search, tensions, users, webhooks, work,
    workspaces,
};
use nestr_cli::config::OutputFormat;

/// Nestr CLI — fast, composable access to Nestr for terminals and agents.
#[derive(Parser)]
#[command(
    name = "nestr",
    version,
    about,
    long_about = None,
    help_template = "{before-help}{about-with-newline}\n{usage-heading} {usage}{after-help}\n\n\x1b[1m\x1b[4mGlobal Options:\x1b[0m\n{options}",
    after_help = "\
\x1b[1m\x1b[4mAuth & Profiles:\x1b[0m
  \x1b[1mauth\x1b[0m       Log in/out and check authentication status
  \x1b[1mprofiles\x1b[0m   Manage profiles (add, list, use, remove)
  \x1b[1mme\x1b[0m         Show the authenticated user
  \x1b[1mversion\x1b[0m    Print the CLI version

\x1b[1m\x1b[4mCore:\x1b[0m
  \x1b[1msearch\x1b[0m     Search nests across the workspace
  \x1b[1mnests\x1b[0m      Get, list, create, update, delete nests
  \x1b[1mcomments\x1b[0m   List, add, edit, delete comments
  \x1b[1minbox\x1b[0m      Capture and manage inbox items
  \x1b[1mplan\x1b[0m       Show/manage today's plan
  \x1b[1mnotifications\x1b[0m  List and clear notifications
  \x1b[1mlabels\x1b[0m     List labels; manage personal labels
  \x1b[1mprojects\x1b[0m   List workspace projects
  \x1b[1mwork\x1b[0m       Show open projects and todos

\x1b[1m\x1b[4mOrg & People:\x1b[0m
  \x1b[1mworkspaces\x1b[0m List workspaces; manage apps
  \x1b[1mcircles\x1b[0m    List/manage circles + their roles/projects/posts
  \x1b[1mroles\x1b[0m      List/manage roles
  \x1b[1musers\x1b[0m      List/manage workspace users (admin)
  \x1b[1mgroups\x1b[0m     List/manage groups (admin)

\x1b[1m\x1b[4mGovernance:\x1b[0m
  \x1b[1mtensions\x1b[0m   Governance tensions: propose, vote, enact

\x1b[1m\x1b[4mGraph & Insights:\x1b[0m
  \x1b[1mlinks\x1b[0m      List/add/remove graph links
  \x1b[1minsights\x1b[0m   Organizational health metrics (BETA)
  \x1b[1mexport\x1b[0m     Dump governance/work as JSON

\x1b[1m\x1b[4mIntegrations:\x1b[0m
  \x1b[1mwebhooks\x1b[0m   List/create/delete workspace webhooks (admin)"
)]
struct Cli {
    /// Profile to use (overrides the default).
    #[arg(
        long,
        short = 'p',
        global = true,
        env = "NESTR_PROFILE",
        help_heading = "Global Options"
    )]
    profile: Option<String>,

    /// API key (overrides the profile credential).
    #[arg(
        long,
        global = true,
        env = "NESTR_API_KEY",
        hide_env_values = true,
        help_heading = "Global Options"
    )]
    api_key: Option<String>,

    /// Host override, e.g. http://localhost:4001 (overrides the profile host).
    #[arg(
        long,
        global = true,
        env = "NESTR_HOST",
        help_heading = "Global Options"
    )]
    host: Option<String>,

    /// Output format: text or json.
    #[arg(long, short = 'o', global = true, help_heading = "Global Options")]
    output: Option<OutputFormat>,

    /// Skip confirmation prompts for destructive operations.
    #[arg(long, global = true, help_heading = "Global Options")]
    yes: bool,

    /// Block all write operations.
    #[arg(
        long,
        global = true,
        env = "NESTR_READ_ONLY",
        help_heading = "Global Options"
    )]
    read_only: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Log in/out and check authentication status.
    Auth {
        #[command(subcommand)]
        cmd: AuthCmd,
    },
    /// Manage profiles.
    Profiles {
        #[command(subcommand)]
        cmd: ProfilesCmd,
    },
    /// Show the authenticated user.
    Me,
    /// Print the CLI version.
    Version,
    /// Search nests in the workspace (or within a nest with --in).
    Search(nestr_cli::commands::search::SearchArgs),
    /// Read and manage nests (the core Nestr object).
    Nests {
        #[command(subcommand)]
        cmd: nestr_cli::commands::nests::NestsCmd,
    },
    /// Read and write comments on nests.
    Comments {
        #[command(subcommand)]
        cmd: nestr_cli::commands::comments::CommentsCmd,
    },
    /// Your personal inbox.
    Inbox {
        #[command(subcommand)]
        cmd: nestr_cli::commands::inbox::InboxCmd,
    },
    /// Your daily plan (the `now` list).
    Plan {
        #[command(subcommand)]
        cmd: nestr_cli::commands::plan::PlanCmd,
    },
    /// Your notifications.
    Notifications {
        #[command(subcommand)]
        cmd: nestr_cli::commands::notifications::NotificationsCmd,
    },
    /// Workspace and personal labels.
    Labels {
        #[command(subcommand)]
        cmd: nestr_cli::commands::labels::LabelsCmd,
    },
    /// List workspace projects.
    Projects {
        #[command(subcommand)]
        cmd: nestr_cli::commands::projects::ProjectsCmd,
    },
    /// Your open work (projects + todos).
    Work,
    /// Workspaces and their apps.
    Workspaces {
        #[command(subcommand)]
        cmd: nestr_cli::commands::workspaces::WorkspacesCmd,
    },
    /// Circles (governance) and their sub-resources.
    Circles {
        #[command(subcommand)]
        cmd: nestr_cli::commands::circles::CirclesCmd,
    },
    /// Roles (governance).
    Roles {
        #[command(subcommand)]
        cmd: nestr_cli::commands::roles::RolesCmd,
    },
    /// Workspace users (admin operations).
    Users {
        #[command(subcommand)]
        cmd: nestr_cli::commands::users::UsersCmd,
    },
    /// Workspace groups.
    Groups {
        #[command(subcommand)]
        cmd: nestr_cli::commands::groups::GroupsCmd,
    },
    /// Governance tensions (propose, vote, enact).
    Tensions {
        #[command(subcommand)]
        cmd: nestr_cli::commands::tensions::TensionsCmd,
    },
    /// Graph links between nests.
    Links {
        #[command(subcommand)]
        cmd: nestr_cli::commands::links::LinksCmd,
    },
    /// Organizational-health insights (BETA).
    Insights {
        #[command(subcommand)]
        cmd: nestr_cli::commands::insights::InsightsCmd,
    },
    /// Export governance/work as JSON.
    Export {
        #[command(subcommand)]
        cmd: nestr_cli::commands::export::ExportCmd,
    },
    /// Workspace webhooks (admin).
    Webhooks {
        #[command(subcommand)]
        cmd: nestr_cli::commands::webhooks::WebhooksCmd,
    },
}

#[derive(Subcommand)]
enum AuthCmd {
    /// Run the browser OAuth login for a profile.
    Login { profile: Option<String> },
    /// Log out a profile (server-side + local).
    Logout { profile: Option<String> },
    /// Show authentication status for a profile.
    Status { profile: Option<String> },
}

#[derive(Subcommand)]
enum ProfilesCmd {
    /// Add a new profile (interactive).
    Add { name: Option<String> },
    /// List profiles.
    List,
    /// Set the default profile.
    Use { name: String },
    /// Remove a profile.
    Remove { name: String },
}

#[tokio::main]
async fn main() {
    // reqwest is built with `rustls-no-provider`, so a process-wide crypto
    // provider must be installed before any reqwest client is constructed.
    // oauth.rs builds its own client during token exchange/refresh (before any
    // NestrClient exists), so install here at startup. Idempotent: first wins.
    let _ = rustls::crypto::ring::default_provider().install_default();
    if let Err(e) = run().await {
        eprintln!("Error: {e:#}");
        std::process::exit(1);
    }
}

async fn run() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let g = nestr_cli::commands::GlobalArgs {
        profile: cli.profile.clone(),
        api_key: cli.api_key.clone(),
        host: cli.host.clone(),
        output: cli.output,
        yes: cli.yes,
        read_only: cli.read_only,
    };

    match cli.command {
        Commands::Auth { cmd } => match cmd {
            AuthCmd::Login { profile } => auth::run_login(profile).await,
            AuthCmd::Logout { profile } => auth::run_logout(profile, cli.yes).await,
            AuthCmd::Status { profile } => auth::run_status(profile).await,
        },
        Commands::Profiles { cmd } => match cmd {
            ProfilesCmd::Add { name } => profiles::run_add(name).await,
            ProfilesCmd::List => profiles::run_list(),
            ProfilesCmd::Use { name } => profiles::run_use(name),
            ProfilesCmd::Remove { name } => profiles::run_remove(name, cli.yes),
        },
        Commands::Me => me::run(&g).await,
        Commands::Search(args) => search::run(args, &g).await,
        Commands::Nests { cmd } => nests::run(cmd, &g).await,
        Commands::Comments { cmd } => comments::run(cmd, &g).await,
        Commands::Inbox { cmd } => inbox::run(cmd, &g).await,
        Commands::Plan { cmd } => plan::run(cmd, &g).await,
        Commands::Notifications { cmd } => notifications::run(cmd, &g).await,
        Commands::Labels { cmd } => labels::run(cmd, &g).await,
        Commands::Projects { cmd } => projects::run(cmd, &g).await,
        Commands::Work => work::run(&g).await,
        Commands::Workspaces { cmd } => workspaces::run(cmd, &g).await,
        Commands::Circles { cmd } => circles::run(cmd, &g).await,
        Commands::Roles { cmd } => roles::run(cmd, &g).await,
        Commands::Users { cmd } => users::run(cmd, &g).await,
        Commands::Groups { cmd } => groups::run(cmd, &g).await,
        Commands::Tensions { cmd } => tensions::run(cmd, &g).await,
        Commands::Links { cmd } => links::run(cmd, &g).await,
        Commands::Insights { cmd } => insights::run(cmd, &g).await,
        Commands::Export { cmd } => export::run(cmd, &g).await,
        Commands::Webhooks { cmd } => webhooks::run(cmd, &g).await,
        Commands::Version => {
            println!("nestr {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
    }
}
