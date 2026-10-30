//! Smoke test for the snapshot tooling: a rounded block with two titles.

use chronoscope::test_support::{buffer_to_styled, buffer_to_text, render};
use ratatui::layout::Alignment;
use ratatui::style::{Color, Stylize};
use ratatui::text::Line;
use ratatui::widgets::{Block, BorderType};

fn draw_block() -> ratatui::buffer::Buffer {
    render(30, 3, |frame| {
        let block = Block::bordered()
            .border_type(BorderType::Rounded)
            .title(Line::from(" left ").fg(Color::Green))
            .title(Line::from(" right ").alignment(Alignment::Right));
        frame.render_widget(block, frame.area());
    })
}

#[test]
fn rounded_block_with_titles_text() {
    insta::assert_snapshot!(buffer_to_text(&draw_block()));
}

#[test]
fn rounded_block_with_titles_styled() {
    insta::assert_snapshot!(buffer_to_styled(&draw_block()));
}
