use owo_colors::OwoColorize;
use std::time::Duration;

pub struct Display;

impl Display {
    pub fn banner() {
        println!();
        println!("{}", "╔═══════════════════════════════════════════════════════════════╗".cyan());
        println!("{}  AnonMiner v0.1.2 - RandomX CPU Miner  {}", "║".cyan(), "║".cyan());
        println!("{}  High-Performance Mining in rust  {}", "║".cyan(), "║".cyan());
        println!("{}", "╚═══════════════════════════════════════════════════════════════╝".cyan());
        println!();
    }

    pub fn startup_info(threads: usize, mode: &str) {
        println!("{} {}", "▶".green(), "Starting Mini-Mine".bold());
        println!("  {} Threads: {}", "├".black(), threads.to_string().yellow());
        println!("  {} Mode: {}", "├".black(), mode.yellow());
        println!("  {} Status: {}", "└".black(), "Initializing...".blue());
        println!();
    }

    pub fn hash_rate_report(hash_rate: f64, elapsed: Duration) {
        let formatted_rate = Self::format_hash_rate(hash_rate);
        
        println!("{}", "┌─ Mining Stats ────────────────────────────────────────────────┐".blue());
        println!("{} {}", "│".blue(), "Current Performance".bold().underline());
        println!("{} Hash Rate: {}", "│".blue(), formatted_rate.green().bold());
        println!("{} Runtime: {}", "│".blue(), Self::format_duration(elapsed).cyan());
        println!("{}", "└───────────────────────────────────────────────────────────────┘".blue());
        println!();
    }

    pub fn share_found(job_id: &str, share_count: u64) {
        println!("{} {}", "✓".green(), format!("Job ID {} submitted. Valid share number {}!", job_id, share_count).green().bold());
    }

    pub fn job_received(job_id: &str) {
        let job_int = u64::from_str_radix(job_id, 16).unwrap_or(0);
        println!("{} {}", "↻".blue(), format!("New job received: {} (0x{})...", job_int, job_id).blue());
    }

    pub fn connection_info(pool: &str, wallet: &str) {
        let short_wallet = if wallet.len() > 12 { &wallet[..12] } else { wallet };
        println!("{} {}", "🔗".cyan(), "Connection Details".bold());
        println!("  {} Pool: {}", "├".black(), pool.yellow());
        println!("  {} Wallet: {}...", "└".black(), short_wallet.yellow());
        println!();
    }

    fn format_hash_rate(rate: f64) -> String {
        if rate >= 1_000_000_000.0 {
            format!("{:.2} GH/s", rate / 1_000_000_000.0)
        } else if rate >= 1_000_000.0 {
            format!("{:.2} MH/s", rate / 1_000_000.0)
        } else if rate >= 1_000.0 {
            format!("{:.2} KH/s", rate / 1_000.0)
        } else {
            format!("{:.2} H/s", rate)
        }
    }

    pub fn format_duration(duration: Duration) -> String {
        let secs = duration.as_secs();
        let hours = secs / 3600;
        let minutes = (secs % 3600) / 60;
        let seconds = secs % 60;

        if hours > 0 {
            format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
        } else {
            format!("{:02}:{:02}", minutes, seconds)
        }
    }

    pub fn format_hash_rate_report(hash_rate: f64, elapsed: Duration) -> String {
        let formatted_rate = Self::format_hash_rate(hash_rate);
        let formatted_duration = Self::format_duration(elapsed);
        format!(
            "┌─ Mining Stats ────────────────────────────────────────────────┐\n│ {}\n│ Hash Rate: {}\n│ Runtime: {}\n└───────────────────────────────────────────────────────────────┘",
            "Current Performance".bold().underline(),
            formatted_rate.green().bold(),
            formatted_duration.cyan()
        )
    }
}
