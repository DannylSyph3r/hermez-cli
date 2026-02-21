use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(
    name = "hermez",
    about = "Expose your localhost to the internet",
    version
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Authenticate with Hermez using an API key
    Login,

    /// Clear stored credentials
    Logout,

    /// Start an HTTP tunnel to a local port
    Http {
        /// Local port to tunnel (e.g. 3000)
        port: u16,

        /// Request a specific subdomain
        #[arg(short = 's', long)]
        subdomain: Option<String>,

        /// Local hostname to forward requests to
        #[arg(short = 'H', long, default_value = "localhost")]
        host: String,

        /// Disable automatic reconnection on disconnect
        #[arg(long)]
        no_reconnect: bool,

        /// Timeout in seconds for forwarded requests
        #[arg(long, default_value_t = 60)]
        request_timeout: u64,
    },

    /// Show current authentication status
    Status,

    /// Print version information
    Version,
}
