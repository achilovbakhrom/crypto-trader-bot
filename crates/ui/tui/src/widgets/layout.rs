use ratatui::layout::{Constraint, Direction, Layout, Rect};

/// Compute the four panel areas from the full terminal `area`.
///
/// Returns `(header_area, [positions, prices, system], log_area)`.
pub fn build_layout(area: Rect) -> (Rect, [Rect; 3], Rect) {
    // Outer split: header / middle row / log
    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // header
            Constraint::Min(10),    // middle panels
            Constraint::Length(10), // event log
        ])
        .split(area);

    let header_area = outer[0];
    let middle_area = outer[1];
    let log_area = outer[2];

    // Middle row: three equal panels
    let middle = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Ratio(1, 3),
            Constraint::Ratio(1, 3),
            Constraint::Ratio(1, 3),
        ])
        .split(middle_area);

    (header_area, [middle[0], middle[1], middle[2]], log_area)
}
