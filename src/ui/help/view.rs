use std::sync::{LazyLock, RwLock};

use ratatui::{
    layout::{Alignment, Constraint, Layout, Margin, Rect},
    style::Style,
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};

use crate::{
    configure,
    ui::state::{
        AppState, HelpCommandSection, HelpCustomizationSection, HelpKeymapSection, HelpTab,
    },
};

use super::panels::{
    command_panel_text, customization_panel_text, heatmap_help_lines, help_desc_style,
    help_key_style, help_muted_style, keymap_panel_text, multichart_help_lines, paragraph_line,
};

const HELP_PANEL_CACHE_SIZE: usize = 23;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum HelpPanelCacheKey {
    Keymap(HelpKeymapSection),
    Command(HelpCommandSection),
    Customization(HelpCustomizationSection),
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

pub fn render_help(frame: &mut Frame<'_>, area: Rect, state: &AppState<'_>) {
    warm_help_panel_cache();
    let popup = centered_rect(area, 176, 44);

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
    render_tab_bar(frame, sections[0], state.help.selected_tab);

    match state.help.selected_tab {
        HelpTab::Keymap => render_keymap_help(frame, sections[1], state),
        HelpTab::Commands => render_command_help(frame, sections[1], state),
        HelpTab::MultiChart => render_cached_panel(
            frame,
            sections[1],
            HelpPanelCacheKey::Guide(HelpTab::MultiChart),
        ),
        HelpTab::Heatmap => render_cached_panel(
            frame,
            sections[1],
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

fn render_tab_bar(frame: &mut Frame<'_>, area: Rect, selected: HelpTab) {
    let labels = [
        (HelpTab::Keymap, "Keymap"),
        (HelpTab::Commands, "Commands"),
        (HelpTab::MultiChart, "Multichart"),
        (HelpTab::Heatmap, "Heatmap"),
        (HelpTab::Configuration, "Customization"),
    ];
    let mut spans = Vec::new();
    for (idx, (tab, label)) in labels.iter().enumerate() {
        if idx > 0 {
            spans.push(Span::styled("  ", help_muted_style()));
        }
        let style = if *tab == selected {
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
        spans.push(Span::styled(format!(" {label} "), style));
    }

    frame.render_widget(
        Paragraph::new(Line::from(spans))
            .alignment(Alignment::Center)
            .style(Style::default().bg(configure::themed_color(|colors| colors.surface.focus_bg))),
        area,
    );
}

fn render_keymap_help(frame: &mut Frame<'_>, area: Rect, state: &AppState<'_>) {
    let layout = Layout::horizontal([Constraint::Length(24), Constraint::Min(0)])
        .spacing(1)
        .split(area);
    render_sidebar(
        frame,
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
    );

    render_cached_panel(
        frame,
        layout[1],
        HelpPanelCacheKey::Keymap(state.help.keymap_section),
    );
}

fn render_command_help(frame: &mut Frame<'_>, area: Rect, state: &AppState<'_>) {
    let layout = Layout::horizontal([Constraint::Length(24), Constraint::Min(0)])
        .spacing(1)
        .split(area);
    render_sidebar(
        frame,
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
    );

    render_cached_panel(
        frame,
        layout[1],
        HelpPanelCacheKey::Command(state.help.command_section),
    );
}

fn render_customization_help(frame: &mut Frame<'_>, area: Rect, state: &AppState<'_>) {
    let layout = Layout::horizontal([Constraint::Length(24), Constraint::Min(0)])
        .spacing(1)
        .split(area);
    render_sidebar(
        frame,
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
    );

    render_cached_panel(
        frame,
        layout[1],
        HelpPanelCacheKey::Customization(state.help.customization_section),
    );
}

fn render_cached_panel(frame: &mut Frame<'_>, area: Rect, key: HelpPanelCacheKey) {
    let (title, lines) = cached_panel(key);
    render_content_panel(frame, area, title, lines);
}

fn render_sidebar<T: Copy + PartialEq>(
    frame: &mut Frame<'_>,
    area: Rect,
    title: &str,
    items: &[(T, &str)],
    selected: T,
) {
    let lines = items
        .iter()
        .map(|(item, label)| {
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
    title: impl Into<String>,
    lines: Vec<Line<'static>>,
) {
    frame.render_widget(
        Paragraph::new(Text::from(lines))
            .block(panel_block(&title.into()))
            .style(Style::default().bg(configure::themed_color(|colors| colors.surface.focus_bg)))
            .wrap(Wrap { trim: false }),
        area,
    );
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

    for (tab, title, lines) in [
        (
            HelpTab::MultiChart,
            "Multichart".to_string(),
            multichart_help_lines(),
        ),
        (
            HelpTab::Heatmap,
            "Heatmap".to_string(),
            heatmap_help_lines(),
        ),
    ] {
        panels.push(CachedHelpPanel {
            generation,
            key: HelpPanelCacheKey::Guide(tab),
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
