use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph};
use ratatui::Frame;

use crate::app::AppState;

/// Render the top header bar showing app name, net profit and uptime.
pub fn render_header(f: &mut Frame, area: Rect, state: &AppState) {
    // Calculate uptime
    let elapsed = state.start_time.elapsed();
    let total_secs = elapsed.as_secs();
    let hours = total_secs / 3600;
    let minutes = (total_secs % 3600) / 60;

    // Format profit: cents → dollars with two decimal places
    let profit_cents = state.metrics.total_profit_cents;
    let profit_str = format_profit(profit_cents);

    let profit_color = if profit_cents >= 0 {
        Color::Green
    } else {
        Color::Red
    };

    // Split into three columns
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Ratio(1, 3),
            Constraint::Ratio(1, 3),
            Constraint::Ratio(1, 3),
        ])
        .split(area);

    // Left: app title
    let title = Paragraph::new(Line::from(vec![Span::styled(
        " CRYPTO TRADER BOT ",
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    )]))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded),
    )
    .alignment(Alignment::Left);
    f.render_widget(title, columns[0]);

    // Centre: net profit
    let profit = Paragraph::new(Line::from(vec![
        Span::styled("net profit: ", Style::default().fg(Color::White)),
        Span::styled(
            profit_str,
            Style::default()
                .fg(profit_color)
                .add_modifier(Modifier::BOLD),
        ),
    ]))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded),
    )
    .alignment(Alignment::Center);
    f.render_widget(profit, columns[1]);

    // Right: uptime
    let uptime = Paragraph::new(Line::from(vec![
        Span::styled("uptime: ", Style::default().fg(Color::White)),
        Span::styled(
            format!("{}h {}m", hours, minutes),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
    ]))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded),
    )
    .alignment(Alignment::Right);
    f.render_widget(uptime, columns[2]);
}

fn format_profit(cents: i64) -> String {
    let sign = if cents < 0 { "-" } else { "" };
    let abs_cents = cents.unsigned_abs();
    let dollars = abs_cents / 100;
    let frac = abs_cents % 100;
    // Format dollars with comma separators
    let dollars_str = format_with_commas(dollars);
    format!("{}${}.{:02}", sign, dollars_str, frac)
}

fn format_with_commas(n: u64) -> String {
    let s = n.to_string();
    let mut result = String::new();
    for (i, ch) in s.chars().rev().enumerate() {
        if i != 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(ch);
    }
    result.chars().rev().collect()
}
