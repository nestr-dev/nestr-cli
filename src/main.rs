use clap::{Parser, Subcommand};

/// Nestr CLI — fast, composable access to Nestr for terminals and agents.
#[derive(Parser)]
#[command(name = "nestr", version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Print the CLI version.
    Version,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    match cli.command {
        Commands::Version => println!("nestr {}", env!("CARGO_PKG_VERSION")),
    }
}
