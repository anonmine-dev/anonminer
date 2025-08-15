mod display;
mod gui_data;
mod hash_rate;
mod job;
mod share;
mod stratum;
pub mod worker;
mod gui;
mod hash_logger;

use crate::{display::Display, gui_data::GuiData, hash_rate::init_hash_rate_tracker, stratum::Stratum, worker::Worker, gui::Gui};
use clap::{Parser};
use tracing::Level;
use owo_colors::OwoColorize;
use std::{
    io::{self},
    num::NonZeroUsize,
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

const KEEP_ALIVE_INTERVAL: Duration = Duration::from_secs(60);
const HASH_RATE_REPORT_INTERVAL: Duration = Duration::from_secs(30);
const INITIAL_WARMUP_DURATION: Duration = Duration::from_secs(45);
const DONATION_POOL_URL: &str = "gulf.moneroocean.stream:10032";
const DONATION_WALLET_ADDRESS: &str = "41p5Kuj5V4qbkxZ6385kFyWgmwFF3EC5FjmL5JyGoVLbi8wSJBFZPi83cAf5moRrkehu8Bk7dtm9UcsT1662U7Wt7vsysCx";
const CYCLE_DURATION: Duration = Duration::from_secs(100 * 60); // 100 minutes
const DONATION_START_OFFSET: Duration = Duration::from_secs(50 * 60); // 50 minutes

#[derive(Parser)]
struct Args {
    /// Pool address (URL:PORT)
    #[arg(short = 'o', long, default_value = "de.monero.herominers.com:1111")]
    url: String,
    /// Wallet address
    #[arg(
        short,
        long,
        default_value = "41p5Kuj5V4qbkxZ6385kFyWgmwFF3EC5FjmL5JyGoVLbi8wSJBFZPi83cAf5moRrkehu8Bk7dtm9UcsT1662U7Wt7vsysCx"
    )]
    user: String,
    /// Worker name
    #[arg(short, long, default_value = "x")]
    pass: String,
    /// Number of CPU threads
    #[arg(short, long)]
    threads: Option<NonZeroUsize>,
    /// Switch to light mode
    #[arg(long)]
    light: bool,
    /// Enable GUI mode
    #[arg(long)]
    gui: bool,
    /// Enable detailed debug output
    #[arg(long)]
    debug_all: bool,
    /// Enable hash value logging without other debug output
    #[arg(long)]
    debug_hash_log: bool,
    /// Set the log level (trace, debug, info, warn, error)
    #[arg(long, default_value_t = Level::WARN, value_name = "LEVEL")]
    log_level: Level,
    /// Developer donation level (percentage, minimum 1%)
    #[arg(long, default_value_t = 1)]
    donate_level: u8,
}

fn all_threads() -> NonZeroUsize {
    std::thread::available_parallelism().expect("Failed to determine available parallelism")
}

fn light_threads() -> NonZeroUsize {
    let all = all_threads().get();
    if all == 1 {
        NonZeroUsize::new(1).unwrap()
    } else {
        NonZeroUsize::new(all / 2).unwrap()
    }
}

fn main() -> io::Result<()> {
    let args = Args::parse();

    // Initialize tracing subscriber to write to stderr to avoid interfering with TUI on stdout
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_max_level(args.log_level)
        .init();
    
    let Args {
        url,
        user,
        pass,
        light,
        threads,
        gui,
        debug_all,
        debug_hash_log,
        log_level: _, // log_level is used by tracing_subscriber
        donate_level,
    } = args;

    let donate_level = donate_level.max(1);

    let thread_count = if light {
        threads.unwrap_or_else(light_threads)
    } else {
        threads.unwrap_or_else(all_threads)
    };

    worker::enable_huge_pages(thread_count);
    worker::apply_msr_mods();

    Display::banner();
    Display::startup_info(thread_count.get(), if light { "Light" } else { "Fast" });
    Display::connection_info(&url, &user);

    let original_url = url.clone();
    let original_user = user.clone();

    let mut stratum = Stratum::login(&url, &user, &pass)?;
    // We need to wait for the first job to initialize the worker
    let initial_job = loop {
        if let Ok(job) = stratum.try_recv_job() {
            if debug_all {
                let job_id_int = u64::from_str_radix(&job.id, 16).unwrap_or(0);
                eprintln!("DEBUG: Initial job received, id={} (0x{}), blob length: {}, seed length: {}", 
                          job_id_int, job.id, job.blob.len(), job.seed.len());
            }
            break job;
        }
        std::thread::sleep(Duration::from_millis(100)); // Wait a bit for the job
    };

    init_hash_rate_tracker(debug_all);
    if debug_all || debug_hash_log {
        crate::hash_logger::init_hash_logger();
    }
    let worker = Worker::init(initial_job, thread_count, !light, debug_all, debug_hash_log);
    
    let mut keep_alive_timer = Instant::now();
    let mut hash_rate_timer = Instant::now();
    let mut share_count = 0;
    let cycle_start_time = Instant::now();
    let mut is_donating = false;

    println!("{} {}", "🚀".green(), "Mining started!".green().bold());
    println!("{} {}", "🔥".yellow(), "Warming up, starting mining...".yellow());
    println!();

    if gui {
        // Create channels for sending logs and data to the GUI thread
        let (log_tx, log_rx) = mpsc::channel::<String>();
        let (gui_data_tx, gui_data_rx) = mpsc::channel::<GuiData>();

        // Spawn the GUI thread
        let gui_handle = thread::spawn(move || {
            let mut gui_app = Gui::new(log_rx, gui_data_rx);
            if let Err(e) = gui_app.run() {
                // This eprintln will go to the actual stderr, as it's outside the redirected scope.
                // It's useful for debugging GUI crashes.
                eprintln!("GUI thread exited with error: {}", e);
            }
        });

        // Send initial messages to GUI log
        let _ = log_tx.send(format!("{} {}", "🚀".green(), "Mining started!".green().bold()));
        let _ = log_tx.send(format!("{} {}", "🔥".yellow(), "Warming up, starting mining...".yellow()));
        let _ = log_tx.send(String::new()); // Add a blank line

        let mut last_gui_data_send = Instant::now();
        const GUI_DATA_SEND_INTERVAL: Duration = Duration::from_millis(500); // Update GUI stats 2 times per second

        loop {
            // --- Mining Logic (adapted from console mode) ---
            if let Ok(_) = stratum.try_reconnect_signal() {
                let _ = log_tx.send(format!("{} Connection lost. Attempting to reconnect...", "⚠️".red()));
                loop {
                    match stratum.reconnect() {
                        Ok(()) => {
                            let _ = log_tx.send(format!("{} Reconnected successfully! Waiting for new job...", "✅".green()));
                            // Wait for the first job after reconnection to ensure worker state is synced
                            let mut new_job_after_reconnect: Option<crate::job::Job> = None;
                            'job_wait_loop: loop {
                                if let Ok(job) = stratum.try_recv_job() {
                                    let _ = log_tx.send(format!("New job received after reconnect: {}", job.id));
                                    new_job_after_reconnect = Some(job);
                                    break 'job_wait_loop;
                                }
                                // Check for another reconnect signal while waiting for the job
                                if stratum.try_reconnect_signal().is_ok() {
                                    let _ = log_tx.send(format!("{} Another reconnect signal while waiting for job. Retrying reconnect...", "⚠️".yellow()));
                                    break 'job_wait_loop; // Break to retry the outer reconnect loop
                                }
                                thread::sleep(Duration::from_millis(100));
                            }

                            if let Some(job_to_work) = new_job_after_reconnect {
                                worker.work(job_to_work);
                                break; // Break out of the reconnection loop only if job was received
                            }
                            // If new_job_after_reconnect is None, it means we broke due to another reconnect signal.
                            // The outer loop's `match stratum.reconnect()` will run again.
                        }
                        Err(e) => {
                            let _ = log_tx.send(format!("{} Reconnection failed: {}. Retrying in 5 seconds...", "❌".red(), e));
                            std::thread::sleep(Duration::from_secs(5));
                        }
                    }
                }
            }

            if let Ok(job) = stratum.try_recv_job() {
                let _ = log_tx.send(format!("New job received: {}", job.id));
                if debug_all {
                    let job_id_int = u64::from_str_radix(&job.id, 16).unwrap_or(0);
                    let debug_msg = format!("DEBUG: Received new job: id={} (0x{}), blob_len={}, seed_len={}", 
                              job_id_int, job.id, job.blob.len(), job.seed.len());
                    let _ = log_tx.send(debug_msg);
                }
                worker.work(job);
            }
            
            if let Ok(share) = worker.try_recv_share() {
                share_count += 1;
                let _ = log_tx.send(format!("Share #{} found for job {}", share_count, share.job_id));
                if let Err(e) = stratum.submit(share) {
                     let _ = log_tx.send(format!("Failed to submit share: {}", e));
                }
            }
            
            if keep_alive_timer.elapsed() >= KEEP_ALIVE_INTERVAL {
                keep_alive_timer = Instant::now();
                if let Err(e) = stratum.keep_alive() {
                    let _ = log_tx.send(format!("Keep alive failed: {}", e));
                }
            }
            
            if hash_rate_timer.elapsed() >= HASH_RATE_REPORT_INTERVAL {
                hash_rate_timer = Instant::now();
                let elapsed = worker.get_elapsed_time();
                
                if elapsed >= INITIAL_WARMUP_DURATION {
                    let hash_rate = worker.get_hash_rate();
                    let report = Display::format_hash_rate_report(hash_rate, elapsed);
                    let _ = log_tx.send(report);
                }
            }

            let elapsed_total = cycle_start_time.elapsed();
            let current_cycle_time = elapsed_total.as_secs() % CYCLE_DURATION.as_secs();
            let donation_duration = Duration::from_secs(donate_level as u64 * 60);

            let should_be_donating = current_cycle_time >= DONATION_START_OFFSET.as_secs() &&
                                     current_cycle_time < (DONATION_START_OFFSET + donation_duration).as_secs();

            if should_be_donating && !is_donating {
                let msg = format!("{} Switching to donation pool...", "🎁".purple());
                let _ = log_tx.send(msg);
                match Stratum::login(DONATION_POOL_URL, DONATION_WALLET_ADDRESS, &pass) {
                    Ok(s) => {
                        stratum = s;
                        let _ = log_tx.send(format!("{} Connected to donation pool. Waiting for new job...", "✅".purple()));
                        // Wait for the first job from the donation pool
                        let mut donation_job: Option<crate::job::Job> = None;
                        'donation_job_wait_loop: loop {
                            if let Ok(job) = stratum.try_recv_job() {
                                let _ = log_tx.send(format!("New job received from donation pool: {}", job.id));
                                donation_job = Some(job);
                                break 'donation_job_wait_loop;
                            }
                            // Check for reconnect signal while waiting for the job
                            if stratum.try_reconnect_signal().is_ok() {
                                let _ = log_tx.send(format!("{} Reconnect signal while waiting for donation job. Aborting donation switch.", "⚠️".yellow()));
                                break 'donation_job_wait_loop;
                            }
                            thread::sleep(Duration::from_millis(100));
                        }
                        if let Some(job_to_work) = donation_job {
                            worker.work(job_to_work);
                            is_donating = true; // Only set is_donating to true if job was received
                        } // If donation_job is None, it means we broke due to reconnect signal, is_donating remains false
                    },
                    Err(e) => {
                        let _ = log_tx.send(format!("Failed to connect to donation pool: {}", e));
                    }
                }
            } else if !should_be_donating && is_donating {
                let msg = format!("{} Switching back to original pool...", "🏡".blue());
                let _ = log_tx.send(msg);
                 match Stratum::login(&original_url, &original_user, &pass) {
                    Ok(s) => {
                        stratum = s;
                        let _ = log_tx.send(format!("{} Reconnected to original pool. Waiting for new job...", "✅".blue()));
                        // Wait for the first job from the original pool
                        let mut original_job_after_donation: Option<crate::job::Job> = None;
                        'original_job_wait_loop: loop {
                            if let Ok(job) = stratum.try_recv_job() {
                                let _ = log_tx.send(format!("New job received from original pool: {}", job.id));
                                original_job_after_donation = Some(job);
                                break 'original_job_wait_loop;
                            }
                            // Check for reconnect signal while waiting for the job
                            if stratum.try_reconnect_signal().is_ok() {
                                let _ = log_tx.send(format!("{} Reconnect signal while waiting for original job. Aborting pool switch.", "⚠️".yellow()));
                                break 'original_job_wait_loop;
                            }
                            thread::sleep(Duration::from_millis(100));
                        }
                        if let Some(job_to_work) = original_job_after_donation {
                            worker.work(job_to_work);
                            is_donating = false; // Only set is_donating to false if job was received
                        } // If original_job_after_donation is None, it means we broke due to reconnect signal, is_donating remains true
                    },
                    Err(e) => {
                        let _ = log_tx.send(format!("Failed to reconnect to original pool: {}", e));
                    }
                }
            }

            // --- Send data to GUI ---
            if last_gui_data_send.elapsed() >= GUI_DATA_SEND_INTERVAL {
                last_gui_data_send = Instant::now();
                let elapsed = worker.get_elapsed_time();
                let gui_data = GuiData {
                    hash_rate: worker.get_hash_rate(),
                    total_hashes: worker.get_total_hashes(),
                    elapsed_time: elapsed,
                    shares_found: share_count as usize, // Cast u64 to usize
                    is_warming_up: elapsed < INITIAL_WARMUP_DURATION,
                };
                if gui_data_tx.send(gui_data).is_err() {
                    let _ = log_tx.send("GUI data channel closed. Mining loop will exit.".to_string());
                    break;
                }
            }

            // Check if GUI thread is still alive
            if gui_handle.is_finished() {
                let _ = log_tx.send("GUI thread has terminated. Mining loop will exit.".to_string());
                break; 
            }
            
            thread::sleep(Duration::from_millis(10)); // Small sleep to prevent busy loop
        }
        
        // Wait for the GUI thread to finish
        let _ = gui_handle.join();

    } else {
        // Run console mode
        loop {
            if let Ok(_) = stratum.try_reconnect_signal() {
                println!("{} Connection lost. Attempting to reconnect...", "⚠️".red());
                loop {
                    match stratum.reconnect() {
                        Ok(()) => {
                            println!("{} Reconnected successfully! Waiting for new job...", "✅".green());
                            // Wait for the first job after reconnection to ensure worker state is synced
                            let mut new_job_after_reconnect: Option<crate::job::Job> = None;
                            'console_job_wait_loop: loop {
                                if let Ok(job) = stratum.try_recv_job() {
                                    println!("New job received after reconnect: {}", job.id);
                                    new_job_after_reconnect = Some(job);
                                    break 'console_job_wait_loop;
                                }
                                // Check for another reconnect signal while waiting for the job
                                if stratum.try_reconnect_signal().is_ok() {
                                    println!("{} Another reconnect signal while waiting for job. Retrying reconnect...", "⚠️".yellow());
                                    break 'console_job_wait_loop; // Break to retry the outer reconnect loop
                                }
                                thread::sleep(Duration::from_millis(100));
                            }

                            if let Some(job_to_work) = new_job_after_reconnect {
                                worker.work(job_to_work);
                                break; // Break out of the reconnection loop only if job was received
                            }
                            // If new_job_after_reconnect is None, it means we broke due to another reconnect signal.
                            // The outer loop's `match stratum.reconnect()` will run again.
                        }
                        Err(e) => {
                            eprintln!("{} Reconnection failed: {}. Retrying in 5 seconds...", "❌".red(), e);
                            std::thread::sleep(Duration::from_secs(5));
                        }
                    }
                }
            }

            if let Ok(job) = stratum.try_recv_job() {
                Display::job_received(&job.id);
                if debug_all {
                    let job_id_int = u64::from_str_radix(&job.id, 16).unwrap_or(0);
                    eprintln!("DEBUG: Received new job: id={} (0x{}), blob_len={}, seed_len={}", 
                              job_id_int, job.id, job.blob.len(), job.seed.len());
                }
                worker.work(job);
            }
            
            if let Ok(share) = worker.try_recv_share() {
                share_count += 1;
                Display::share_found(&share.job_id, share_count);
                stratum.submit(share)?;
            }
            
            if keep_alive_timer.elapsed() >= KEEP_ALIVE_INTERVAL {
                keep_alive_timer = Instant::now();
                stratum.keep_alive()?;
            }
            
            if hash_rate_timer.elapsed() >= HASH_RATE_REPORT_INTERVAL {
                hash_rate_timer = Instant::now();
                let elapsed = worker.get_elapsed_time();
                
                if elapsed >= INITIAL_WARMUP_DURATION {
                    let hash_rate = worker.get_hash_rate();
                    
                    Display::hash_rate_report(hash_rate, elapsed);
                }
            }

            let elapsed_total = cycle_start_time.elapsed();
            let current_cycle_time = elapsed_total.as_secs() % CYCLE_DURATION.as_secs();
            let donation_duration = Duration::from_secs(donate_level as u64 * 60);

            let should_be_donating = current_cycle_time >= DONATION_START_OFFSET.as_secs() &&
                                     current_cycle_time < (DONATION_START_OFFSET + donation_duration).as_secs();

            if should_be_donating && !is_donating {
                println!("{} Switching to donation pool...", "🎁".purple());
                match Stratum::login(DONATION_POOL_URL, DONATION_WALLET_ADDRESS, &pass) {
                    Ok(s) => {
                        stratum = s;
                        println!("{} Connected to donation pool. Waiting for new job...", "✅".purple());
                        // Wait for the first job from the donation pool
                        let mut donation_job: Option<crate::job::Job> = None;
                        'console_donation_job_wait_loop: loop {
                            if let Ok(job) = stratum.try_recv_job() {
                                println!("New job received from donation pool: {}", job.id);
                                donation_job = Some(job);
                                break 'console_donation_job_wait_loop;
                            }
                            // Check for reconnect signal while waiting for the job
                            if stratum.try_reconnect_signal().is_ok() {
                                println!("{} Reconnect signal while waiting for donation job. Aborting donation switch.", "⚠️".yellow());
                                break 'console_donation_job_wait_loop;
                            }
                            thread::sleep(Duration::from_millis(100));
                        }
                        if let Some(job_to_work) = donation_job {
                            worker.work(job_to_work);
                            is_donating = true; // Only set is_donating to true if job was received
                        } // If donation_job is None, it means we broke due to reconnect signal, is_donating remains false
                    },
                    Err(e) => {
                        eprintln!("Failed to connect to donation pool: {}", e);
                    }
                }
            } else if !should_be_donating && is_donating {
                println!("{} Switching back to original pool...", "🏡".blue());
                match Stratum::login(&original_url, &original_user, &pass) {
                    Ok(s) => {
                        stratum = s;
                        println!("{} Reconnected to original pool. Waiting for new job...", "✅".blue());
                        // Wait for the first job from the original pool
                        let mut original_job_after_donation: Option<crate::job::Job> = None;
                        'console_original_job_wait_loop: loop {
                            if let Ok(job) = stratum.try_recv_job() {
                                println!("New job received from original pool: {}", job.id);
                                original_job_after_donation = Some(job);
                                break 'console_original_job_wait_loop;
                            }
                            // Check for reconnect signal while waiting for the job
                            if stratum.try_reconnect_signal().is_ok() {
                                println!("{} Reconnect signal while waiting for original job. Aborting pool switch.", "⚠️".yellow());
                                break 'console_original_job_wait_loop;
                            }
                            thread::sleep(Duration::from_millis(100));
                        }
                        if let Some(job_to_work) = original_job_after_donation {
                            worker.work(job_to_work);
                            is_donating = false; // Only set is_donating to false if job was received
                        } // If original_job_after_donation is None, it means we broke due to reconnect signal, is_donating remains true
                    },
                    Err(e) => {
                        eprintln!("Failed to reconnect to original pool: {}", e);
                    }
                }
            }
        }
    }
    
    if debug_all || debug_hash_log {
        crate::hash_logger::flush_hash_log();
    }
    
    Ok(())
}
