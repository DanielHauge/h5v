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
    ui::{
        command::{command_catalog, command_usage, CommandCategory, CommandDescriptor},
        input::keymap::{
            AttributesAction, BoundAction, ContentAction, Direction, EffectiveKeymaps,
            GlobalAction, KeyBinding, MultiChartAction, NormalAction, TreeAction, WindowAction,
        },
        state::{
            AppState, HelpCommandSection, HelpCustomizationSection, HelpKeymapSection, HelpTab,
        },
        std_comp_render::highlighted_lines,
    },
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

fn keymap_panel_text(
    keymaps: &EffectiveKeymaps,
    section: HelpKeymapSection,
) -> (String, Vec<Line<'static>>) {
    match section {
        HelpKeymapSection::Global => (
            "Global keymaps".to_string(),
            grouped_keymap_lines(
                &keymaps.global,
                describe_global_target,
                "Available everywhere",
            ),
        ),
        HelpKeymapSection::Normal => (
            "Normal mode".to_string(),
            grouped_keymap_lines(
                &keymaps.normal,
                describe_normal_target,
                "Core app navigation and mode switches",
            ),
        ),
        HelpKeymapSection::Window => (
            "Window chord".to_string(),
            grouped_keymap_lines(
                &keymaps.window,
                describe_window_target,
                "Used after Ctrl+W for pane management",
            ),
        ),
        HelpKeymapSection::Tree => (
            "Tree pane".to_string(),
            grouped_keymap_lines(
                &keymaps.tree,
                describe_tree_target,
                "Dataset and group browsing",
            ),
        ),
        HelpKeymapSection::Content => (
            "Content pane".to_string(),
            grouped_keymap_lines(
                &keymaps.content,
                describe_content_target,
                "Preview and matrix navigation",
            ),
        ),
        HelpKeymapSection::Heatmap => (
            "Heatmap extras".to_string(),
            grouped_keymap_lines(
                &keymaps.heatmap,
                describe_content_target,
                "Heatmap-only bindings layered on top of content/global bindings",
            ),
        ),
        HelpKeymapSection::Attributes => (
            "Attributes pane".to_string(),
            grouped_keymap_lines(
                &keymaps.attributes,
                describe_attributes_target,
                "Metadata editing and navigation",
            ),
        ),
        HelpKeymapSection::MultiChart => (
            "Multichart mode".to_string(),
            grouped_keymap_lines(
                &keymaps.multichart,
                describe_multichart_target,
                "Series management, pan, zoom, and expressions",
            ),
        ),
    }
}

fn command_panel_text(section: HelpCommandSection) -> (String, Vec<Line<'static>>) {
    let (title, category) = match section {
        HelpCommandSection::Navigation => ("Navigation commands", CommandCategory::Navigation),
        HelpCommandSection::View => ("View commands", CommandCategory::View),
        HelpCommandSection::Selection => ("Selection commands", CommandCategory::Selection),
        HelpCommandSection::Attributes => ("Attribute commands", CommandCategory::Attributes),
        HelpCommandSection::App => ("App commands", CommandCategory::App),
        HelpCommandSection::MultiChart => ("Multichart commands", CommandCategory::MultiChart),
        HelpCommandSection::Input => ("Input commands", CommandCategory::Input),
    };

    let mut lines = vec![
        Line::from(Span::styled(
            "Commands are available from ':' and also power startup scripts and Lua helpers.",
            help_desc_style(),
        )),
        Line::raw(""),
    ];
    let descriptors = command_catalog()
        .iter()
        .filter(|descriptor| descriptor.category == category)
        .collect::<Vec<_>>();
    for (idx, descriptor) in descriptors.iter().enumerate() {
        lines.extend(command_descriptor_lines(descriptor));
        if idx + 1 != descriptors.len() {
            lines.push(Line::raw(""));
        }
    }
    (title.to_string(), lines)
}

fn command_descriptor_lines(descriptor: &CommandDescriptor) -> Vec<Line<'static>> {
    let mut lines = vec![Line::from(vec![
        Span::styled(command_usage(descriptor), help_key_style()),
        Span::raw("  "),
        Span::styled(descriptor.description.to_string(), help_desc_style()),
    ])];
    if !descriptor.aliases.is_empty() {
        lines.push(Line::from(Span::styled(
            format!("aliases: {}", descriptor.aliases.join(", ")),
            help_muted_style(),
        )));
    }
    if !descriptor.keybindings.is_empty() {
        lines.push(Line::from(Span::styled(
            format!("keys: {}", descriptor.keybindings.join(", ")),
            help_muted_style(),
        )));
    }
    lines
}

fn grouped_keymap_lines<T>(
    bindings: &[KeyBinding<T>],
    describe_target: fn(&BoundAction<T>) -> String,
    intro: &str,
) -> Vec<Line<'static>> {
    let mut lines = vec![
        Line::from(Span::styled(intro.to_string(), help_desc_style())),
        Line::from(Span::styled(
            "The list reflects the current active keymap, including Lua config overrides.",
            help_muted_style(),
        )),
        Line::raw(""),
    ];
    let mut grouped: Vec<(String, Vec<String>)> = Vec::new();
    for binding in bindings {
        let description = binding
            .description
            .clone()
            .unwrap_or_else(|| describe_target(&binding.target));
        let key = binding.key.to_string();
        if let Some((_, keys)) = grouped.iter_mut().find(|(desc, _)| *desc == description) {
            keys.push(key);
        } else {
            grouped.push((description, vec![key]));
        }
    }

    if grouped.is_empty() {
        lines.push(Line::from(Span::styled(
            "No bindings available.",
            help_muted_style(),
        )));
        return lines;
    }

    for (description, keys) in grouped {
        lines.push(Line::from(vec![
            Span::styled(keys.join(", "), help_key_style()),
            Span::raw("  "),
            Span::styled(description, help_desc_style()),
        ]));
    }
    lines
}

fn describe_global_target(target: &BoundAction<GlobalAction>) -> String {
    describe_bound_action(target, |action| match action {
        GlobalAction::EnterCommand => "Open command mode",
        GlobalAction::ShowHelp => "Open help",
        GlobalAction::Quit => "Quit the app",
        GlobalAction::ReloadFile => "Reload the current file",
        GlobalAction::ToggleMultiChart => "Toggle multichart mode",
    })
}

fn describe_normal_target(target: &BoundAction<NormalAction>) -> String {
    describe_bound_action(target, |action| -> String {
        match action {
            NormalAction::EnterCommand => "Open command mode".to_string(),
            NormalAction::RepeatCommand => "Repeat the last successful command".to_string(),
            NormalAction::EnterSearch => "Open search".to_string(),
            NormalAction::Quit => "Quit the app".to_string(),
            NormalAction::ToggleContentMode => "Cycle content modes".to_string(),
            NormalAction::ShowHelp => "Open help".to_string(),
            NormalAction::ToggleMultiChart => "Open multichart".to_string(),
            NormalAction::ToggleTreeView => "Show or hide the tree pane".to_string(),
            NormalAction::ReloadFile => "Reload the current file".to_string(),
            NormalAction::Focus(direction) => focus_description(*direction).to_string(),
            NormalAction::StartWindowChord => "Start the Ctrl+W window chord".to_string(),
            NormalAction::ChangeX(delta) => step_description("Change preview X dimension", *delta),
            NormalAction::ChangeRow(delta) => step_description("Change row dimension", *delta),
            NormalAction::ChangeCol(delta) => step_description("Change column dimension", *delta),
            NormalAction::ChangeSelectedIndex(delta) => {
                step_description("Change the selected index", *delta)
            }
            NormalAction::ChangeSelectedDimension(delta) => {
                step_description("Change the selected dimension", *delta)
            }
            NormalAction::Scroll(direction, amount) => {
                format!("Scroll {} by {}", direction_label(*direction), amount)
            }
        }
    })
}

fn describe_window_target(target: &BoundAction<WindowAction>) -> String {
    describe_bound_action(target, |action| match action {
        WindowAction::Focus(direction) => focus_description(*direction),
        WindowAction::ToggleTreeView => "Show or hide the tree pane",
    })
}

fn describe_tree_target(target: &BoundAction<TreeAction>) -> String {
    describe_bound_action(target, |action| -> String {
        match action {
            TreeAction::MoveUp(amount) => format!("Move up by {}", amount),
            TreeAction::MoveDown(amount) => format!("Move down by {}", amount),
            TreeAction::MoveTop => "Jump to the top".to_string(),
            TreeAction::MoveBottom => "Jump to the bottom".to_string(),
            TreeAction::Collapse => "Collapse the selected node".to_string(),
            TreeAction::Expand => "Expand the selected node".to_string(),
            TreeAction::Toggle => "Toggle expansion".to_string(),
            TreeAction::AddToMultiChart => "Add the current selection to multichart".to_string(),
        }
    })
}

fn describe_content_target(target: &BoundAction<ContentAction>) -> String {
    describe_bound_action(target, |action| -> String {
        match action {
            ContentAction::Move(direction, amount) => {
                format!("Move {} by {}", direction_label(*direction), amount)
            }
            ContentAction::Edit => "Edit the selected value".to_string(),
            ContentAction::Copy => "Copy the selected value".to_string(),
            ContentAction::HeatmapZoomIn => {
                "Zoom in to the selected or hovered heatmap region".to_string()
            }
            ContentAction::HeatmapZoomOut => "Zoom out the heatmap viewport".to_string(),
            ContentAction::HeatmapResetView => "Reset the heatmap viewport".to_string(),
            ContentAction::HeatmapClearSelection => "Clear the heatmap selection".to_string(),
            ContentAction::HeatmapPan(direction) => {
                format!("Pan the heatmap {}", direction_label(*direction))
            }
        }
    })
}

fn describe_attributes_target(target: &BoundAction<AttributesAction>) -> String {
    describe_bound_action(target, |action| -> String {
        match action {
            AttributesAction::Move(direction, amount) => {
                format!("Move {} by {}", direction_label(*direction), amount)
            }
            AttributesAction::Edit => "Edit the selected attribute".to_string(),
            AttributesAction::Copy => "Copy the selected attribute value".to_string(),
            AttributesAction::Create => "Create an attribute".to_string(),
            AttributesAction::Delete => "Delete the selected attribute".to_string(),
        }
    })
}

fn describe_multichart_target(target: &BoundAction<MultiChartAction>) -> String {
    describe_bound_action(target, |action| -> String {
        match action {
            MultiChartAction::EnterCommand => "Open command mode over multichart".to_string(),
            MultiChartAction::Exit => "Close multichart".to_string(),
            MultiChartAction::Quit => "Quit the app".to_string(),
            MultiChartAction::ShowHelp => "Open the multichart help page".to_string(),
            MultiChartAction::ZoomIn => "Zoom in".to_string(),
            MultiChartAction::ZoomOut => "Zoom out".to_string(),
            MultiChartAction::PanLeft => "Pan left".to_string(),
            MultiChartAction::PanRight => "Pan right".to_string(),
            MultiChartAction::ClearZoom => "Reset zoom".to_string(),
            MultiChartAction::DeleteSelected => "Remove the selected series".to_string(),
            MultiChartAction::ClearAll => "Remove all series".to_string(),
            MultiChartAction::ToggleSelectedVisible => {
                "Show or hide the selected series".to_string()
            }
            MultiChartAction::OpenExpressionPrompt => "Open the expression editor".to_string(),
            MultiChartAction::MoveUp => "Select the previous series".to_string(),
            MultiChartAction::MoveDown => "Select the next series".to_string(),
        }
    })
}

fn describe_bound_action<T, S: Into<String>>(
    target: &BoundAction<T>,
    describe_action: impl Fn(&T) -> S,
) -> String {
    match target {
        BoundAction::Action(action) => describe_action(action).into(),
        BoundAction::Command(command) => format!("Run command: {command}"),
        BoundAction::Script(script) => {
            let first = script.lines().next().unwrap_or_default().trim();
            if first.is_empty() {
                "Run keybinding script".to_string()
            } else {
                format!("Run keybinding script: {first}")
            }
        }
        BoundAction::LuaCallback(_) => "Run a Lua callback".to_string(),
    }
}

fn direction_label(direction: Direction) -> &'static str {
    match direction {
        Direction::Left => "left",
        Direction::Right => "right",
        Direction::Up => "up",
        Direction::Down => "down",
    }
}

fn focus_description(direction: Direction) -> &'static str {
    match direction {
        Direction::Left => "Focus the pane to the left",
        Direction::Right => "Focus the pane to the right",
        Direction::Up => "Focus the pane above",
        Direction::Down => "Focus the pane below",
    }
}

fn step_description(label: &str, delta: isize) -> String {
    if delta > 0 {
        format!("{label} forward by {}", delta)
    } else {
        format!("{label} backward by {}", delta.abs())
    }
}

fn multichart_help_lines() -> Vec<Line<'static>> {
    guide_text(&[
        (
            "Overview",
            &[
                "Multichart lets you compare several dataset selections and expressions in one plot.",
                "Open it with M, add the current previewable selection with m, or use :mchart add from anywhere.",
            ],
        ),
        (
            "Expressions",
            &[
                "Press Enter or e to open the expression editor below the chart. Enter submits, Tab completes, and Esc closes it.",
                "Use $1 for an existing item, $1[0..128] for an item slice, !/group/data[..,0] for a dataset series, and #/group/value for a scalar.",
                "Tuple expressions make explicit x/y plots, for example (!/time[..], $2 * #/calibration:scale).",
            ],
        ),
        (
            "Navigation",
            &[
                "j/k select series, Space or v toggles visibility, and ? opens this help page directly from multichart.",
                "Use h/l or Shift+Left/Right to pan and + / - (or Shift+Up/Down) to zoom.",
                "c resets zoom, d/Delete removes the selected series, and C clears everything.",
            ],
        ),
    ])
}

fn heatmap_help_lines() -> Vec<Line<'static>> {
    guide_text(&[
        (
            "Overview",
            &[
                "Heatmap mode gives you a dense overview of matrix-like data, with selection stats and a configurable color scale.",
                "Use Tab to switch into heatmap mode when the dataset supports it.",
            ],
        ),
        (
            "Selection and viewport",
            &[
                "Left click selects a cell, wheel zooms toward the hovered region, and right-drag pans the zoomed viewport.",
                "z / Z zoom in and out, 0 resets the viewport, and v clears the current selection.",
                "H J K L pan the zoomed viewport when you want precise keyboard control.",
            ],
        ),
        (
            "Settings and presets",
            &[
                "Arrow keys move through the heatmap settings card and adjust colormap, normalization, and axis inversion.",
                "Use :heatmap range ... commands or h5v.heatmap.range_modes in Lua to define custom named ranges.",
                "The sidebar shows legend and region stats for the current viewport or selection.",
            ],
        ),
    ])
}

fn customization_panel_text(section: HelpCustomizationSection) -> (String, Vec<Line<'static>>) {
    match section {
        HelpCustomizationSection::Configuration => customization_configuration_panel(),
        HelpCustomizationSection::Settings => customization_settings_panel(),
        HelpCustomizationSection::Colors => customization_colors_panel(),
        HelpCustomizationSection::Symbols => customization_symbols_panel(),
        HelpCustomizationSection::Keymaps => customization_keymaps_panel(),
        HelpCustomizationSection::Scripting => customization_scripting_panel(),
    }
}

fn customization_configuration_panel() -> (String, Vec<Line<'static>>) {
    let config_path = configure::config_path()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|error| format!("Unavailable: {error}"));
    let mut lines = vec![
        paragraph_line(
            "Use :configure to open the active init.lua in $VISUAL or $EDITOR. h5v reloads it automatically when you return, so the feedback loop stays short.",
        ),
        paragraph_line(
            "Use :configure reset when you want to replace the file with the default scaffold. Configuration errors are non-fatal and stay visible until the file loads cleanly.",
        ),
        Line::raw(""),
        Line::from(vec![
            Span::styled("Loaded config path: ", help_muted_style()),
            Span::styled(config_path, help_desc_style()),
        ]),
        Line::raw(""),
        paragraph_line("Common entry points:"),
    ];
    lines.extend(highlighted_code_block(
        "sh",
        "terminal",
        ":configure\n:configure reset\nhelp reload",
    ));
    lines.push(Line::raw(""));
    lines.push(paragraph_line(
        "A minimal init.lua usually starts with just a few high-level choices:",
    ));
    lines.extend(highlighted_code_block(
        "lua",
        "lua",
        "h5v.theme = \"light\"\nh5v.symbol_theme = \"compatibility\"\nh5v.content_mode_order = { \"preview\", \"matrix\", \"heatmap\" }",
    ));
    ("Configuration".to_string(), lines)
}

fn customization_settings_panel() -> (String, Vec<Line<'static>>) {
    let mut lines = vec![
        paragraph_line(
            "Settings live directly under the h5v table. Good top-level defaults are theme, compatibility behavior, preferred content mode order, and heatmap defaults.",
        ),
        paragraph_line(
            "These are best for opinions you want every launch to inherit before you make more targeted overrides.",
        ),
        Line::raw(""),
        section_title_line("Common settings"),
        paragraph_line("Useful values include h5v.theme, h5v.symbol_theme, h5v.compatibility, h5v.content_mode_order, and h5v.heatmap.* defaults."),
        Line::raw(""),
    ];
    lines.extend(highlighted_code_block(
        "lua",
        "lua",
        "h5v.theme = \"dark\"\nh5v.symbol_theme = \"rich\"\nh5v.compatibility = false\nh5v.content_mode_order = { \"preview\", \"heatmap\", \"matrix\" }\n\nh5v.heatmap.default_range = \"auto\"\nh5v.heatmap.default_colormap = \"inferno\"\nh5v.heatmap.default_normalization = \"sqrt\"\nh5v.heatmap.default_invert_x = false\nh5v.heatmap.default_invert_y = true\nh5v.heatmap.default_invert_c = false",
    ));
    lines.push(Line::raw(""));
    lines.push(paragraph_line(
        "Custom range presets can make heatmap work much faster when you revisit the same style of data:",
    ));
    lines.extend(highlighted_code_block(
        "lua",
        "lua",
        "h5v.heatmap.range_modes = {\n  { label = \"Clip 1-99%\", min = \"1%\", max = \"99%\" },\n  { label = \"Zero to 255\", min = 0, max = 255 },\n  { label = \"Noise floor\", min = 0, max = 20 },\n}\nh5v.heatmap.default_range = \"Clip 1-99%\"",
    ));
    ("Settings".to_string(), lines)
}

fn customization_colors_panel() -> (String, Vec<Line<'static>>) {
    let mut lines = vec![
        paragraph_line(
            "Color overrides live under h5v.colors. They are grouped by purpose, so you can change only the surfaces or accents you care about without replacing a full theme.",
        ),
        paragraph_line(
            "Good starting groups are accent, text, surface, tree, chart, status, toast, and content.",
        ),
        Line::raw(""),
    ];
    lines.extend(highlighted_code_block(
        "lua",
        "lua",
        "h5v.colors.surface.panel_border = \"#5f87ff\"\nh5v.colors.surface.title_bg = \"#1b1d2b\"\nh5v.colors.content.tab_active = \"#ffd75f\"\nh5v.colors.accent.selection_bg = \"#005f87\"\nh5v.colors.accent.selection_fg = \"#ffffff\"\nh5v.colors.status.update_available = \"#ffaf00\"",
    ));
    lines.push(Line::raw(""));
    lines.push(paragraph_line(
        "A common pattern is to keep the built-in theme and only tune a few accents for focus, selection, or status visibility.",
    ));
    ("Colors".to_string(), lines)
}

fn customization_symbols_panel() -> (String, Vec<Line<'static>>) {
    let mut lines = vec![
        paragraph_line(
            "Symbol overrides live under h5v.symbols and are grouped similarly to the built-in symbol themes. This is useful if you want richer icons in one area but ASCII-friendly symbols elsewhere.",
        ),
        paragraph_line(
            "When you need a more conservative baseline, set h5v.symbol_theme = \"compatibility\" first and then selectively add richer symbols back in.",
        ),
        Line::raw(""),
    ];
    lines.extend(highlighted_code_block(
        "lua",
        "lua",
        "h5v.symbol_theme = \"compatibility\"\nh5v.symbols.tree.root_file_icon = \"FILE \"\nh5v.symbols.tree.group_collapsed = \"> \"\nh5v.symbols.tree.group_expanded = \"v \"\nh5v.symbols.title.help = \" Help \"",
    ));
    lines.push(Line::raw(""));
    lines.push(paragraph_line(
        "Symbols are especially handy for tree readability and panel titles when you want the UI to better match your terminal font.",
    ));
    ("Symbols".to_string(), lines)
}

fn customization_keymaps_panel() -> (String, Vec<Line<'static>>) {
    let mut lines = vec![
        paragraph_line(
            "Keymaps are configured in Lua with helpers like bind, bind_command, bind_commands, bind_script, bind_lua, and unbind. Use h5v.modes.* and h5v.actions.* constants so LuaLS autocomplete can help you.",
        ),
        paragraph_line(
            "Use bind for built-in actions, bind_command for a single command, bind_commands or bind_script for repeatable command sequences, and bind_lua when you want a callback.",
        ),
        Line::raw(""),
        section_title_line("Examples"),
    ];
    lines.extend(highlighted_code_block(
        "lua",
        "lua",
        "bind(h5v.modes.Global, \"ctrl+h\", h5v.actions.ShowHelp, \"Show help\")\nunbind(h5v.modes.Heatmap, \"v\")\n\nbind_command(\n  h5v.modes.Heatmap,\n  \"ctrl+alt+r\",\n  \"heatmap range use \\\"Clip 1-99%\\\"\",\n  \"Use clipped range\"\n)\n\nbind_commands(\n  h5v.modes.Global,\n  \"ctrl+k\",\n  { \"down 2\", \"up 1\" },\n  \"Run a short command sequence\"\n)\n\nbind_script(\n  h5v.modes.Global,\n  \"ctrl+s\",\n  \"goto /group/data\\nmode heatmap\\nheatmap range use \\\"Clip 1-99%\\\"\",\n  \"Open a saved view\"\n)\n\nbind_lua(h5v.modes.Global, \"ctrl+l\", function(ctx)\n  ctx.command(\"help reload\")\nend, \"Reload help\")",
    ));
    ("Keymaps".to_string(), lines)
}

fn customization_scripting_panel() -> (String, Vec<Line<'static>>) {
    let mut lines = vec![
        paragraph_line(
            "Startup scripting is built on normal commands, so anything you can express in command mode can usually be scripted for repeatable workflows.",
        ),
        paragraph_line(
            "Use --command for a few one-offs, --script for reusable files, and --script-test when you want validation without launching the UI.",
        ),
        Line::raw(""),
        section_title_line("Script file"),
    ];
    lines.extend(highlighted_code_block(
        "sh",
        "terminal",
        "h5v data.h5 --script workflow.h5v\nh5v data.h5 --script-test < workflow.h5v",
    ));
    lines.push(Line::raw(""));
    lines.extend(highlighted_code_block(
        "sh",
        "script",
        "goto /experiments/run_04/image\nmode heatmap\nheatmap range use \"Clip 1-99%\"\nmchart add /experiments/run_04/signal[..,0]\npress ctrl+w o",
    ));
    lines.push(Line::raw(""));
    lines.push(paragraph_line(
        "The press command is useful when you want scripts to reuse existing keymaps instead of duplicating their behavior.",
    ));
    lines.push(Line::raw(""));
    lines.push(section_title_line("Mixing CLI and Lua"));
    lines.extend(highlighted_code_block(
        "sh",
        "terminal",
        "h5v data.h5 \\\n  --command 'goto /group/image' \\\n  --command 'mode heatmap' \\\n  --command 'heatmap range use \"Clip 1-99%\"'",
    ));
    lines.push(Line::raw(""));
    lines.push(paragraph_line(
        "Lua callbacks are a good fit when a script should stay attached to a keybinding and be shared across sessions.",
    ));
    lines.extend(highlighted_code_block(
        "lua",
        "lua",
        "bind_lua(h5v.modes.Global, \"ctrl+l\", function(ctx)\n  ctx.commands({\n    \"goto /group/image\",\n    \"mode heatmap\",\n    \"heatmap range use \\\"Clip 1-99%\\\"\",\n  })\nend, \"Open the default heatmap workflow\")",
    ));
    ("Scripting".to_string(), lines)
}

fn guide_text(sections: &[(&str, &[&str])]) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    for (idx, (title, paragraphs)) in sections.iter().enumerate() {
        lines.push(Line::from(Span::styled(
            title.to_string(),
            help_section_style(),
        )));
        for paragraph in *paragraphs {
            lines.push(Line::from(Span::styled(
                paragraph.to_string(),
                help_desc_style(),
            )));
        }
        if idx + 1 != sections.len() {
            lines.push(Line::raw(""));
        }
    }
    lines
}

fn section_title_line(title: &str) -> Line<'static> {
    Line::from(Span::styled(title.to_string(), help_section_style()))
}

fn paragraph_line(text: &str) -> Line<'static> {
    Line::from(Span::styled(text.to_string(), help_desc_style()))
}

fn highlighted_code_block(language: &str, badge: &str, source: &str) -> Vec<Line<'static>> {
    let mut rendered = Vec::new();
    rendered.push(Line::from(vec![
        Span::styled(format!(" {badge} "), help_code_badge_style()),
        Span::styled(
            format!("  {}", language_label(language)),
            help_muted_style(),
        ),
    ]));
    let mut code_lines = highlighted_lines(source, language)
        .unwrap_or_else(|| source.lines().map(code_fallback_line).collect::<Vec<_>>());
    for line in &mut code_lines {
        for span in &mut line.spans {
            span.style = span
                .style
                .bg(configure::themed_color(|colors| colors.surface.bg_val3));
        }
        if line.spans.is_empty() {
            line.spans
                .push(Span::styled("".to_string(), help_code_style()));
        }
    }
    rendered.extend(code_lines);
    rendered
}

fn code_fallback_line(code: &str) -> Line<'static> {
    Line::from(Span::styled(code.to_string(), help_code_style()))
}

fn language_label(language: &str) -> &'static str {
    match language {
        "lua" => "Lua",
        "sh" => "Shell",
        _ => "Code",
    }
}

fn help_key_style() -> Style {
    Style::default()
        .fg(configure::themed_color(|colors| colors.text.primary))
        .bg(configure::themed_color(|colors| colors.surface.help_key_bg))
        .underlined()
        .bold()
}

fn help_section_style() -> Style {
    Style::default()
        .fg(configure::themed_color(|colors| colors.help.section))
        .bold()
        .underlined()
}

fn help_desc_style() -> Style {
    Style::default().fg(configure::themed_color(|colors| colors.help.description))
}

fn help_muted_style() -> Style {
    Style::default().fg(configure::themed_color(|colors| colors.help.muted))
}

fn help_code_style() -> Style {
    Style::default()
        .fg(configure::themed_color(|colors| colors.text.primary))
        .bg(configure::themed_color(|colors| colors.surface.bg_val3))
}

fn help_code_badge_style() -> Style {
    Style::default()
        .fg(configure::themed_color(|colors| colors.accent.selection_fg))
        .bg(configure::themed_color(|colors| colors.accent.selection_bg))
        .bold()
}
