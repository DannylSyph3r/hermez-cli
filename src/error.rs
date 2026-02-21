use thiserror::Error;

#[derive(Debug, Error)]
pub enum HermezError {
    #[error("Not authenticated. Run 'hermez login' first.")]
    NotAuthenticated,

    #[error("Authentication failed: {0}")]
    AuthFailed(String),

    #[error("Connection failed: {0}")]
    ConnectionFailed(String),

    #[error("Subdomain unavailable: {0}")]
    SubdomainUnavailable(String),

    #[error("Local server not reachable at {host}:{port}")]
    LocalServerUnreachable { host: String, port: u16 },

    #[error("Protocol error: {0}")]
    Protocol(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}
