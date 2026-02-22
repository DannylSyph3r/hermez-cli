use colored::Colorize;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

/// Log a completed request: [HH:MM:SS]
pub fn log_request(method: &str, path: &str, status: u16, started_at: Instant) {
    let duration_ms = started_at.elapsed().as_millis();

    let status_colored = if status < 300 {
        status.to_string().green().bold()
    } else if status < 400 {
        status.to_string().yellow().bold()
    } else {
        status.to_string().red().bold()
    };

    println!(
        "{} {} {} {} {}",
        current_time_str().dimmed(),
        method.bold().white(),
        path,
        status_colored,
        format!("({}ms)", duration_ms).dimmed(),
    );
}

fn current_time_str() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let h = (secs % 86400) / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    format!("[{:02}:{:02}:{:02}]", h, m, s)
}
