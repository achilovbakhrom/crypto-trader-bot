use ratatui::Frame;
use ratatui::layout::{Constraint, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;
use ratatui::widgets::{Block, BorderType, Borders, Cell, Row, Table};

use crate::app::AppState;

/// Render the live price-feed panel (symbol | bid | ask).
pub fn render_prices(f: &mut Frame, area: Rect, state: &AppState) {
    let header_style = Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD | Modifier::UNDERLINED);

    let header = Row::new(vec![
        Cell::from(Span::styled("Symbol", header_style)),
        Cell::from(Span::styled("Bid", header_style)),
        Cell::from(Span::styled("Ask", header_style)),
    ]);

    let rows: Vec<Row> = state
        .prices
        .iter()
        .map(|q| {
            let symbol = q.symbol.0.clone();
            let bid = format!("{}", q.bid.0);
            let ask = format!("{}", q.ask.0);

            Row::new(vec![
                Cell::from(Span::styled(symbol, Style::default().fg(Color::White))),
                Cell::from(Span::styled(
                    bid,
                    Style::default().fg(Color::Green),
                )),
                Cell::from(Span::styled(
                    ask,
                    Style::default().fg(Color::Red),
                )),
            ])
        })
        .collect();

    let widths = [
        Constraint::Min(10),
        Constraint::Min(12),
        Constraint::Min(12),
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .block(
            Block::default()
                .title(Span::styled(
                    " PRICE FEED ",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ))
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded),
        )
        .column_spacing(1);

    f.render_widget(table, area);
}
