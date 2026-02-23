use rand::Rng;
use std::time::Duration;

pub struct ReconnectStrategy;

impl ReconnectStrategy {
    /// Backoff sequence: 1s, 2s, 4s, 8s, 16s, 30s (cap), with ±20% jitter.
    pub fn delay_for_attempt(attempt: u32) -> Duration {
        let base_secs: u64 = match attempt {
            0 => 1,
            1 => 2,
            2 => 4,
            3 => 8,
            4 => 16,
            _ => 30,
        };

        let jitter = rand::rng().random_range(0.8_f64..=1.2_f64);
        let secs = ((base_secs as f64) * jitter).round() as u64;
        Duration::from_secs(secs.max(1))
    }
}
