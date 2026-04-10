use std::sync::Arc;
use std::time::Duration;

use crossterm::event::{Event, EventStream, KeyCode, KeyEvent, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use futures::StreamExt;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use tokio::sync::{RwLock, mpsc};

use trader_core::types::{BorrowerPosition, Quote};
use monitor::metrics::MetricsSnapshot;

use crate::error::TuiError;
use crate::widgets::layout::build_layout;
use crate::widgets::{
    event_log::render_event_log, header::render_header, positions::render_positions,
    prices::render_prices, system::render_system,
};

/// Message types sent to the TUI for real-time updates.
#[derive(Debug, Clone)]
pub enum TuiEvent {
    PositionUpdate(BorrowerPosition),
    PriceUpdate(Quote),
    LogEvent(LogEntry),
    MetricsUpdate(MetricsSnapshot),
    Quit,
}

#[derive(Debug, Clone)]
pub struct LogEntry {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub level: LogLevel,
    pub message: String,
}

impl LogEntry {
    pub fn new(level: LogLevel, message: impl Into<String>) -> Self {
        Self {
            timestamp: chrono::Utc::now(),
            level,
            message: message.into(),
        }
    }

    pub fn info(message: impl Into<String>) -> Self {
        Self::new(LogLevel::Info, message)
    }

    pub fn warn(message: impl Into<String>) -> Self {
        Self::new(LogLevel::Warn, message)
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self::new(LogLevel::Error, message)
    }

    pub fn success(message: impl Into<String>) -> Self {
        Self::new(LogLevel::Success, message)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Info,
    Warn,
    Error,
    Success,
}

/// Shared TUI state updated by events.
pub struct AppState {
    pub positions: Vec<BorrowerPosition>,
    pub prices: Vec<Quote>,
    /// Last 100 log entries (newest at the end).
    pub log: Vec<LogEntry>,
    pub metrics: MetricsSnapshot,
    pub start_time: std::time::Instant,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            positions: Vec::new(),
            prices: Vec::new(),
            log: Vec::new(),
            metrics: MetricsSnapshot {
                liquidations_attempted: 0,
                liquidations_succeeded: 0,
                liquidations_failed: 0,
                total_profit_cents: 0,
                positions_monitored: 0,
                price_updates_received: 0,
                unlocks_tracked: 0,
            },
            start_time: std::time::Instant::now(),
        }
    }

    /// Append a log entry, keeping only the last 100.
    pub fn add_log(&mut self, entry: LogEntry) {
        self.log.push(entry);
        if self.log.len() > 100 {
            self.log.remove(0);
        }
    }

    /// Insert or replace a position identified by address + protocol.
    pub fn update_position(&mut self, pos: BorrowerPosition) {
        if let Some(existing) = self
            .positions
            .iter_mut()
            .find(|p| p.address == pos.address && p.protocol == pos.protocol)
        {
            *existing = pos;
        } else {
            self.positions.push(pos);
        }
    }

    /// Insert or replace a quote identified by symbol + exchange.
    pub fn update_price(&mut self, quote: Quote) {
        if let Some(existing) = self
            .prices
            .iter_mut()
            .find(|q| q.symbol == quote.symbol && q.exchange == quote.exchange)
        {
            *existing = quote;
        } else {
            self.prices.push(quote);
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

/// The TUI application.
pub struct App {
    state: Arc<RwLock<AppState>>,
    event_tx: mpsc::Sender<TuiEvent>,
}

impl App {
    /// Create a new App and return the receiver end of the event channel.
    pub fn new() -> (Self, mpsc::Receiver<TuiEvent>) {
        let (tx, rx) = mpsc::channel(256);
        let state = Arc::new(RwLock::new(AppState::new()));
        (
            Self {
                state,
                event_tx: tx,
            },
            rx,
        )
    }

    /// Clone the sender so callers can push events from other tasks.
    pub fn event_sender(&self) -> mpsc::Sender<TuiEvent> {
        self.event_tx.clone()
    }

    /// Run the TUI. Blocks until the user presses 'q' or Ctrl-C.
    pub async fn run(self, mut event_rx: mpsc::Receiver<TuiEvent>) -> Result<(), TuiError> {
        // --- terminal setup ---
        enable_raw_mode()?;
        let mut stdout = std::io::stdout();
        execute!(stdout, EnterAlternateScreen)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;
        terminal.clear()?;

        let state = self.state.clone();

        // --- event processing task ---
        // We drive a separate task that reads from `event_rx` and mutates `state`.
        // The main loop just draws and handles keyboard input.
        let state_for_task = state.clone();
        let (quit_tx, mut quit_rx) = tokio::sync::oneshot::channel::<()>();

        tokio::spawn(async move {
            while let Some(ev) = event_rx.recv().await {
                let mut s = state_for_task.write().await;
                match ev {
                    TuiEvent::PositionUpdate(pos) => s.update_position(pos),
                    TuiEvent::PriceUpdate(quote) => s.update_price(quote),
                    TuiEvent::LogEvent(entry) => s.add_log(entry),
                    TuiEvent::MetricsUpdate(snap) => s.metrics = snap,
                    TuiEvent::Quit => {
                        // Signal quit to the main loop via the oneshot (best-effort).
                        let _ = quit_tx.send(());
                        break;
                    }
                }
            }
        });

        // --- crossterm keyboard event stream ---
        let mut key_stream = EventStream::new();

        // --- main draw loop ---
        let tick = Duration::from_millis(100);
        let result = 'main: loop {
            // Draw frame
            {
                let s = state.read().await;
                let draw_result = terminal.draw(|f| {
                    let area = f.area();
                    let (header_area, panels, log_area) = build_layout(area);
                    render_header(f, header_area, &s);
                    render_positions(f, panels[0], &s);
                    render_prices(f, panels[1], &s);
                    render_system(f, panels[2], &s);
                    render_event_log(f, log_area, &s);
                });
                if let Err(e) = draw_result {
                    break 'main Err(TuiError::DrawError(e.to_string()));
                }
            }

            // Poll for keyboard input with a timeout so we keep redrawing.
            let timeout = tokio::time::sleep(tick);
            tokio::select! {
                // Check if the event task asked us to quit.
                _ = &mut quit_rx => break 'main Ok(()),

                // Keyboard / terminal events
                maybe_event = key_stream.next() => {
                    match maybe_event {
                        Some(Ok(Event::Key(KeyEvent { code, modifiers, .. }))) => {
                            let quit = matches!(code, KeyCode::Char('q') | KeyCode::Char('Q'))
                                || (code == KeyCode::Char('c')
                                    && modifiers.contains(KeyModifiers::CONTROL));
                            if quit {
                                break 'main Ok(());
                            }
                        }
                        Some(Err(e)) => break 'main Err(TuiError::IoError(e)),
                        _ => {}
                    }
                }

                // Tick: just redraw on the next iteration
                _ = timeout => {}
            }
        };

        // --- terminal restore ---
        disable_raw_mode()?;
        execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
        terminal.show_cursor()?;

        result
    }
}

impl Default for App {
    fn default() -> Self {
        let (app, _rx) = Self::new();
        app
    }
}
