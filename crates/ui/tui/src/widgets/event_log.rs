use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, List, ListItem};
use ratatui::Frame;

use crate::app::{AppState, LogLevel};

/// Render the scrollable event log panel.
///
/// Colour coding:
/// - Error   → Red
/// - Warn    → Yellow
/// - Success → Green
/// - Info    → White
pub fn render_event_log(f: &mut Frame, area: Rect, state: &AppState) {
    let items: Vec<ListItem> = state
        .log
        .iter()
        .rev() // newest first
        .map(|entry| {
            let (level_tag, level_color) = match entry.level {
                LogLevel::Error => ("ERROR", Color::Red),
                LogLevel::Warn => ("WARN ", Color::Yellow),
                LogLevel::Success => ("OK   ", Color::Green),
                LogLevel::Info => ("INFO ", Color::White),
            };

            let ts = entry.timestamp.format("%H:%M:%S").to_string();

            let line = Line::from(vec![
                Span::styled(format!("[{}] ", ts), Style::default().fg(Color::DarkGray)),
                Span::styled(
                    format!("{} ", level_tag),
                    Style::default()
                        .fg(level_color)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(entry.message.clone(), Style::default().fg(level_color)),
            ]);

            ListItem::new(line)
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .title(Span::styled(
                " EVENT LOG ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ))
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded),
    );

    f.render_widget(list, area);
}
