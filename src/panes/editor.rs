use crate::animation::AnimationEngine;
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

pub struct EditorPane;

impl EditorPane {
    pub fn render(&self, f: &mut Frame, area: Rect, engine: &AnimationEngine) {
        let block = Block::default()
            .title("Editor")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Green));

        // Get visible lines based on scroll offset and area height
        let content_height = area.height.saturating_sub(2) as usize; // Subtract borders
        let scroll_offset = engine.buffer.scroll_offset;
        let buffer_lines = &engine.buffer.lines;

        // Calculate line number width
        let total_lines = buffer_lines.len();
        let line_num_width = format!("{}", total_lines).len().max(3);

        let visible_lines: Vec<Line> = buffer_lines
            .iter()
            .skip(scroll_offset)
            .take(content_height)
            .enumerate()
            .map(|(idx, line_content)| {
                let line_num = scroll_offset + idx;
                let is_cursor_line = line_num == engine.buffer.cursor_line;

                let mut spans = Vec::new();

                // Line number
                let line_num_str = format!("{:>width$} ", line_num + 1, width = line_num_width);
                if is_cursor_line {
                    spans.push(Span::styled(
                        line_num_str,
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD),
                    ));
                } else {
                    spans.push(Span::styled(
                        line_num_str,
                        Style::default().fg(Color::DarkGray),
                    ));
                }

                // Line separator
                spans.push(Span::styled("│ ", Style::default().fg(Color::DarkGray)));

                // Check if cursor is on this line
                if is_cursor_line && engine.cursor_visible {
                    // Insert cursor character (use char indices, not byte indices)
                    let cursor_col = engine.buffer.cursor_col;
                    let chars: Vec<char> = line_content.chars().collect();

                    // Text before cursor
                    if cursor_col > 0 && cursor_col <= chars.len() {
                        let before: String = chars[..cursor_col].iter().collect();
                        spans.push(Span::raw(before));
                    }

                    // Cursor character
                    let cursor_char = chars.get(cursor_col).copied().unwrap_or(' ');
                    spans.push(Span::styled(
                        cursor_char.to_string(),
                        Style::default()
                            .bg(Color::White)
                            .fg(Color::Black)
                            .add_modifier(Modifier::BOLD),
                    ));

                    // Text after cursor
                    if cursor_col + 1 < chars.len() {
                        let after: String = chars[cursor_col + 1..].iter().collect();
                        spans.push(Span::raw(after));
                    }
                } else {
                    spans.push(Span::raw(line_content.clone()));
                }

                Line::from(spans)
            })
            .collect();

        let content = Paragraph::new(visible_lines).block(block);
        f.render_widget(content, area);
    }
}
