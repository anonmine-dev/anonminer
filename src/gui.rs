use crate::{display::Display, gui_data::GuiData};
use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::{io, sync::mpsc, time::Duration};
use tui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    text::{Span, Spans},
    widgets::{Block, Borders, Paragraph, Row, Table, Wrap},
    Frame, Terminal,
};

const MAX_LOG_LINES: usize = 100;

pub struct Gui {
    log_rx: mpsc::Receiver<String>,
    log_messages: Vec<String>,
    gui_data_rx: mpsc::Receiver<GuiData>,
    current_gui_data: GuiData,
}

impl Gui {
    pub fn new(log_rx: mpsc::Receiver<String>, gui_data_rx: mpsc::Receiver<GuiData>) -> Self {
        Self {
            log_rx,
            log_messages: Vec::new(),
            gui_data_rx,
            current_gui_data: GuiData {
                hash_rate: 0.0,
                total_hashes: 0,
                elapsed_time: Duration::from_secs(0),
                shares_found: 0,
                is_warming_up: true,
            },
        }
    }

    pub fn run(&mut self) -> io::Result<()> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        let result = self.run_app(&mut terminal);

        disable_raw_mode()?;
        execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
        terminal.show_cursor()?;

        result
    }

    fn run_app(&mut self, terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> io::Result<()> {
        let mut last_update = std::time::Instant::now();
        
        loop {
            let now = std::time::Instant::now();
            let should_update = (now - last_update).as_millis() >= 250; // Update UI ~4 times per sec

            while let Ok(msg) = self.log_rx.try_recv() {
                self.add_log_message(msg);
            }

            while let Ok(data) = self.gui_data_rx.try_recv() {
                self.current_gui_data = data;
            }

            if event::poll(Duration::from_millis(10))? {
                if let Event::Key(key) = event::read()? {
                    match key.code {
                        KeyCode::Char('q') | KeyCode::Char('c') => {
                            if key.modifiers.contains(event::KeyModifiers::CONTROL) || key.code == KeyCode::Char('q') {
                                return Ok(());
                            }
                        }
                        _ => {}
                    }
                }
                // An event occurred, so we will update the UI below if not already scheduled by the timer.
                // This makes the UI more responsive to input.
                if !should_update {
                    terminal.draw(|f| self.ui(f))?;
                    last_update = now;
                }
            }

            if should_update {
                terminal.draw(|f| self.ui(f))?;
                last_update = now;
            }

            std::thread::sleep(Duration::from_millis(10));
        }
    }

    fn ui(&self, f: &mut Frame<CrosstermBackend<io::Stdout>>) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints(
                [
                    Constraint::Length(3), // Banner
                    Constraint::Percentage(60), // Main content (Stats & Shares)
                    Constraint::Percentage(35), // Log output
                    Constraint::Length(1), // Footer
                ]
                .as_ref(),
            )
            .split(f.size());

        let banner = Paragraph::new("Mini-Mine v0.1.2 - RandomX CPU Miner")
            .style(Style::default().fg(Color::Cyan))
            .alignment(tui::layout::Alignment::Center);
        f.render_widget(banner, chunks[0]);

        let main_content_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(60), Constraint::Percentage(40)].as_ref())
            .split(chunks[1]);

        let data = &self.current_gui_data;

        if !data.is_warming_up {
            let hash_rate_str = format!("{:.2} H/s", data.hash_rate);
            let total_hashes_str = data.total_hashes.to_string();
            let elapsed_time_str = Display::format_duration(data.elapsed_time);
            let shares_found_str = data.shares_found.to_string();
            
            let stats = vec![
                Row::new(vec!["Hash Rate", &hash_rate_str]),
                Row::new(vec!["Total Hashes", &total_hashes_str]),
                Row::new(vec!["Runtime", &elapsed_time_str]),
                Row::new(vec!["Shares Found", &shares_found_str]),
            ];

            let stats_table = Table::new(stats)
                .header(Row::new(vec!["Metric", "Value"]).style(Style::default().fg(Color::Yellow)))
                .block(Block::default().title("Mining Stats").borders(Borders::ALL))
                .widths(&[
                    Constraint::Percentage(50),
                    Constraint::Percentage(50),
                ]);
            f.render_widget(stats_table, main_content_chunks[0]);
        } else {
            // Use elapsed_time from GuiData for warmup display
            let warmup_text = format!("Warming up... {:.1}s/45.0s", data.elapsed_time.as_secs_f64());
            let warmup_paragraph = Paragraph::new(warmup_text)
                .style(Style::default().fg(Color::Yellow))
                .alignment(tui::layout::Alignment::Center);
            f.render_widget(warmup_paragraph, main_content_chunks[0]);
        }

        let status_spans = vec
![Spans::from(Span::raw("Mining active..."))];
        let shares_widget = Paragraph::new(status_spans)
            .block(Block::default().title("Status").borders(Borders::ALL));
        f.render_widget(shares_widget, main_content_chunks[1]);


        let log_spans: Vec<Spans> = self.log_messages.iter().rev().take(MAX_LOG_LINES).map(|s| {
            let span = if s.starts_with("DEBUG:") || s.starts_with("ERROR:") {
                Span::styled(s, Style::default().fg(Color::Red))
            } else {
                Span::raw(s)
            };
            Spans::from(span)
        }).collect();

        let log_widget = Paragraph::new(log_spans)
            .block(Block::default().title("Terminal Output").borders(Borders::ALL))
            .wrap(Wrap { trim: true });
        f.render_widget(log_widget, chunks[2]);
        
        let footer = Paragraph::new("Press 'q' to quit")
            .style(Style::default().fg(Color::Gray))
            .alignment(tui::layout::Alignment::Center);
        f.render_widget(footer, chunks[3]);
    }


    fn add_log_message(&mut self, msg: String) {
        // Split multi-line messages and add them individually
        for line in msg.lines() {
            if !line.trim().is_empty() { // Avoid adding empty lines
                 self.log_messages.push(line.to_string());
            }
        }
        if self.log_messages.len() > MAX_LOG_LINES {
            let drain = self.log_messages.len() - MAX_LOG_LINES;
            self.log_messages.drain(0..drain);
        }
    }
}
