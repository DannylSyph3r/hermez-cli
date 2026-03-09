use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(
    name = "hermez",
    about = "Expose your localhost to the internet",
    version,
    override_usage = "hermez [command] [flags]",
    after_help = "Common flags for 'hermez http':\n  -s, --subdomain <NAME>   Request a specific subdomain (e.g. myapp or myapp.hermez.one)\n  -H, --host <HOST>        Local hostname to forward to [default: localhost]\n      --no-reconnect       Disable automatic reconnection on disconnect\n\nExamples:\n  hermez http 3000\n  hermez http 3000 --subdomain myapp\n  hermez http 3000 --subdomain myapp.hermez.one"
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
    Whoami,

    /// Update the hermez CLI to the latest version
    Update,

    /// Print version information
    Version,
}
