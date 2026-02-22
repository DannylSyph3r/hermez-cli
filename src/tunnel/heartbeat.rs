use std::time::{Duration, Instant};

/// Server sends a PING every 5s. If we go 15s without one, the connection is dead.
const DEAD_CONNECTION_TIMEOUT: Duration = Duration::from_secs(15);

/// Tracks the last PING received from the server.
pub struct PingTracker {
    last_ping_at: Instant,
}

impl PingTracker {
    pub fn new() -> Self {
        Self {
            last_ping_at: Instant::now(),
        }
    }

    /// Call each time a PING frame is received.
    pub fn record_ping(&mut self) {
        self.last_ping_at = Instant::now();
    }

    /// Returns true if no PING has arrived within the timeout window.
    pub fn is_stale(&self) -> bool {
        self.last_ping_at.elapsed() > DEAD_CONNECTION_TIMEOUT
    }
}
