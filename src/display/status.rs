use crate::tunnel::connection::TunnelInfo;
use colored::Colorize;
use std::time::Duration;

pub struct StatusDisplay;

impl StatusDisplay {
    pub fn new() -> Self {
        Self
    }

    pub fn show_connecting(&self, attempt: u32) {
        if attempt == 0 {
            println!("Connecting to hermez...");
        } else {
            println!("{} (attempt {})...", "Reconnecting".yellow(), attempt);
        }
    }

    pub fn show_connected(&self, info: &TunnelInfo) {
        println!();
        println!("{}", "hermez".bold().magenta());
        println!();
        println!("{} {}", "Tunnel:".bold(), info.public_url.green().bold());
        println!(
            "{} {}:{}",
            "Forward:".bold(),
            "localhost".dimmed(),
            info.local_port
        );
        println!();
        println!("{}", "Connections".bold().dimmed());
        println!();
    }

    pub fn show_disconnected(&self, reason: &str) {
        println!();
        println!("{} {}", "Disconnected:".red().bold(), reason);
    }

    pub fn show_reconnecting(&self, delay: Duration) {
        println!(
            "{} in {:.1}s...",
            "Reconnecting".yellow(),
            delay.as_secs_f32()
        );
    }

    pub fn show_connection_failed(&self, msg: &str) {
        eprintln!("{} {}", "Connection failed:".red().bold(), msg);
    }

    pub fn show_error(&self, msg: &str) {
        eprintln!("{} {}", "Error:".red().bold(), msg);
    }
}
