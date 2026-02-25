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
use colored::Colorize;

use crate::auth::config::{config_path, load_config};
use crate::auth::login::require_auth;
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
            // require_auth() checks HERMEZ_API_KEY env var first, falls back to config file.
            let token = require_auth()?;
            // Server config from file if present, otherwise defaults to hermez.one endpoints.
            let server = load_config()?.map(|c| c.server).unwrap_or_default();

            let display = StatusDisplay::new();
            let forwarder = Arc::new(HttpForwarder::new(host.clone(), port, request_timeout));

            let conn_config = ConnectionConfig {
                token,
                tunnel_url: server.tunnel_url,
                local_host: host,
                local_port: port,
                subdomain,
                request_timeout,
            };

            let mut attempt: u32 = 0;

            // Pin the ctrl_c future once and reuse it across all select! points in the loop.
            let ctrl_c = tokio::signal::ctrl_c();
            tokio::pin!(ctrl_c);

            'outer: loop {
                display.show_connecting(attempt);

                // Race connection attempt against Ctrl+C.
                let connect_result = tokio::select! {
                    result = TunnelConnection::connect(&conn_config) => result,
                    _ = &mut ctrl_c => {
                        println!();
                        break 'outer;
                    }
                };

                match connect_result {
                    Ok(connection) => {
                        attempt = 0;
                        display.show_connected(connection.tunnel_info());

                        // Race the active tunnel against Ctrl+C.
                        let run_result = tokio::select! {
                            result = connection.run(Arc::clone(&forwarder)) => result,
                            _ = &mut ctrl_c => {
                                display.show_disconnected("Interrupted");
                                break 'outer;
                            }
                        };

                        match run_result {
                            Ok(()) => {
                                display.show_disconnected("Tunnel closed");
                                break 'outer;
                            }
                            Err(TunnelError::TunnelClosed { reason, .. }) => {
                                display.show_disconnected(&reason);
                                if no_reconnect {
                                    break 'outer;
                                }
                            }
                            Err(TunnelError::HeartbeatTimeout) => {
                                display.show_disconnected("Connection lost (heartbeat timeout)");
                                if no_reconnect {
                                    break 'outer;
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

                    Err(TunnelError::FatalClose(msg)) => {
                        eprintln!("{}", msg);
                        return Err(anyhow::anyhow!(msg));
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

                // Race reconnect sleep against Ctrl+C.
                let delay = ReconnectStrategy::delay_for_attempt(attempt);
                display.show_reconnecting(delay);

                tokio::select! {
                    _ = tokio::time::sleep(delay) => {}
                    _ = &mut ctrl_c => {
                        println!();
                        break 'outer;
                    }
                }

                attempt += 1;
            }
        }

        Commands::Status => {
            let path = config_path()?;
            match load_config()? {
                Some(config) => {
                    println!(
                        "{} Logged in as {}",
                        "✓".green().bold(),
                        config.user.email.bold()
                    );
                    println!("  Config: {}", path.display().to_string().dimmed());
                }
                None => {
                    println!(
                        "  Not logged in. Run {} to authenticate.",
                        "'hermez login'".bold()
                    );
                }
            }
        }

        Commands::Version => {
            println!("hermez {}", env!("CARGO_PKG_VERSION"));
            println!("Protocol version: 1");
        }
    }

    Ok(())
}
