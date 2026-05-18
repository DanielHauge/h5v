use ratatui::{
    style::Style,
    symbols::border,
    text::{Line, Span},
};

use crate::{
    configure,
    ui::{
        command::{command_keybindings_metadata, command_usage_metadata},
        input::keymap::{
            AttributesAction, BoundAction, ContentAction, Direction, EffectiveKeymaps,
            GlobalAction, KeyBinding, MultiChartAction, NormalAction, TreeAction, WindowAction,
        },
        state::{HelpCommandSection, HelpKeymapSection},
        std_comp_render::highlighted_lines,
    },
};

mod customization;
mod health;
mod multichart;

pub(super) use customization::customization_panel_text;
pub(super) use health::health_panel_text;
pub(super) use multichart::multichart_panel_text;

pub(super) fn keymap_panel_text(
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

pub(super) fn command_panel_text(section: HelpCommandSection) -> (String, Vec<Line<'static>>) {
    let (title, category) = match section {
        HelpCommandSection::Navigation => ("Navigation commands", "Navigation"),
        HelpCommandSection::View => ("View commands", "View"),
        HelpCommandSection::Selection => ("Selection commands", "Selection"),
        HelpCommandSection::Attributes => ("Attribute commands", "Attributes"),
        HelpCommandSection::App => ("App commands", "App"),
        HelpCommandSection::MultiChart => ("Multichart commands", "MultiChart"),
        HelpCommandSection::Input => ("Input commands", "Input"),
    };

    let mut lines = vec![
        Line::from(Span::styled(
            "Commands are available from ':' and also power startup scripts and Lua helpers.",
            help_desc_style(),
        )),
        Line::raw(""),
    ];
    let snapshot = configure::current_registry_snapshot();
    let commands = snapshot
        .commands()
        .filter(|metadata| {
            metadata.visibility == configure::registry::CommandVisibility::Visible
                && metadata.category.eq_ignore_ascii_case(category)
        })
        .cloned()
        .collect::<Vec<_>>();
    for (idx, metadata) in commands.iter().enumerate() {
        lines.extend(command_metadata_lines(metadata));
        if idx + 1 != commands.len() {
            lines.push(Line::raw(""));
        }
    }
    (title.to_string(), lines)
}

fn command_metadata_lines(metadata: &configure::registry::CommandMetadata) -> Vec<Line<'static>> {
    let mut lines = vec![
        command_signature_line(metadata),
        paragraph_line(&metadata.summary),
    ];
    if !metadata.aliases.is_empty() {
        lines.push(metadata_line("aliases", metadata.aliases.join(", ")));
    }
    let keybindings = command_keybindings_metadata(metadata);
    if !keybindings.is_empty() {
        lines.push(metadata_line("keys", keybindings));
    }
    for (index, arg) in metadata.args.iter().enumerate() {
        lines.extend(command_arg_lines(arg, index));
    }
    lines.extend(command_example_block(metadata));
    lines
}

fn command_signature_line(metadata: &configure::registry::CommandMetadata) -> Line<'static> {
    let usage = command_usage_metadata(metadata);
    let mut parts = usage.split_whitespace();
    let mut spans = vec![Span::styled(
        parts.next().unwrap_or_default().to_string(),
        help_function_name_style(),
    )];
    for (index, arg) in metadata.args.iter().enumerate() {
        spans.push(Span::raw(" "));
        let open = if arg.required { "<" } else { "[" };
        let close = if arg.required { ">" } else { "]" };
        spans.push(Span::styled(open.to_string(), help_muted_style()));
        spans.push(Span::styled(arg.name.to_string(), help_arg_style(index)));
        spans.push(Span::styled(": ".to_string(), help_muted_style()));
        spans.push(Span::styled(
            command_arg_kind_label(arg.kind).to_string(),
            help_desc_style(),
        ));
        spans.push(Span::styled(close.to_string(), help_muted_style()));
    }
    Line::from(spans)
}

fn command_arg_kind_label(kind: configure::registry::CommandArgValueKind) -> &'static str {
    match kind {
        configure::registry::CommandArgValueKind::UnsignedInt => "uint",
        configure::registry::CommandArgValueKind::Word => "word",
    }
}

fn metadata_line(label: &str, value: String) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("{label}: "), help_muted_style()),
        Span::styled(value, help_desc_style()),
    ])
}

fn command_arg_lines(
    arg: &configure::registry::CommandArgMetadata,
    index: usize,
) -> Vec<Line<'static>> {
    let mut lines = vec![Line::from(vec![
        Span::styled("  ", help_muted_style()),
        Span::styled(format!("{}: ", arg.name), help_arg_style(index)),
        Span::styled(arg.help.to_string(), help_muted_style()),
    ])];
    if !arg.values.is_empty() {
        lines.push(Line::from(vec![
            Span::styled("    values: ", help_muted_style()),
            Span::styled(arg.values.join(" | "), help_desc_style()),
        ]));
    }
    lines
}

fn command_example_block(metadata: &configure::registry::CommandMetadata) -> Vec<Line<'static>> {
    if metadata.examples.is_empty() {
        Vec::new()
    } else {
        framed_example_lines(
            Some("h5v"),
            metadata
                .examples
                .iter()
                .map(|example| command_example_line(example))
                .collect(),
        )
    }
}

fn command_example_line(example: &str) -> Line<'static> {
    match example.split_once(' ') {
        Some((command, rest)) => Line::from(vec![
            Span::styled(command.to_string(), help_function_name_style()),
            Span::styled(" ".to_string(), help_code_style()),
            Span::styled(rest.to_string(), help_code_style()),
        ]),
        None => Line::from(Span::styled(
            example.to_string(),
            help_function_name_style(),
        )),
    }
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
                "Zoom in the active heatmap or preview chart".to_string()
            }
            ContentAction::HeatmapZoomOut => {
                "Zoom out the active heatmap or preview chart".to_string()
            }
            ContentAction::HeatmapResetView => {
                "Reset the heatmap or preview-chart viewport".to_string()
            }
            ContentAction::HeatmapClearSelection => "Clear the heatmap selection".to_string(),
            ContentAction::HeatmapPan(direction) => {
                format!(
                    "Pan the active heatmap or preview chart {}",
                    direction_label(*direction)
                )
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
            MultiChartAction::CycleViewMode => {
                "Cycle line, histogram, box plot, and comparison scatter modes".to_string()
            }
            MultiChartAction::ZoomIn => "Zoom in".to_string(),
            MultiChartAction::ZoomOut => "Zoom out".to_string(),
            MultiChartAction::PanLeft => "Pan left".to_string(),
            MultiChartAction::PanRight => "Pan right".to_string(),
            MultiChartAction::ClearZoom => "Reset zoom".to_string(),
            MultiChartAction::FitAll => "Fit the viewport to all visible series".to_string(),
            MultiChartAction::FitSelected => "Fit the viewport to the selected series".to_string(),
            MultiChartAction::DeleteSelected => "Remove the selected series".to_string(),
            MultiChartAction::ClearAll => "Remove all series".to_string(),
            MultiChartAction::ToggleSelectedVisible => {
                "Show or hide the selected series".to_string()
            }
            MultiChartAction::OpenExpressionPrompt => "Open the expression editor".to_string(),
            MultiChartAction::EditSelectedExpression => {
                "Edit the selected series in the expression editor".to_string()
            }
            MultiChartAction::MoveUp => "Select the previous series".to_string(),
            MultiChartAction::MoveDown => "Select the next series".to_string(),
            MultiChartAction::ReorderUp => {
                "Move the selected series earlier in the list".to_string()
            }
            MultiChartAction::ReorderDown => {
                "Move the selected series later in the list".to_string()
            }
        }
    })
}

fn describe_bound_action<T, S: Into<String>>(
    target: &BoundAction<T>,
    describe_action: impl Fn(&T) -> S,
) -> String {
    match target {
        BoundAction::Action(action) => describe_action(action).into(),
        BoundAction::Command(command) => crate::ui::command::command_metadata(command)
            .map(|metadata| format!("Run command: {}", command_usage_metadata(&metadata)))
            .unwrap_or_else(|| format!("Run command: {command}")),
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

pub(super) fn heatmap_help_lines() -> Vec<Line<'static>> {
    guide_text(&[
        (
            "Overview",
            &[
                "Heatmap shows numeric datasets as a rendered 2D slice with viewport, selection, legend, and histogram panels.",
                "Use Tab to switch into heatmap mode when the selected dataset supports it.",
            ],
        ),
        (
            "Selection rules",
            &[
                "No explicit selection means the active region is the current viewport.",
                "One left click selects one cell region, a second click expands that to a rectangle, and another click clears it.",
                "y copies the selection summary when a region is selected, or the viewport summary otherwise.",
            ],
        ),
        (
            "Viewport",
            &[
                "Wheel zoom is anchored to the hovered cell.",
                "Right click on an explicit selection zooms into that selection, and right-drag pans the viewport.",
                "z / Z zoom in and out, 0 resets the viewport, v clears the explicit selection, and H J K L pan by keyboard.",
                "PageUp and PageDown move through segmented heatmap pages.",
            ],
        ),
        (
            "Settings and ranges",
            &[
                "Up and Down move through settings. Left and Right change the selected value.",
                "Settings include colormap, range mode, invert x, invert y, invert colors, and normalization.",
                "Built-in range modes include Auto, MIN/MAX, Clip 1-99%, Sigma +-2sigma, and Winsor 2-98%.",
                "Use :heatmap range ... commands or h5v.heatmap.range_modes in Lua to add named custom ranges.",
            ],
        ),
    ])
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

pub(super) fn section_title_line(title: &str) -> Line<'static> {
    Line::from(Span::styled(title.to_string(), help_section_style()))
}

pub(super) fn paragraph_line(text: &str) -> Line<'static> {
    Line::from(Span::styled(text.to_string(), help_desc_style()))
}

pub(super) fn highlighted_code_block(
    language: &str,
    title: &str,
    source: &str,
) -> Vec<Line<'static>> {
    let mut code_lines = highlighted_lines(source, language)
        .unwrap_or_else(|| source.lines().map(code_fallback_line).collect::<Vec<_>>());
    if code_lines.is_empty() {
        code_lines.push(code_fallback_line(""));
    }
    framed_example_lines(Some(title), code_lines)
}

pub(super) fn code_fallback_line(code: &str) -> Line<'static> {
    Line::from(Span::styled(code.to_string(), help_code_style()))
}

pub(super) fn framed_example_lines(
    title: Option<&str>,
    mut content_lines: Vec<Line<'static>>,
) -> Vec<Line<'static>> {
    let content_width = content_lines.iter().map(Line::width).max().unwrap_or(0);
    let title_width = title
        .map(|title| title.chars().count().saturating_add(2))
        .unwrap_or(0);
    let inner_width = content_width.max(title_width);
    let mut rendered = Vec::with_capacity(content_lines.len().saturating_add(2));
    rendered.push(example_box_top_line(title, inner_width));
    for line in &mut content_lines {
        let current_width = line.width();
        let padding = inner_width.saturating_sub(current_width);
        for span in &mut line.spans {
            span.style = span
                .style
                .bg(configure::themed_color(|colors| colors.surface.bg_val3));
        }
        if line.spans.is_empty() {
            line.spans
                .push(Span::styled("".to_string(), help_code_style()));
        }
        let mut spans = Vec::with_capacity(line.spans.len().saturating_add(3));
        spans.push(Span::styled("│ ".to_string(), help_code_border_style()));
        spans.extend(line.spans.clone());
        if padding > 0 {
            spans.push(Span::styled(" ".repeat(padding), help_code_style()));
        }
        spans.push(Span::styled(" │".to_string(), help_code_border_style()));
        rendered.push(Line::from(spans));
    }
    rendered.push(example_box_bottom_line(inner_width));
    rendered
}

fn example_box_top_line(title: Option<&str>, inner_width: usize) -> Line<'static> {
    let set = border::ROUNDED;
    let total_width = inner_width.saturating_add(2);
    let Some(title) = title.filter(|title| !title.is_empty()) else {
        return Line::from(vec![
            Span::styled(set.top_left.to_string(), help_code_border_style()),
            Span::styled(
                set.horizontal_top.repeat(total_width),
                help_code_border_style(),
            ),
            Span::styled(set.top_right.to_string(), help_code_border_style()),
        ]);
    };
    let title_text = format!(" {title} ");
    let trailing_width = total_width.saturating_sub(title_text.chars().count());
    Line::from(vec![
        Span::styled(set.top_left.to_string(), help_code_border_style()),
        Span::styled(title_text, help_code_title_style(title)),
        Span::styled(
            set.horizontal_top.repeat(trailing_width),
            help_code_border_style(),
        ),
        Span::styled(set.top_right.to_string(), help_code_border_style()),
    ])
}

fn example_box_bottom_line(inner_width: usize) -> Line<'static> {
    let set = border::ROUNDED;
    Line::from(vec![
        Span::styled(set.bottom_left.to_string(), help_code_border_style()),
        Span::styled(
            set.horizontal_bottom.repeat(inner_width.saturating_add(2)),
            help_code_border_style(),
        ),
        Span::styled(set.bottom_right.to_string(), help_code_border_style()),
    ])
}

pub(super) fn help_key_style() -> Style {
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

pub(super) fn help_desc_style() -> Style {
    Style::default().fg(configure::themed_color(|colors| colors.help.description))
}

pub(super) fn help_muted_style() -> Style {
    Style::default().fg(configure::themed_color(|colors| colors.help.muted))
}

pub(super) fn help_code_style() -> Style {
    Style::default()
        .fg(configure::themed_color(|colors| colors.text.primary))
        .bg(configure::themed_color(|colors| colors.surface.bg_val3))
}

pub(super) fn help_function_name_style() -> Style {
    Style::default()
        .fg(configure::themed_color(|colors| colors.help.section))
        .bold()
}

pub(super) fn help_arg_style(index: usize) -> Style {
    Style::default().fg(configure::themed_color(|colors| {
        colors.chart.series[index % colors.chart.series.len()]
    }))
}

pub(super) fn help_return_style() -> Style {
    Style::default()
        .fg(configure::themed_color(|colors| colors.accent.selection_fg))
        .bg(configure::themed_color(|colors| colors.accent.selection_bg))
        .bold()
}

fn help_code_border_style() -> Style {
    Style::default()
        .fg(configure::themed_color(|colors| colors.help.muted))
        .bg(configure::themed_color(|colors| colors.surface.bg_val3))
        .dim()
}

fn help_code_title_style(title: &str) -> Style {
    let key = title.to_ascii_lowercase();
    let fg = match key.as_str() {
        "shell" => configure::themed_color(|colors| colors.toast.warning),
        "lua" => {
            configure::themed_color(|colors| colors.chart.series[2 % colors.chart.series.len()])
        }
        "h5v" => configure::themed_color(|colors| colors.mchart.prompt_prefix),
        "prompt" => configure::themed_color(|colors| colors.tree.dataset_file),
        _ => configure::themed_color(|colors| colors.help.muted),
    };
    Style::default()
        .fg(fg)
        .bg(configure::themed_color(|colors| colors.surface.bg_val3))
        .bold()
        .dim()
}
