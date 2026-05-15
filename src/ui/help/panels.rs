use ratatui::{
    style::Style,
    text::{Line, Span},
};

use crate::{
    configure,
    ui::{
        command::{command_catalog, command_usage, CommandCategory, CommandDescriptor},
        input::keymap::{
            AttributesAction, BoundAction, ContentAction, Direction, EffectiveKeymaps,
            GlobalAction, KeyBinding, MultiChartAction, NormalAction, TreeAction, WindowAction,
        },
        state::{HelpCommandSection, HelpCustomizationSection, HelpKeymapSection},
        std_comp_render::highlighted_lines,
    },
};

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

pub(super) fn multichart_help_lines() -> Vec<Line<'static>> {
    guide_text(&[
        (
            "Overview",
            &[
                "Multichart compares dataset selections and derived expressions in one plot.",
                "Open with M. Add the current previewable selection with m or :mchart add.",
            ],
        ),
        (
            "Workflow",
            &[
                "Add raw series first so you have stable $1, $2, and $3 references.",
                "Press Enter or n for a new expression, e to edit the selected one, Tab to switch between name and expression, and Esc to close.",
            ],
        ),
        (
            "Reference rules",
            &[
                "$id or $name refers to a chart item. Series items can be sliced, scalar items cannot.",
                "load(path) reads data and infers whether it stays a series or becomes a scalar.",
                "Use selectors after the call, for example load(/signals/sine_wave), load(/matrix)[..,0], or load(/group/ds:BIAS).",
                "Series helpers avg(x)/mean(x), min(x), max(x), stddev(x), and len(x) return scalars; abs/sqrt/ln/log10/sin/cos/tan/floor/ceil/round preserve scalar-vs-series shape; rolling_mean/median/stddev/min/max(x, window), rolling_quantile(x, window, q), threshold(x, cutoff), and diff(x) return derived series. interp($xy, step) and slice($item, x_min, x_max) are top-level transforms.",
                "(x_expr, y_expr) creates an explicit x/y plot.",
            ],
        ),
        (
            "Examples",
            &[
                "$1 - $2",
                "$temperature * load(/measurements/sensors/group1/temperature:scale)",
                "load(/signals/sine_wave) + load(/group_preview/offset)",
                "avg($1) / max2(load(/scalar), 1.0)",
                "mean($1) + stddev($1) + len($1)",
                "sqrt(abs($1 - 4)) + round(load(/scalar))",
                "rolling_mean($1, 16), threshold($1, 0.5), diff($1)",
                "interp($3, 0.05)",
                "slice($3, 25.5, 250.5)",
                "exp($1, load(/scalar))",
                "($1 * load(/group_preview:scale), load(/group_preview/time))",
                "(load(/signals/sine_wave), load(/signals/cosine_wave))",
            ],
        ),
        (
            "Viewport and items",
            &[
                "j/k selects, Space or v toggles visibility, d removes, and C clears all.",
                "f fits all visible series, F fits the selected series, and 0 or c resets the viewport.",
                "z / Z and + / - zoom, h / l pan, and the chart title shows the active viewport whenever zoom is active.",
                "Wheel zoom is anchored to the pointer; Ctrl-wheel zooms x only, and Shift-wheel zooms y only.",
                "Right-drag snapshots on press and pans on release.",
            ],
        ),
        (
            "Config",
            &[
                "Use h5v.multichart in Lua to tune large-series behavior.",
                "overview_max_samples sets the background overview cap, and detail_enabled toggles viewport refinement.",
                "detail_samples_per_column with detail_min_samples and detail_max_samples controls detail density.",
                "detail_padding_ratio loads extra x-range around the viewport, and derived_detail_enabled lets derived series refine from shared detail windows.",
            ],
        ),
    ])
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

pub(super) fn customization_panel_text(
    section: HelpCustomizationSection,
) -> (String, Vec<Line<'static>>) {
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

pub(super) fn paragraph_line(text: &str) -> Line<'static> {
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
