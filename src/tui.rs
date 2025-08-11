use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame, Terminal,
};
use std::io;
use std::time::{Duration, Instant};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::net::UnixStream;
use tracing::{error, info};

pub struct App {
    workers: Vec<WorkerDisplay>,
    selected: usize,
    last_update: Instant,
}

#[derive(Debug, Clone)]
pub struct WorkerDisplay {
    pub tool: String,
    pub directory: String,
    pub status: String,
    pub progress: Option<f64>,
    pub message: Option<String>,
    pub bytes: Option<u64>,
}

impl App {
    pub fn new() -> Self {
        Self {
            workers: Vec::new(),
            selected: 0,
            last_update: Instant::now(),
        }
    }

    pub fn update_worker(
        &mut self,
        tool: &str,
        directory: &str,
        status: &str,
        message: Option<String>,
        bytes: Option<u64>,
    ) {
        let worker_id = format!("{}:{}", tool, directory);

        if let Some(worker) = self
            .workers
            .iter_mut()
            .find(|w| format!("{}:{}", w.tool, w.directory) == worker_id)
        {
            worker.status = status.to_string();
            worker.message = message;
            worker.bytes = bytes;
        } else {
            self.workers.push(WorkerDisplay {
                tool: tool.to_string(),
                directory: directory.to_string(),
                status: status.to_string(),
                progress: None,
                message,
                bytes,
            });
        }

        self.last_update = Instant::now();
    }

    pub fn next(&mut self) {
        if !self.workers.is_empty() {
            self.selected = (self.selected + 1) % self.workers.len();
        }
    }

    pub fn previous(&mut self) {
        if !self.workers.is_empty() {
            self.selected = if self.selected > 0 {
                self.selected - 1
            } else {
                self.workers.len() - 1
            };
        }
    }
}

pub async fn run_dashboard() -> Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app state
    let mut app = App::new();

    // Try to connect to daemon socket
    let socket_connection = connect_to_daemon().await;

    let result = run_app(&mut terminal, &mut app, socket_connection).await;

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    result
}

async fn connect_to_daemon() -> Option<UnixStream> {
    match UnixStream::connect(crate::socket::SOCKET_PATH).await {
        Ok(stream) => {
            info!("Connected to freight daemon");
            Some(stream)
        }
        Err(e) => {
            error!("Failed to connect to daemon: {}", e);
            None
        }
    }
}

async fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
    socket_connection: Option<UnixStream>,
) -> Result<()> {
    let mut last_tick = Instant::now();
    let tick_rate = Duration::from_millis(250);

    // If we have a socket connection, spawn a task to read messages
    if let Some(stream) = socket_connection {
        let mut reader = BufReader::new(stream);
        tokio::spawn(async move {
            let mut line = String::new();
            loop {
                line.clear();
                match reader.read_line(&mut line).await {
                    Ok(0) => break, // Connection closed
                    Ok(_) => {
                        // Parse and handle daemon messages
                        // This would update the app state
                    }
                    Err(e) => {
                        error!("Error reading from daemon: {}", e);
                        break;
                    }
                }
            }
        });
    }

    loop {
        terminal.draw(|f| ui(f, app))?;

        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_secs(0));

        if crossterm::event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') => return Ok(()),
                    KeyCode::Down => app.next(),
                    KeyCode::Up => app.previous(),
                    KeyCode::Char('r') => {
                        // Refresh - could trigger rescan
                    }
                    _ => {}
                }
            }
        }

        if last_tick.elapsed() >= tick_rate {
            // Update app state periodically
            last_tick = Instant::now();
        }
    }
}

fn ui(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(3),
        ])
        .split(f.size());

    // Header
    let header = Paragraph::new("Freight NFS Migration Suite")
        .style(Style::default().fg(Color::Cyan))
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(header, chunks[0]);

    // Worker list
    let workers: Vec<ListItem> = app
        .workers
        .iter()
        .enumerate()
        .map(|(i, worker)| {
            let status_color = match worker.status.as_str() {
                "running" => Color::Yellow,
                "completed" => Color::Green,
                "failed" => Color::Red,
                _ => Color::Gray,
            };

            let bytes_str = worker
                .bytes
                .map(|b| format!(" ({})", format_bytes(b)))
                .unwrap_or_default();

            let message_str = worker
                .message
                .as_ref()
                .map(|m| format!(" - {}", m))
                .unwrap_or_default();

            let content = Line::from(vec![
                Span::styled(
                    format!("{:8}", worker.tool),
                    Style::default().fg(Color::Blue),
                ),
                Span::raw(" "),
                Span::styled(
                    format!("{:20}", truncate(&worker.directory, 20)),
                    Style::default().fg(Color::White),
                ),
                Span::raw(" "),
                Span::styled(
                    format!("{:10}", worker.status),
                    Style::default().fg(status_color),
                ),
                Span::styled(bytes_str, Style::default().fg(Color::Gray)),
                Span::styled(message_str, Style::default().fg(Color::Gray)),
            ]);

            let mut item = ListItem::new(content);
            if i == app.selected {
                item = item.style(Style::default().add_modifier(Modifier::REVERSED));
            }
            item
        })
        .collect();

    let workers_list = List::new(workers)
        .block(Block::default().borders(Borders::ALL).title("Workers"))
        .highlight_style(Style::default().add_modifier(Modifier::BOLD));

    f.render_widget(workers_list, chunks[1]);

    // Footer with controls
    let footer = Paragraph::new("↑/↓: Navigate | r: Refresh | q: Quit")
        .style(Style::default().fg(Color::Gray))
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(footer, chunks[2]);
}

fn format_bytes(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut unit_index = 0;

    while size >= 1024.0 && unit_index < UNITS.len() - 1 {
        size /= 1024.0;
        unit_index += 1;
    }

    if unit_index == 0 {
        format!("{} {}", bytes, UNITS[unit_index])
    } else {
        format!("{:.1} {}", size, UNITS[unit_index])
    }
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    }
}
