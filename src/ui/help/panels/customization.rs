use ratatui::text::{Line, Span};

use crate::{configure, ui::state::HelpCustomizationSection};

use super::{
    help_desc_style, help_muted_style, highlighted_code_block, paragraph_line, section_title_line,
};

pub(in crate::ui::help) fn customization_panel_text(
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
        "h5v",
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
            "Keymaps are configured in Lua with h5v.keys.bind({ ... }) and h5v.keys.unbind({ ... }). Use h5v.ids.keymap_modes.*, h5v.ids.commands.*, and h5v.actions.* constants so LuaLS autocomplete can help you.",
        ),
        paragraph_line(
            "Each binding declares one target: a command handle, built-in action, single command string, command list, script, or Lua callback.",
        ),
        Line::raw(""),
        section_title_line("Examples"),
    ];
    lines.extend(highlighted_code_block(
        "lua",
        "lua",
        "h5v.keys.bind({\n  mode = h5v.ids.keymap_modes.global,\n  key = \"ctrl+h\",\n  target = h5v.actions.ShowHelp,\n  description = \"Show help\",\n})\n\nh5v.keys.unbind({\n  mode = h5v.ids.keymap_modes.heatmap,\n  key = \"v\",\n})\n\nh5v.keys.bind({\n  mode = h5v.ids.keymap_modes.heatmap,\n  key = \"ctrl+alt+r\",\n  command = \"heatmap range use \\\"Clip 1-99%\\\"\",\n  description = \"Use clipped range\",\n})\n\nh5v.keys.bind({\n  mode = h5v.ids.keymap_modes.global,\n  key = \"ctrl+k\",\n  commands = { \"down 2\", \"up 1\" },\n  description = \"Run a short command sequence\",\n})\n\nh5v.keys.bind({\n  mode = h5v.ids.keymap_modes.global,\n  key = \"ctrl+l\",\n  lua = function(ctx)\n    ctx.command(\"help reload\")\n  end,\n  description = \"Reload help\",\n})",
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
        "shell",
        "h5v data.h5 --script workflow.h5v\nh5v data.h5 --script-test < workflow.h5v",
    ));
    lines.push(Line::raw(""));
    lines.extend(highlighted_code_block(
        "sh",
        "h5v",
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
        "shell",
        "h5v data.h5 \\\n  --command 'goto /group/image' \\\n  --command 'mode heatmap' \\\n  --command 'heatmap range use \"Clip 1-99%\"'",
    ));
    lines.push(Line::raw(""));
    lines.push(paragraph_line(
        "Lua callbacks are a good fit when a script should stay attached to a keybinding and be shared across sessions.",
    ));
    lines.extend(highlighted_code_block(
        "lua",
        "lua",
        "h5v.keys.bind({\n  mode = h5v.ids.keymap_modes.global,\n  key = \"ctrl+l\",\n  lua = function(ctx)\n    ctx.commands({\n      \"goto /group/image\",\n      \"mode heatmap\",\n      \"heatmap range use \\\"Clip 1-99%\\\"\",\n    })\n  end,\n  description = \"Open the default heatmap workflow\",\n})",
    ));
    ("Scripting".to_string(), lines)
}
