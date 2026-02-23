use thiserror::Error;

#[derive(Debug, Error)]
pub enum HermezError {
    #[error("Not authenticated. Run 'hermez login' first.")]
    NotAuthenticated,

    #[error("Authentication failed: {0}")]
    AuthFailed(String),

    #[error("Connection failed: {0}")]
    ConnectionFailed(String),

    #[allow(dead_code)]
    #[error("Subdomain unavailable: {0}")]
    SubdomainUnavailable(String),

    #[allow(dead_code)]
    #[error("Local server not reachable at {host}:{port}")]
    LocalServerUnreachable { host: String, port: u16 },

    #[allow(dead_code)]
    #[error("Protocol error: {0}")]
    Protocol(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}
