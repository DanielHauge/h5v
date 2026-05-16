use std::sync::{LazyLock, RwLock};

use ratatui::{
    layout::{Alignment, Constraint, Layout, Margin, Rect},
    style::Style,
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

use crate::{
    configure,
    ui::state::{
        AppState, HelpCommandSection, HelpCustomizationSection, HelpKeymapSection,
        HelpMultiChartSection, HelpSidebarHitbox, HelpSidebarTarget, HelpTab, HelpTabHitbox,
    },
};

use super::panels::{
    command_panel_text, customization_panel_text, heatmap_help_lines, help_desc_style,
    help_key_style, help_muted_style, keymap_panel_text, multichart_panel_text, paragraph_line,
};

const HELP_PANEL_CACHE_SIZE: usize = 27;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum HelpPanelCacheKey {
    Keymap(HelpKeymapSection),
    Command(HelpCommandSection),
    Customization(HelpCustomizationSection),
    MultiChart(HelpMultiChartSection),
    Guide(HelpTab),
}

#[derive(Clone)]
struct CachedHelpPanel {
    generation: u64,
    key: HelpPanelCacheKey,
    title: String,
    lines: Vec<Line<'static>>,
}

static HELP_PANEL_CACHE: LazyLock<RwLock<Vec<CachedHelpPanel>>> =
    LazyLock::new(|| RwLock::new(Vec::with_capacity(HELP_PANEL_CACHE_SIZE)));

pub fn render_help(frame: &mut Frame<'_>, area: Rect, state: &mut AppState<'_>) {
    warm_help_panel_cache();
    let popup = centered_rect(area, 176, 44);
    state.ui_layout.help_top_bar = Some(Rect {
        x: popup.x.saturating_add(1),
        y: popup.y,
        width: popup.width.saturating_sub(2),
        height: 1,
    });
    state.ui_layout.help_tabs.clear();
    state.ui_layout.help_sidebar_items.clear();
    state.ui_layout.help_content = None;
    state.ui_layout.help_scrollbar = None;

    frame.render_widget(
        Block::default()
            .style(Style::default().bg(configure::themed_color(|colors| colors.surface.bg_val3))),
        area,
    );
    frame.render_widget(Clear, popup);

    let help_block = Block::default()
        .borders(Borders::ALL)
        .border_style(
            Style::default().fg(configure::themed_color(|colors| colors.surface.break_line)),
        )
        .border_type(ratatui::widgets::BorderType::Rounded)
        .title(configure::configured_symbol(|symbols| symbols.title.help))
        .title_style(
            Style::default()
                .fg(configure::themed_color(|colors| colors.help.title))
                .bold(),
        )
        .title_bottom(Line::from(vec![
            Span::styled(" Esc ", help_key_style()),
            Span::styled(" close ", help_desc_style()),
            Span::styled("  ", help_muted_style()),
            Span::styled(" PgUp / PgDn ", help_key_style()),
            Span::styled(" scroll ", help_desc_style()),
            Span::styled("  ", help_muted_style()),
            Span::styled(" Tab / Shift+Tab ", help_key_style()),
            Span::styled(" switch tabs ", help_desc_style()),
            Span::styled("  ", help_muted_style()),
            Span::styled(" ←→ h/l ", help_key_style()),
            Span::styled(" tabs ", help_desc_style()),
            Span::styled("  ", help_muted_style()),
            Span::styled(" ↑↓ j/k ", help_key_style()),
            Span::styled(" browse lists ", help_desc_style()),
        ]))
        .title_alignment(Alignment::Center)
        .style(Style::default().bg(configure::themed_color(|colors| colors.surface.focus_bg)));
    frame.render_widget(help_block, popup);

    let inner = popup.inner(Margin {
        horizontal: 2,
        vertical: 1,
    });
    let sections = Layout::vertical([Constraint::Length(3), Constraint::Min(0)]).split(inner);
    render_tab_bar(frame, sections[0], state);

    match state.help.selected_tab {
        HelpTab::Keymap => render_keymap_help(frame, sections[1], state),
        HelpTab::Commands => render_command_help(frame, sections[1], state),
        HelpTab::MultiChart => render_multichart_help(frame, sections[1], state),
        HelpTab::Heatmap => render_cached_panel(
            frame,
            sections[1],
            state,
            HelpPanelCacheKey::Guide(HelpTab::Heatmap),
        ),
        HelpTab::Configuration => render_customization_help(frame, sections[1], state),
    }
}

pub fn centered_rect(area: Rect, max_width: u16, max_height: u16) -> Rect {
    let width = area.width.saturating_sub(4).min(max_width).max(20);
    let height = area.height.saturating_sub(4).min(max_height).max(10);

    let vertical = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(height),
        Constraint::Fill(1),
    ])
    .split(area);
    let horizontal = Layout::horizontal([
        Constraint::Fill(1),
        Constraint::Length(width),
        Constraint::Fill(1),
    ])
    .split(vertical[1]);
    horizontal[1]
}

fn render_tab_bar(frame: &mut Frame<'_>, area: Rect, state: &mut AppState<'_>) {
    let labels = [
        (HelpTab::Keymap, "Keymap"),
        (HelpTab::Commands, "Commands"),
        (HelpTab::MultiChart, "Multichart"),
        (HelpTab::Heatmap, "Heatmap"),
        (HelpTab::Configuration, "Customization"),
    ];
    let mut spans = Vec::new();
    let mut tab_layout = Vec::new();
    for (idx, (tab, label)) in labels.iter().enumerate() {
        if idx > 0 {
            spans.push(Span::styled("  ", help_muted_style()));
        }
        let padded = format!(" {label} ");
        tab_layout.push((
            *tab,
            padded.clone(),
            Line::from(padded.as_str()).width() as u16,
        ));
        let style = if *tab == state.help.selected_tab {
            Style::default()
                .fg(configure::themed_color(|colors| colors.accent.selection_fg))
                .bg(configure::themed_color(|colors| colors.accent.selection_bg))
                .bold()
        } else {
            Style::default()
                .fg(configure::themed_color(|colors| colors.help.description))
                .bg(configure::themed_color(|colors| colors.surface.help_key_bg))
                .bold()
        };
        spans.push(Span::styled(padded, style));
    }
    let line = Line::from(spans);
    let line_width = line.width() as u16;
    let start_x = area
        .x
        .saturating_add(area.width.saturating_sub(line_width) / 2);
    let separator_width = Line::from("  ").width() as u16;
    let mut current_x = start_x;
    for (idx, (tab, _, width)) in tab_layout.iter().enumerate() {
        state.ui_layout.help_tabs.push(HelpTabHitbox {
            area: Rect {
                x: current_x,
                y: area.y,
                width: *width,
                height: 1,
            },
            tab: *tab,
        });
        current_x = current_x.saturating_add(*width);
        if idx + 1 != tab_layout.len() {
            current_x = current_x.saturating_add(separator_width);
        }
    }

    frame.render_widget(
        Paragraph::new(line)
            .alignment(Alignment::Center)
            .style(Style::default().bg(configure::themed_color(|colors| colors.surface.focus_bg))),
        area,
    );
}

fn render_keymap_help(frame: &mut Frame<'_>, area: Rect, state: &mut AppState<'_>) {
    let layout = Layout::horizontal([Constraint::Length(24), Constraint::Min(0)])
        .spacing(1)
        .split(area);
    render_sidebar(
        frame,
        state,
        layout[0],
        "Modes",
        &[
            (HelpKeymapSection::Global, "Global"),
            (HelpKeymapSection::Normal, "Normal"),
            (HelpKeymapSection::Window, "Window"),
            (HelpKeymapSection::Tree, "Tree"),
            (HelpKeymapSection::Content, "Content"),
            (HelpKeymapSection::Heatmap, "Heatmap"),
            (HelpKeymapSection::Attributes, "Attributes"),
            (HelpKeymapSection::MultiChart, "Multichart"),
        ],
        state.help.keymap_section,
        HelpSidebarTarget::Keymap,
    );

    render_cached_panel(
        frame,
        layout[1],
        state,
        HelpPanelCacheKey::Keymap(state.help.keymap_section),
    );
}

fn render_command_help(frame: &mut Frame<'_>, area: Rect, state: &mut AppState<'_>) {
    let layout = Layout::horizontal([Constraint::Length(24), Constraint::Min(0)])
        .spacing(1)
        .split(area);
    render_sidebar(
        frame,
        state,
        layout[0],
        "Categories",
        &[
            (HelpCommandSection::Navigation, "Navigation"),
            (HelpCommandSection::View, "View"),
            (HelpCommandSection::Selection, "Selection"),
            (HelpCommandSection::Attributes, "Attributes"),
            (HelpCommandSection::App, "App"),
            (HelpCommandSection::MultiChart, "Multichart"),
            (HelpCommandSection::Input, "Input"),
        ],
        state.help.command_section,
        HelpSidebarTarget::Command,
    );

    render_cached_panel(
        frame,
        layout[1],
        state,
        HelpPanelCacheKey::Command(state.help.command_section),
    );
}

fn render_customization_help(frame: &mut Frame<'_>, area: Rect, state: &mut AppState<'_>) {
    let layout = Layout::horizontal([Constraint::Length(24), Constraint::Min(0)])
        .spacing(1)
        .split(area);
    render_sidebar(
        frame,
        state,
        layout[0],
        "Sections",
        &[
            (HelpCustomizationSection::Configuration, "Configuration"),
            (HelpCustomizationSection::Settings, "Settings"),
            (HelpCustomizationSection::Colors, "Colors"),
            (HelpCustomizationSection::Symbols, "Symbols"),
            (HelpCustomizationSection::Keymaps, "Keymaps"),
            (HelpCustomizationSection::Scripting, "Scripting"),
        ],
        state.help.customization_section,
        HelpSidebarTarget::Customization,
    );

    render_cached_panel(
        frame,
        layout[1],
        state,
        HelpPanelCacheKey::Customization(state.help.customization_section),
    );
}

fn render_multichart_help(frame: &mut Frame<'_>, area: Rect, state: &mut AppState<'_>) {
    let layout = Layout::horizontal([Constraint::Length(24), Constraint::Min(0)])
        .spacing(1)
        .split(area);
    render_sidebar(
        frame,
        state,
        layout[0],
        "Topics",
        &[
            (HelpMultiChartSection::Overview, "Overview"),
            (HelpMultiChartSection::Expressions, "Expressions"),
            (HelpMultiChartSection::FunctionReducers, "Fns: reducers"),
            (HelpMultiChartSection::FunctionMath, "Fns: math"),
            (HelpMultiChartSection::FunctionTransforms, "Fns: transforms"),
        ],
        state.help.multichart_section,
        HelpSidebarTarget::MultiChart,
    );
    render_cached_panel(
        frame,
        layout[1],
        state,
        HelpPanelCacheKey::MultiChart(state.help.multichart_section),
    );
}

fn render_cached_panel(
    frame: &mut Frame<'_>,
    area: Rect,
    state: &mut AppState<'_>,
    key: HelpPanelCacheKey,
) {
    let (title, lines) = cached_panel(key);
    render_content_panel(frame, area, state, title, lines);
}

fn render_sidebar<T: Copy + PartialEq>(
    frame: &mut Frame<'_>,
    state: &mut AppState<'_>,
    area: Rect,
    title: &str,
    items: &[(T, &str)],
    selected: T,
    make_target: impl Fn(T) -> HelpSidebarTarget,
) {
    let inner = area.inner(Margin {
        horizontal: 1,
        vertical: 1,
    });
    let lines = items
        .iter()
        .enumerate()
        .map(|(index, (item, label))| {
            if index < inner.height as usize {
                state.ui_layout.help_sidebar_items.push(HelpSidebarHitbox {
                    area: Rect {
                        x: inner.x,
                        y: inner.y.saturating_add(index as u16),
                        width: inner.width,
                        height: 1,
                    },
                    target: make_target(*item),
                });
            }
            if *item == selected {
                Line::from(Span::styled(
                    format!("> {label}"),
                    Style::default()
                        .fg(configure::themed_color(|colors| colors.accent.selection_fg))
                        .bg(configure::themed_color(|colors| colors.accent.selection_bg))
                        .bold(),
                ))
            } else {
                Line::from(Span::styled(format!("  {label}"), help_desc_style()))
            }
        })
        .collect::<Vec<_>>();

    frame.render_widget(
        Paragraph::new(Text::from(lines))
            .block(panel_block(title))
            .style(Style::default().bg(configure::themed_color(|colors| colors.surface.focus_bg))),
        area,
    );
}

fn render_content_panel(
    frame: &mut Frame<'_>,
    area: Rect,
    state: &mut AppState<'_>,
    title: impl Into<String>,
    lines: Vec<Line<'static>>,
) {
    let title = title.into();
    frame.render_widget(panel_block(&title), area);
    let inner = area.inner(Margin {
        horizontal: 2,
        vertical: 1,
    });
    state.ui_layout.help_content = Some(inner);
    if inner.width == 0 || inner.height == 0 {
        return;
    }
    let viewport_lines = inner.height as usize;
    let mut wrapped_lines = wrap_help_lines(&lines, inner.width as usize);
    let mut total_lines = wrapped_lines.len().max(1);
    let mut max_scroll = total_lines.saturating_sub(viewport_lines);
    let show_scrollbar = max_scroll > 0 && inner.width > 3;
    let (content_area, scrollbar_area) = if show_scrollbar {
        let split = Layout::horizontal([Constraint::Min(0), Constraint::Length(1)])
            .spacing(1)
            .split(inner);
        wrapped_lines = wrap_help_lines(&lines, split[0].width as usize);
        total_lines = wrapped_lines.len().max(1);
        max_scroll = total_lines.saturating_sub(viewport_lines);
        (split[0], Some(split[1]))
    } else {
        (inner, None)
    };
    if state.help.scroll_offset > max_scroll {
        state.help.scroll_offset = max_scroll;
    }
    state.ui_layout.help_content = Some(content_area);
    frame.render_widget(
        Paragraph::new(Text::from(wrapped_lines))
            .style(Style::default().bg(configure::themed_color(|colors| colors.surface.focus_bg)))
            .scroll((state.help.scroll_offset.min(u16::MAX as usize) as u16, 0)),
        content_area,
    );
    if let Some(scrollbar_area) = scrollbar_area {
        state.ui_layout.help_scrollbar = Some(crate::ui::state::HelpScrollbarHitbox {
            area: scrollbar_area,
            content_lines: total_lines,
            viewport_lines,
        });
        render_help_scrollbar(
            frame,
            scrollbar_area,
            state.help.scroll_offset,
            total_lines,
            viewport_lines,
        );
    }
}

fn panel_block(title: &str) -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .border_style(
            Style::default().fg(configure::themed_color(|colors| colors.surface.break_line)),
        )
        .border_type(ratatui::widgets::BorderType::Rounded)
        .title(title.to_string())
        .title_style(
            Style::default()
                .fg(configure::themed_color(|colors| colors.help.title))
                .bold(),
        )
}

fn render_help_scrollbar(
    frame: &mut Frame<'_>,
    area: Rect,
    scroll_offset: usize,
    total_lines: usize,
    viewport_lines: usize,
) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let track_len = area.height as usize;
    let thumb_len = ((viewport_lines.saturating_mul(track_len) + total_lines.saturating_sub(1))
        / total_lines.max(1))
    .max(1)
    .min(track_len);
    let max_scroll = total_lines.saturating_sub(viewport_lines);
    let thumb_start = if max_scroll == 0 || track_len <= thumb_len {
        0
    } else {
        scroll_offset.saturating_mul(track_len.saturating_sub(thumb_len)) / max_scroll
    };
    let thumb_end = thumb_start.saturating_add(thumb_len).min(track_len);
    let lines = (0..track_len)
        .map(|idx| {
            if (thumb_start..thumb_end).contains(&idx) {
                Line::from(Span::styled("█", help_scrollbar_thumb_style()))
            } else {
                Line::from(Span::styled("│", help_scrollbar_track_style()))
            }
        })
        .collect::<Vec<_>>();
    frame.render_widget(
        Paragraph::new(Text::from(lines))
            .style(Style::default().bg(configure::themed_color(|colors| colors.surface.focus_bg))),
        area,
    );
}

fn wrap_help_lines(lines: &[Line<'static>], width: usize) -> Vec<Line<'static>> {
    let width = width.max(1);
    let mut wrapped = Vec::new();
    for line in lines {
        if line.spans.is_empty() || line.width() == 0 {
            wrapped.push(Line::default());
            continue;
        }
        let mut current_spans = Vec::new();
        let mut current_width = 0usize;
        for span in &line.spans {
            let mut remaining = span.content.to_string();
            if remaining.is_empty() {
                continue;
            }
            while !remaining.is_empty() {
                if current_width == width {
                    wrapped.push(Line::from(current_spans));
                    current_spans = Vec::new();
                    current_width = 0;
                }
                let available = width.saturating_sub(current_width).max(1);
                let (chunk, rest) = split_prefix_by_width(&remaining, available);
                if chunk.is_empty() {
                    break;
                }
                current_width += chunk.chars().count();
                current_spans.push(Span::styled(chunk, span.style));
                remaining = rest;
                if current_width == width {
                    wrapped.push(Line::from(current_spans));
                    current_spans = Vec::new();
                    current_width = 0;
                }
            }
        }
        if !current_spans.is_empty() {
            wrapped.push(Line::from(current_spans));
        }
    }
    if wrapped.is_empty() {
        wrapped.push(Line::default());
    }
    wrapped
}

fn split_prefix_by_width(text: &str, width: usize) -> (String, String) {
    if width == 0 {
        return (String::new(), text.to_string());
    }
    let mut end = text.len();
    let mut count = 0usize;
    for (idx, ch) in text.char_indices() {
        if count == width {
            end = idx;
            break;
        }
        count += 1;
        end = idx + ch.len_utf8();
    }
    if count <= width && end == text.len() {
        (text.to_string(), String::new())
    } else {
        (text[..end].to_string(), text[end..].to_string())
    }
}

fn help_scrollbar_track_style() -> Style {
    Style::default()
        .fg(configure::themed_color(|colors| colors.help.muted))
        .bg(configure::themed_color(|colors| colors.surface.focus_bg))
}

fn help_scrollbar_thumb_style() -> Style {
    Style::default()
        .fg(configure::themed_color(|colors| colors.text.primary))
        .bg(configure::themed_color(|colors| colors.surface.focus_bg))
        .bold()
}

fn cached_panel(key: HelpPanelCacheKey) -> (String, Vec<Line<'static>>) {
    let generation = configure::current_config_generation();
    let guard = match HELP_PANEL_CACHE.read() {
        Ok(guard) => guard,
        Err(error) => error.into_inner(),
    };
    if let Some(panel) = guard
        .iter()
        .find(|panel| panel.generation == generation && panel.key == key)
    {
        return (panel.title.clone(), panel.lines.clone());
    }
    drop(guard);
    warm_help_panel_cache();
    let guard = match HELP_PANEL_CACHE.read() {
        Ok(guard) => guard,
        Err(error) => error.into_inner(),
    };
    guard
        .iter()
        .find(|panel| panel.generation == generation && panel.key == key)
        .map(|panel| (panel.title.clone(), panel.lines.clone()))
        .unwrap_or_else(|| {
            (
                "Help".to_string(),
                vec![paragraph_line("Help content unavailable.")],
            )
        })
}

fn warm_help_panel_cache() {
    let generation = configure::current_config_generation();
    {
        let guard = match HELP_PANEL_CACHE.read() {
            Ok(guard) => guard,
            Err(error) => error.into_inner(),
        };
        let cached_count = guard
            .iter()
            .filter(|panel| panel.generation == generation)
            .count();
        if cached_count == HELP_PANEL_CACHE_SIZE {
            return;
        }
    }

    let keymaps = configure::current_keymaps();
    let mut panels = Vec::with_capacity(HELP_PANEL_CACHE_SIZE);
    for section in [
        HelpKeymapSection::Global,
        HelpKeymapSection::Normal,
        HelpKeymapSection::Window,
        HelpKeymapSection::Tree,
        HelpKeymapSection::Content,
        HelpKeymapSection::Heatmap,
        HelpKeymapSection::Attributes,
        HelpKeymapSection::MultiChart,
    ] {
        let (title, lines) = keymap_panel_text(&keymaps, section);
        panels.push(CachedHelpPanel {
            generation,
            key: HelpPanelCacheKey::Keymap(section),
            title,
            lines,
        });
    }

    for section in [
        HelpCommandSection::Navigation,
        HelpCommandSection::View,
        HelpCommandSection::Selection,
        HelpCommandSection::Attributes,
        HelpCommandSection::App,
        HelpCommandSection::MultiChart,
        HelpCommandSection::Input,
    ] {
        let (title, lines) = command_panel_text(section);
        panels.push(CachedHelpPanel {
            generation,
            key: HelpPanelCacheKey::Command(section),
            title,
            lines,
        });
    }

    for section in [
        HelpCustomizationSection::Configuration,
        HelpCustomizationSection::Settings,
        HelpCustomizationSection::Colors,
        HelpCustomizationSection::Symbols,
        HelpCustomizationSection::Keymaps,
        HelpCustomizationSection::Scripting,
    ] {
        let (title, lines) = customization_panel_text(section);
        panels.push(CachedHelpPanel {
            generation,
            key: HelpPanelCacheKey::Customization(section),
            title,
            lines,
        });
    }

    for (tab, title, lines) in [(
        HelpTab::Heatmap,
        "Heatmap".to_string(),
        heatmap_help_lines(),
    )] {
        panels.push(CachedHelpPanel {
            generation,
            key: HelpPanelCacheKey::Guide(tab),
            title,
            lines,
        });
    }
    for section in [
        HelpMultiChartSection::Overview,
        HelpMultiChartSection::Expressions,
        HelpMultiChartSection::FunctionReducers,
        HelpMultiChartSection::FunctionMath,
        HelpMultiChartSection::FunctionTransforms,
    ] {
        let (title, lines) = multichart_panel_text(section);
        panels.push(CachedHelpPanel {
            generation,
            key: HelpPanelCacheKey::MultiChart(section),
            title,
            lines,
        });
    }

    let mut guard = match HELP_PANEL_CACHE.write() {
        Ok(guard) => guard,
        Err(error) => error.into_inner(),
    };
    let cached_count = guard
        .iter()
        .filter(|panel| panel.generation == generation)
        .count();
    if cached_count == HELP_PANEL_CACHE_SIZE {
        return;
    }
    guard.clear();
    guard.extend(panels);
}
