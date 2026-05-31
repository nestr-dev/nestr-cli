use clap::{Parser, Subcommand};

use nestr_cli::commands::{auth, comments, me, nests, profiles, search};
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
  \x1b[1mcomments\x1b[0m   List, add, edit, delete comments"
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
        Commands::Version => {
            println!("nestr {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
    }
}
