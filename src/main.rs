mod auth;
mod cli;
mod display;
mod error;
mod protocol;
mod tunnel;

use std::sync::Arc;

use anyhow::Result;
use clap::Parser;
use cli::commands::{Cli, Commands};

use crate::auth::config::load_config;
use crate::display::status::StatusDisplay;
use crate::tunnel::connection::{ConnectionConfig, TunnelConnection, TunnelError};
use crate::tunnel::forwarder::HttpForwarder;
use crate::tunnel::reconnect::ReconnectStrategy;

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
            let config = load_config()?
                .ok_or_else(|| anyhow::anyhow!("Not authenticated. Run 'hermez login' first."))?;

            let display = StatusDisplay::new();
            let forwarder = Arc::new(HttpForwarder::new(host.clone(), port, request_timeout));

            let conn_config = ConnectionConfig {
                token: config.api_key,
                tunnel_url: config.server.tunnel_url,
                local_host: host,
                local_port: port,
                subdomain,
                request_timeout,
            };

            let mut attempt: u32 = 0;

            loop {
                display.show_connecting(attempt);

                match TunnelConnection::connect(&conn_config).await {
                    Ok(connection) => {
                        attempt = 0;
                        display.show_connected(connection.tunnel_info());

                        match connection.run(Arc::clone(&forwarder)).await {
                            Ok(()) => {
                                display.show_disconnected("Tunnel closed");
                                break;
                            }
                            Err(TunnelError::TunnelClosed { reason, .. }) => {
                                display.show_disconnected(&reason);
                                if no_reconnect {
                                    break;
                                }
                            }
                            Err(TunnelError::HeartbeatTimeout) => {
                                display.show_disconnected("Connection lost (heartbeat timeout)");
                                if no_reconnect {
                                    break;
                                }
                            }
                            Err(e) => {
                                display.show_error(&e.to_string());
                                if no_reconnect {
                                    return Err(e.into());
                                }
                            }
                        }
                    }
                    Err(TunnelError::ConnectionFailed(401)) => {
                        eprintln!("Authentication failed. Run 'hermez login' to re-authenticate.");
                        return Err(anyhow::anyhow!("Authentication failed"));
                    }
                    Err(TunnelError::ConnectionFailed(403)) => {
                        eprintln!("Access denied. The subdomain may be reserved by another user.");
                        return Err(anyhow::anyhow!("Access denied"));
                    }
                    Err(TunnelError::ConnectionFailed(400)) => {
                        eprintln!("Invalid request. Check your subdomain name.");
                        return Err(anyhow::anyhow!("Invalid request"));
                    }
                    Err(e) => {
                        display.show_connection_failed(&e.to_string());
                        if no_reconnect {
                            return Err(e.into());
                        }
                    }
                }

                let delay = ReconnectStrategy::delay_for_attempt(attempt);
                display.show_reconnecting(delay);
                tokio::time::sleep(delay).await;
                attempt += 1;
            }
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
