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
use crate::display::banner::print_banner;
use crate::display::status::StatusDisplay;
use crate::tunnel::connection::{ConnectionConfig, TunnelConnection, TunnelError};
use crate::tunnel::forwarder::HttpForwarder;
use crate::tunnel::reconnect::ReconnectStrategy;

#[tokio::main]
async fn main() -> Result<()> {
    // Enable ANSI colour support on Windows CMD and older terminals
    #[cfg(windows)]
    {
        if colored::control::set_virtual_terminal(true).is_err() {
            colored::control::set_override(false);
        }
    }
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::WARN.into()),
        )
        .with_target(false)
        .init();

    if std::env::args().len() == 1 {
        print_banner();
        println!();
        Cli::parse_from(["hermez", "--help"]);
        return Ok(());
    }

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
            domain,
            host,
            no_reconnect,
            request_timeout,
        } => {
            // require_auth() checks HERMEZ_API_KEY env var first, falls back to config file.
            let token = require_auth()?;

            let display = StatusDisplay::new();
            let forwarder = Arc::new(HttpForwarder::new(host.clone(), port, request_timeout));

            let subdomain = subdomain.map(|s| {
                if let Some(prefix) = s.strip_suffix(".hermez.one") {
                    prefix.to_string()
                } else if let Some(prefix) = s.strip_suffix(".hermez.online") {
                    prefix.to_string()
                } else {
                    s
                }
            });

            let conn_config = ConnectionConfig {
                token,
                tunnel_url: auth::config::TUNNEL_URL.to_string(),
                local_host: host,
                local_port: port,
                subdomain,
                custom_domain: domain,
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
                            Err(TunnelError::TunnelClosed { reason, code }) => {
                                display.show_disconnected(&reason);
                                // dashboard_close is an intentional user action — never reconnect
                                if code == "dashboard_close" || no_reconnect {
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
                        std::process::exit(1);
                    }
                    Err(TunnelError::ConnectionFailed(401)) => {
                        eprintln!("Authentication failed. Run 'hermez login' to re-authenticate.");
                        std::process::exit(1);
                    }
                    Err(TunnelError::ConnectionFailed(403)) => {
                        eprintln!("Access denied. The subdomain may be reserved by another user.");
                        std::process::exit(1);
                    }
                    Err(TunnelError::ConnectionFailed(400)) => {
                        eprintln!("Invalid request. Check your subdomain name.");
                        std::process::exit(1);
                    }
                    Err(e) => {
                        let msg = match &e {
                            TunnelError::WebSocket(tokio_tungstenite::tungstenite::Error::Io(
                                _,
                            )) => {
                                "Cannot reach tunnel.hermez.online. Check your internet connection."
                                    .to_string()
                            }
                            _ => e.to_string(),
                        };
                        display.show_connection_failed(&msg);
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

        Commands::Whoami => {
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

        Commands::Update => {
            let is_npm = std::env::current_exe()
                .ok()
                .and_then(|p| p.to_str().map(|s| s.to_string()))
                .map(|p| p.contains("node_modules") || p.contains("@hermez-tunnels"))
                .unwrap_or(false);

            if !is_npm {
                println!("{} hermez was not installed via npm.", "!".yellow().bold());
                println!(
                    "  Please update manually. Visit {} for instructions.",
                    "https://hermez.one/docs/cli".cyan()
                );
                return Ok(());
            }

            println!("{}", "Updating hermez CLI...".dimmed());

            let status = std::process::Command::new("npm")
                .args(["install", "-g", "@hermez-tunnels/cli@latest"])
                .status();

            match status {
                Ok(s) if s.success() => {
                    println!("{} hermez CLI updated successfully.", "✓".green().bold());
                }
                Ok(_) => {
                    eprintln!("{} Update failed. Try running manually:", "✗".red().bold());
                    eprintln!("  npm install -g @hermez-tunnels/cli@latest");
                }
                Err(_) => {
                    eprintln!("{} Could not run npm. Is it installed?", "✗".red().bold());
                    eprintln!("  Try: npm install -g @hermez-tunnels/cli@latest");
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
