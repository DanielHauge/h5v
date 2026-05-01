use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph, Scrollbar, ScrollbarState, Wrap},
    Frame,
};

use crate::color_consts;
use crate::error::AppError;

pub struct SegmentDisplayInfo<'a> {
    pub title: &'a str,
    pub current: usize,
    pub total: usize,
    pub range_start: usize,
    pub range_end: usize,
    pub total_items: usize,
    pub unit: &'a str,
}

pub fn render_position_scroll(
    f: &mut Frame,
    area: &Rect,
    total: usize,
    position: usize,
    viewport_len: usize,
) -> Result<(), AppError> {
    let scrollbar = Scrollbar::new(ratatui::widgets::ScrollbarOrientation::VerticalRight)
        .begin_symbol(Some("⬆"))
        .thumb_symbol("█")
        .end_symbol(Some("⬇"));
    let mut scrollbar_state = ScrollbarState::new(total.max(1))
        .viewport_content_length(viewport_len.max(1))
        .position(position.min(total.saturating_sub(1)));
    f.render_stateful_widget(scrollbar, *area, &mut scrollbar_state);
    Ok(())
}

pub fn render_segment_panel(
    f: &mut Frame,
    area: &Rect,
    info: &SegmentDisplayInfo<'_>,
) -> Result<(), AppError> {
    let block = Block::default()
        .title(format!(
            " {} {}/{} ",
            info.title,
            info.current.saturating_add(1),
            info.total.max(1)
        ))
        .title_style(Style::default().fg(color_consts::TITLE).bold())
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(color_consts::BREAK_COLOR))
        .style(Style::default().bg(color_consts::BG_VAL3_COLOR));
    let inner = block.inner(*area);
    f.render_widget(block, *area);

    let split = Layout::horizontal([Constraint::Min(1), Constraint::Length(2)]).split(inner);
    let [text_area, scroll_area] = split.as_ref() else {
        return Ok(());
    };

    let size = info.range_end.saturating_sub(info.range_start);
    let start_pct = if info.total_items == 0 {
        0.0
    } else {
        (info.range_start as f64 / info.total_items as f64) * 100.0
    };
    let end_pct = if info.total_items == 0 {
        0.0
    } else {
        (info.range_end as f64 / info.total_items as f64) * 100.0
    };
    let lines = vec![
        Line::from(vec![
            Span::styled("range ", Style::default().fg(color_consts::TYPE_DESC_COLOR)),
            Span::raw(format!(
                "{}..{}",
                compact_count(info.range_start),
                compact_count(info.range_end.saturating_sub(1))
            )),
        ]),
        Line::from(vec![
            Span::styled("size  ", Style::default().fg(color_consts::TYPE_DESC_COLOR)),
            Span::raw(format!("{} {}", compact_count(size), info.unit)),
        ]),
        Line::from(vec![
            Span::styled("total ", Style::default().fg(color_consts::TYPE_DESC_COLOR)),
            Span::raw(format!("{} {}", compact_count(info.total_items), info.unit)),
        ]),
        Line::from(vec![
            Span::styled("cover ", Style::default().fg(color_consts::TYPE_DESC_COLOR)),
            Span::raw(format!("{start_pct:.1}-{end_pct:.1}%")),
        ]),
        Line::from(vec![
            Span::styled("nav   ", Style::default().fg(color_consts::TYPE_DESC_COLOR)),
            Span::raw("j/k PgUp/Dn"),
        ]),
    ];
    f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), *text_area);
    render_position_scroll(f, scroll_area, info.total, info.current, 1)
}

fn compact_count(value: usize) -> String {
    match value {
        0..=999 => value.to_string(),
        1_000..=999_999 => format!("{:.1}k", value as f64 / 1_000.0),
        1_000_000..=999_999_999 => format!("{:.1}M", value as f64 / 1_000_000.0),
        _ => format!("{:.1}B", value as f64 / 1_000_000_000.0),
    }
}
