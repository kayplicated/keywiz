use ratatui::layout::Rect;
use ratatui::style::{Color, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::layout::Layout;

/// Key cell width in characters (including borders).
const KEY_W: u16 = 5;
/// Key cell height in lines.
const KEY_H: u16 = 3;

/// Row offsets (in half-key units) to simulate stagger.
/// Number row: 0, Top: 1, Home: 2, Bottom: 3
const ROW_OFFSETS: [u16; 4] = [0, 1, 3, 5];

pub fn render_keyboard(f: &mut Frame, area: Rect, layout: &Layout, highlight: Option<char>) {
    let highlight_lower = highlight.map(|c| c.to_ascii_lowercase());

    // Center the keyboard in the area
    let max_keys = layout.rows.iter().map(|r| r.keys.len()).max().unwrap_or(13);
    let kb_width = (max_keys as u16) * KEY_W + ROW_OFFSETS[3] + 2;
    let kb_height = 4 * KEY_H;

    let x_offset = area.x + area.width.saturating_sub(kb_width) / 2;
    let y_offset = area.y + area.height.saturating_sub(kb_height) / 2;

    for (row_idx, row) in layout.rows.iter().enumerate() {
        let stagger = ROW_OFFSETS[row_idx];
        let row_y = y_offset + (row_idx as u16) * KEY_H;

        for (col_idx, key) in row.keys.iter().enumerate() {
            let key_x = x_offset + stagger + (col_idx as u16) * KEY_W;

            let is_highlighted = highlight_lower == Some(key.lower);

            if key_x + KEY_W <= area.x + area.width && row_y + KEY_H <= area.y + area.height {
                let key_area = Rect::new(key_x, row_y, KEY_W, KEY_H);

                let lines = if is_highlighted {
                    let border = Style::default().fg(Color::Gray);
                    let letter = Style::default().fg(Color::White).bold();
                    vec![
                        Line::from(Span::styled("┌───┐", border)),
                        Line::from(vec![
                            Span::styled("│ ", border),
                            Span::styled(format!("{}", key.lower), letter),
                            Span::styled(" │", border),
                        ]),
                        Line::from(Span::styled("└───┘", border)),
                    ]
                } else {
                    let style = Style::default().fg(key.finger.color());
                    vec![
                        Line::from(Span::styled("┌───┐", style)),
                        Line::from(Span::styled(format!("│ {} │", key.lower), style)),
                        Line::from(Span::styled("└───┘", style)),
                    ]
                };

                f.render_widget(Paragraph::new(lines), key_area);
            }
        }
    }
}
