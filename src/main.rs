mod auth;
mod cli;
mod display;
mod error;
mod protocol;
mod tunnel;

use anyhow::Result;
use clap::Parser;
use cli::commands::{Cli, Commands};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::WARN.into()),
        )
        .with_target(false)
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Login => {
            auth::login::login().await?;
        }
        Commands::Logout => {
            auth::login::logout()?;
        }
        Commands::Http {
            port,
            subdomain,
            host,
            no_reconnect,
            request_timeout,
        } => {
            // Phase 6
            let _ = (port, subdomain, host, no_reconnect, request_timeout);
            eprintln!("hermez http — not yet implemented");
        }
        Commands::Status => {
            // Phase 8
            eprintln!("hermez status — not yet implemented");
        }
        Commands::Version => {
            println!("hermez {}", env!("CARGO_PKG_VERSION"));
            println!("Protocol version: 1");
        }
    }

    Ok(())
}
