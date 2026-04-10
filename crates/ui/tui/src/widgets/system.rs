use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph};
use ratatui::Frame;

use crate::app::AppState;

/// Render the system metrics panel.
pub fn render_system(f: &mut Frame, area: Rect, state: &AppState) {
    let m = &state.metrics;

    // Format profit
    let profit_cents = m.total_profit_cents;
    let profit_color = if profit_cents >= 0 {
        Color::Green
    } else {
        Color::Red
    };
    let profit_str = format_profit(profit_cents);

    let label_style = Style::default().fg(Color::Gray);
    let value_style = Style::default()
        .fg(Color::White)
        .add_modifier(Modifier::BOLD);

    let lines = vec![
        Line::from(vec![
            Span::styled("liquidations:  ", label_style),
            Span::styled(
                format!(
                    "{}/{}/{}",
                    m.liquidations_attempted, m.liquidations_succeeded, m.liquidations_failed
                ),
                value_style,
            ),
        ]),
        Line::from(vec![Span::styled(
            "  attempted/ok/fail",
            Style::default().fg(Color::DarkGray),
        )]),
        Line::from(vec![
            Span::styled("profit:        ", label_style),
            Span::styled(
                profit_str,
                Style::default()
                    .fg(profit_color)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled("positions:     ", label_style),
            Span::styled(m.positions_monitored.to_string(), value_style),
        ]),
        Line::from(vec![
            Span::styled("price updates: ", label_style),
            Span::styled(m.price_updates_received.to_string(), value_style),
        ]),
        Line::from(vec![
            Span::styled("unlocks:       ", label_style),
            Span::styled(m.unlocks_tracked.to_string(), value_style),
        ]),
    ];

    let paragraph = Paragraph::new(lines).block(
        Block::default()
            .title(Span::styled(
                " SYSTEM ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ))
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded),
    );

    f.render_widget(paragraph, area);
}

fn format_profit(cents: i64) -> String {
    let sign = if cents < 0 { "-" } else { "" };
    let abs_cents = cents.unsigned_abs();
    let dollars = abs_cents / 100;
    let frac = abs_cents % 100;
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
