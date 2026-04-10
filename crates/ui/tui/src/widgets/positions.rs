use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;
use ratatui::widgets::{Block, BorderType, Borders, Cell, Row, Table};

use trader_core::types::Wei;

use crate::app::AppState;

/// Render the borrower positions panel.
///
/// Health-factor colour coding:
/// - HF < 1.00 → Red   (liquidatable)
/// - HF < 1.05 → Yellow (watch closely)
/// - HF ≥ 1.05 → White
pub fn render_positions(f: &mut Frame, area: Rect, state: &AppState) {
    let header_style = Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD | Modifier::UNDERLINED);

    let header = Row::new(vec![
        Cell::from(Span::styled("Address", header_style)),
        Cell::from(Span::styled("HF", header_style)),
        Cell::from(Span::styled("Protocol", header_style)),
    ]);

    let rows: Vec<Row> = state
        .positions
        .iter()
        .map(|pos| {
            let hf_decimal = health_factor_display(pos.health_factor);
            let color = hf_color(pos.health_factor);

            // Shorten address: 0x1234…abcd
            let short_addr = shorten_address(&pos.address);

            let protocol_str = pos.protocol.to_string();

            Row::new(vec![
                Cell::from(Span::styled(short_addr, Style::default().fg(Color::White))),
                Cell::from(Span::styled(hf_decimal, Style::default().fg(color).add_modifier(Modifier::BOLD))),
                Cell::from(Span::styled(protocol_str, Style::default().fg(Color::Gray))),
            ])
        })
        .collect();

    let widths = [
        ratatui::layout::Constraint::Min(10),
        ratatui::layout::Constraint::Length(6),
        ratatui::layout::Constraint::Min(8),
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .block(
            Block::default()
                .title(Span::styled(
                    " POSITIONS ",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ))
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded),
        )
        .row_highlight_style(Style::default().add_modifier(Modifier::REVERSED))
        .column_spacing(1);

    f.render_widget(table, area);
}

/// Convert `Wei` health factor (1e18 == 1.0) to a display string with 4 dp.
fn health_factor_display(hf: Wei) -> String {
    // hf.0 is in units of 1e18
    // We want X.XXXX
    let integer_part = hf.0 / 1_000_000_000_000_000_000u128;
    let frac_part = hf.0 % 1_000_000_000_000_000_000u128;
    // 4 decimal places: divide by 1e14
    let frac_4dp = frac_part / 100_000_000_000_000u128;
    format!("{}.{:04}", integer_part, frac_4dp)
}

fn hf_color(hf: Wei) -> Color {
    // 1.0  == 1_000_000_000_000_000_000
    // 1.05 == 1_050_000_000_000_000_000
    const ONE: u128 = 1_000_000_000_000_000_000;
    const ONE_05: u128 = 1_050_000_000_000_000_000;

    if hf.0 < ONE {
        Color::Red
    } else if hf.0 < ONE_05 {
        Color::Yellow
    } else {
        Color::White
    }
}

fn shorten_address(addr: &str) -> String {
    if addr.len() <= 12 {
        return addr.to_string();
    }
    // e.g. "0x1234…abcd"
    let prefix = &addr[..6];
    let suffix = &addr[addr.len() - 4..];
    format!("{}…{}", prefix, suffix)
}
