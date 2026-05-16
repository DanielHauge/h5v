use ratatui::{
    style::Style,
    symbols::border,
    text::{Line, Span},
};

use crate::{
    configure,
    ui::{
        command::{
            command_catalog, CommandArgKind, CommandArgSpec, CommandCategory, CommandDescriptor,
        },
        input::keymap::{
            AttributesAction, BoundAction, ContentAction, Direction, EffectiveKeymaps,
            GlobalAction, KeyBinding, MultiChartAction, NormalAction, TreeAction, WindowAction,
        },
        state::{
            HelpCommandSection, HelpCustomizationSection, HelpKeymapSection, HelpMultiChartSection,
        },
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
    let mut lines = vec![
        command_signature_line(descriptor),
        paragraph_line(descriptor.description),
    ];
    if !descriptor.aliases.is_empty() {
        lines.push(metadata_line("aliases", descriptor.aliases.join(", ")));
    }
    if !descriptor.keybindings.is_empty() {
        lines.push(metadata_line("keys", descriptor.keybindings.join(", ")));
    }
    for (index, arg) in descriptor.args.iter().enumerate() {
        lines.extend(command_arg_lines(arg, index));
    }
    lines.extend(command_example_block(descriptor));
    lines
}

fn command_signature_line(descriptor: &CommandDescriptor) -> Line<'static> {
    let mut spans = vec![Span::styled(
        descriptor.name.to_string(),
        help_function_name_style(),
    )];
    for (index, arg) in descriptor.args.iter().enumerate() {
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

fn command_arg_kind_label(kind: CommandArgKind) -> &'static str {
    match kind {
        CommandArgKind::UnsignedInt => "uint",
        CommandArgKind::Word => "word",
    }
}

fn metadata_line(label: &str, value: String) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("{label}: "), help_muted_style()),
        Span::styled(value, help_desc_style()),
    ])
}

fn command_arg_lines(arg: &CommandArgSpec, index: usize) -> Vec<Line<'static>> {
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

fn command_example_block(descriptor: &CommandDescriptor) -> Vec<Line<'static>> {
    framed_example_lines(Some("h5v"), vec![command_example_line(descriptor.example)])
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
            MultiChartAction::CycleViewMode => {
                "Cycle line, histogram, and comparison scatter modes".to_string()
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

pub(super) fn multichart_panel_text(
    section: HelpMultiChartSection,
) -> (String, Vec<Line<'static>>) {
    match section {
        HelpMultiChartSection::Overview => multichart_overview_panel(),
        HelpMultiChartSection::Expressions => multichart_expressions_panel(),
        HelpMultiChartSection::FunctionReducers => multichart_function_reducers_panel(),
        HelpMultiChartSection::FunctionMath => multichart_function_math_panel(),
        HelpMultiChartSection::FunctionTransforms => multichart_function_transforms_panel(),
    }
}

fn multichart_overview_panel() -> (String, Vec<Line<'static>>) {
    let mut lines = vec![
        paragraph_line("Multichart compares raw selections, derived series, and scalar values in one workspace."),
        paragraph_line("Open it with M. Add the current preview selection with m. Use Enter or n to create expressions."),
        paragraph_line("Use t / Tab to cycle line, histogram, and comparison scatter views; f / F fit the visible data; 0 / c resets the line viewport."),
        Line::raw(""),
        section_title_line("Quick flow"),
    ];
    lines.extend(highlighted_code_block(
        "expr",
        "flow",
        "1. Add raw series with m\n2. Reference them as $1, $2, $name\n3. Build derived series or scalars\n4. Switch views with t / Tab",
    ));
    lines.push(Line::raw(""));
    lines.push(paragraph_line("Use j/k to pick items, Space or v to hide/show them, and e to reopen the selected expression."));
    ("Multichart overview".to_string(), lines)
}

fn multichart_expressions_panel() -> (String, Vec<Line<'static>>) {
    let mut lines = vec![
        paragraph_line("Use $id or $name to reference items already loaded into multichart. Use load(...) to bring in datasets or attributes directly from the file."),
        paragraph_line("The editor accepts plain expressions, named derived series, scalar reducers, and transforms like interp(...) or slice(...)."),
        Line::raw(""),
        section_title_line("Editor examples"),
    ];
    lines.extend(expression_editor_example(
        "Reference an existing series",
        "raw-a",
        "$1",
        "Keep a raw source around under a readable name so later expressions can reference $raw-a instead of a numeric id.",
    ));
    lines.push(Line::raw(""));
    lines.extend(expression_editor_example(
        "Load a dataset as a series",
        "trace",
        "load(/signals/trace)",
        "load(/path) reads a one-dimensional dataset directly into multichart as a named series.",
    ));
    lines.push(Line::raw(""));
    lines.extend(expression_editor_example(
        "Slice a dataset while loading it",
        "first-column",
        "load(/matrix)[..,0]",
        "Selectors let you pick one series axis from higher-rank arrays. Here the expression reads column 0 across all rows.",
    ));
    lines.push(Line::raw(""));
    lines.extend(expression_editor_example(
        "Load an attribute and use it in math",
        "scaled",
        "$1 * load(/group/ds:SCALE) + load(/group/ds:BIAS)",
        "Attributes loaded with :ATTR_NAME behave like scalars, so they can scale or offset an existing series.",
    ));
    lines.push(Line::raw(""));
    lines.extend(expression_editor_example(
        "Slice or smooth an existing series",
        "focus-window",
        "rolling_mean(slice($1, 25.0, 250.0), 16)",
        "slice($item, start_x, end_x) narrows a series to an x-range; rolling helpers then build a new derived series from the windowed data.",
    ));
    lines.push(Line::raw(""));
    lines.extend(expression_editor_example(
        "Normalize a series by its own statistics",
        "normalized",
        "($1 - mean($1)) / stddev($1)",
        "Reducers return scalars, so they combine naturally with per-sample math to build normalized derived series.",
    ));
    lines.push(Line::raw(""));
    lines.push(paragraph_line("Tab switches between the name and expression fields while editing. Invalid expressions stay as drafts so they can be repaired instead of being discarded."));
    ("Expressions".to_string(), lines)
}

fn multichart_function_reducers_panel() -> (String, Vec<Line<'static>>) {
    let mut lines = vec![
        paragraph_line("Reducers collapse a whole series to one scalar value. They are useful for labels, normalization, thresholds, and scalar-only derived items."),
    ];
    for entry in [
        function_card(
            "avg",
            &[("series", "Series")],
            "scalar",
            "Alias of mean(...); returns the arithmetic mean of the series values.",
            &[("series", "The input series to reduce.")],
            "avg($1)",
        ),
        function_card(
            "mean",
            &[("series", "Series")],
            "scalar",
            "Returns the arithmetic mean of the series values.",
            &[("series", "The input series to reduce.")],
            "mean($1)",
        ),
        function_card(
            "min",
            &[("series", "Series")],
            "scalar",
            "Returns the minimum y-value in the series.",
            &[("series", "The input series to reduce.")],
            "min($1)",
        ),
        function_card(
            "max",
            &[("series", "Series")],
            "scalar",
            "Returns the maximum y-value in the series.",
            &[("series", "The input series to reduce.")],
            "max($1)",
        ),
        function_card(
            "stddev",
            &[("series", "Series")],
            "scalar",
            "Returns the standard deviation of the series values.",
            &[("series", "The input series to reduce.")],
            "stddev($1)",
        ),
        function_card(
            "len",
            &[("series", "Series")],
            "scalar",
            "Returns the number of samples in the series.",
            &[("series", "The input series to count.")],
            "len($1)",
        ),
        function_card(
            "max2",
            &[("lhs", "Scalar"), ("rhs", "Scalar")],
            "scalar",
            "Returns the larger of two scalar values.",
            &[
                ("lhs", "Left scalar value."),
                ("rhs", "Right scalar value."),
            ],
            "max2(mean($1), mean($2))",
        ),
        function_card(
            "min2",
            &[("lhs", "Scalar"), ("rhs", "Scalar")],
            "scalar",
            "Returns the smaller of two scalar values.",
            &[
                ("lhs", "Left scalar value."),
                ("rhs", "Right scalar value."),
            ],
            "min2(max($1), max($2))",
        ),
    ] {
        lines.extend(entry);
        lines.push(Line::raw(""));
    }
    ("Functions · reducers".to_string(), lines)
}

fn multichart_function_math_panel() -> (String, Vec<Line<'static>>) {
    let mut lines = vec![
        paragraph_line("These helpers preserve shape: series stay series, scalars stay scalars. Use them for cleanup, scaling, and nonlinear transforms."),
    ];
    for entry in [
        function_card(
            "abs",
            &[("value", "Scalar | Series")],
            "same shape",
            "Absolute value.",
            &[("value", "Scalar or series to transform.")],
            "abs($1)",
        ),
        function_card(
            "sqrt",
            &[("value", "Scalar | Series")],
            "same shape",
            "Square root.",
            &[("value", "Scalar or series to transform.")],
            "sqrt(abs($1))",
        ),
        function_card(
            "ln",
            &[("value", "Scalar | Series")],
            "same shape",
            "Natural logarithm.",
            &[("value", "Scalar or series to transform.")],
            "ln($1)",
        ),
        function_card(
            "log10",
            &[("value", "Scalar | Series")],
            "same shape",
            "Base-10 logarithm.",
            &[("value", "Scalar or series to transform.")],
            "log10($1)",
        ),
        function_card(
            "sin",
            &[("value", "Scalar | Series")],
            "same shape",
            "Sine.",
            &[("value", "Scalar or series to transform.")],
            "sin($1)",
        ),
        function_card(
            "cos",
            &[("value", "Scalar | Series")],
            "same shape",
            "Cosine.",
            &[("value", "Scalar or series to transform.")],
            "cos($1)",
        ),
        function_card(
            "tan",
            &[("value", "Scalar | Series")],
            "same shape",
            "Tangent.",
            &[("value", "Scalar or series to transform.")],
            "tan($1)",
        ),
        function_card(
            "floor",
            &[("value", "Scalar | Series")],
            "same shape",
            "Round toward negative infinity.",
            &[("value", "Scalar or series to transform.")],
            "floor($1)",
        ),
        function_card(
            "ceil",
            &[("value", "Scalar | Series")],
            "same shape",
            "Round toward positive infinity.",
            &[("value", "Scalar or series to transform.")],
            "ceil($1)",
        ),
        function_card(
            "round",
            &[("value", "Scalar | Series")],
            "same shape",
            "Round to the nearest integer value.",
            &[("value", "Scalar or series to transform.")],
            "round($1)",
        ),
        function_card(
            "exp",
            &[("base", "Scalar | Series"), ("power", "Scalar | Series")],
            "same shape",
            "Raises base to power element-wise.",
            &[
                ("base", "Base value or series."),
                ("power", "Exponent value or series."),
            ],
            "exp($1, 2)",
        ),
    ] {
        lines.extend(entry);
        lines.push(Line::raw(""));
    }
    ("Functions · math".to_string(), lines)
}

fn multichart_function_transforms_panel() -> (String, Vec<Line<'static>>) {
    let mut lines = vec![
        paragraph_line("Transforms build new series from existing ones. rolling_* helpers work anywhere; interp(...) and slice(...) must stay at the top level of the expression."),
    ];
    for entry in [
        function_card(
            "rolling_mean",
            &[("series", "Series"), ("window", "Scalar")],
            "series",
            "Sliding-window mean.",
            &[
                ("series", "Input series."),
                ("window", "Window size in samples."),
            ],
            "rolling_mean($1, 16)",
        ),
        function_card(
            "rolling_median",
            &[("series", "Series"), ("window", "Scalar")],
            "series",
            "Sliding-window median.",
            &[
                ("series", "Input series."),
                ("window", "Window size in samples."),
            ],
            "rolling_median($1, 16)",
        ),
        function_card(
            "rolling_stddev",
            &[("series", "Series"), ("window", "Scalar")],
            "series",
            "Sliding-window standard deviation.",
            &[
                ("series", "Input series."),
                ("window", "Window size in samples."),
            ],
            "rolling_stddev($1, 16)",
        ),
        function_card(
            "rolling_min",
            &[("series", "Series"), ("window", "Scalar")],
            "series",
            "Sliding-window minimum.",
            &[
                ("series", "Input series."),
                ("window", "Window size in samples."),
            ],
            "rolling_min($1, 16)",
        ),
        function_card(
            "rolling_max",
            &[("series", "Series"), ("window", "Scalar")],
            "series",
            "Sliding-window maximum.",
            &[
                ("series", "Input series."),
                ("window", "Window size in samples."),
            ],
            "rolling_max($1, 16)",
        ),
        function_card(
            "rolling_quantile",
            &[("series", "Series"), ("window", "Scalar"), ("q", "Scalar")],
            "series",
            "Sliding-window quantile.",
            &[
                ("series", "Input series."),
                ("window", "Window size in samples."),
                ("q", "Quantile from 0.0 to 1.0."),
            ],
            "rolling_quantile($1, 32, 0.95)",
        ),
        function_card(
            "threshold",
            &[("value", "Scalar | Series"), ("threshold", "Scalar")],
            "same shape",
            "Returns 1.0 where value >= threshold, otherwise 0.0.",
            &[
                ("value", "Scalar or series to test."),
                ("threshold", "Threshold value."),
            ],
            "threshold($1, 0.5)",
        ),
        function_card(
            "diff",
            &[("series", "Series")],
            "series",
            "Returns the first difference of a series.",
            &[("series", "Input series.")],
            "diff($1)",
        ),
        function_card(
            "interp",
            &[("series", "Series"), ("step", "Scalar")],
            "series",
            "Top-level transform that resamples a series to a fixed x-step.",
            &[
                ("series", "Direct chart item reference like $1."),
                ("step", "Target spacing between samples."),
            ],
            "interp($1, 0.05)",
        ),
        function_card(
            "slice",
            &[
                ("series", "Series"),
                ("start_x", "Scalar"),
                ("end_x", "Scalar"),
            ],
            "series",
            "Top-level transform that keeps only the requested x-range.",
            &[
                ("series", "Direct chart item reference like $1."),
                ("start_x", "Inclusive starting x value."),
                ("end_x", "Inclusive ending x value."),
            ],
            "slice($1, 25.5, 250.5)",
        ),
    ] {
        lines.extend(entry);
        lines.push(Line::raw(""));
    }
    ("Functions · transforms".to_string(), lines)
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

fn expression_editor_example(
    title: &str,
    name: &str,
    expression: &str,
    description: &str,
) -> Vec<Line<'static>> {
    let mut lines = vec![section_title_line(title)];
    lines.extend(multichart_prompt_example(7, name, expression, "prompt"));
    lines.push(Line::from(Span::styled(
        description.to_string(),
        help_muted_style(),
    )));
    lines
}

fn function_card(
    name: &str,
    args: &[(&str, &str)],
    returns: &str,
    description: &str,
    params: &[(&str, &str)],
    example: &str,
) -> Vec<Line<'static>> {
    let mut lines = vec![function_signature_line(name, args, returns)];
    lines.push(paragraph_line(description));
    for (index, (arg_name, arg_desc)) in params.iter().enumerate() {
        lines.push(Line::from(vec![
            Span::styled("  ", help_muted_style()),
            Span::styled(format!("{arg_name}: "), help_arg_style(index)),
            Span::styled(arg_desc.to_string(), help_muted_style()),
        ]));
    }
    lines.extend(multichart_prompt_example(
        7,
        &format!("{name}-demo"),
        example,
        "prompt",
    ));
    lines
}

fn function_signature_line(name: &str, args: &[(&str, &str)], returns: &str) -> Line<'static> {
    let mut spans = vec![
        Span::styled(name.to_string(), help_function_name_style()),
        Span::styled("(".to_string(), help_muted_style()),
    ];
    for (index, (arg_name, arg_kind)) in args.iter().enumerate() {
        if index > 0 {
            spans.push(Span::styled(", ".to_string(), help_muted_style()));
        }
        spans.push(Span::styled(arg_name.to_string(), help_arg_style(index)));
        spans.push(Span::styled(": ".to_string(), help_muted_style()));
        spans.push(Span::styled(arg_kind.to_string(), help_desc_style()));
    }
    spans.push(Span::styled(")".to_string(), help_muted_style()));
    spans.push(Span::styled(" -> ".to_string(), help_muted_style()));
    spans.push(Span::styled(returns.to_string(), help_return_style()));
    Line::from(spans)
}

fn highlighted_code_block(language: &str, title: &str, source: &str) -> Vec<Line<'static>> {
    let mut code_lines = highlighted_lines(source, language)
        .unwrap_or_else(|| source.lines().map(code_fallback_line).collect::<Vec<_>>());
    if code_lines.is_empty() {
        code_lines.push(code_fallback_line(""));
    }
    framed_example_lines(Some(title), code_lines)
}

fn code_fallback_line(code: &str) -> Line<'static> {
    Line::from(Span::styled(code.to_string(), help_code_style()))
}

fn framed_example_lines(
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

fn multichart_prompt_example(
    item_id: usize,
    name: &str,
    expression: &str,
    title: &str,
) -> Vec<Line<'static>> {
    let expression_line = highlighted_lines(expression, "expr")
        .and_then(|mut lines| {
            if lines.is_empty() {
                None
            } else {
                Some(lines.remove(0))
            }
        })
        .unwrap_or_else(|| code_fallback_line(expression));
    let mut line = Line::from(vec![
        Span::styled(
            format!("${item_id} "),
            Style::default()
                .fg(configure::themed_color(|colors| colors.toast.warning))
                .bold(),
        ),
        Span::styled(
            format!("${name}"),
            Style::default()
                .fg(configure::themed_color(|colors| colors.tree.dataset_file))
                .underlined(),
        ),
        Span::styled(
            " = ".to_string(),
            Style::default()
                .fg(configure::themed_color(|colors| {
                    colors.mchart.prompt_prefix
                }))
                .bold()
                .dim(),
        ),
    ]);
    line.spans.extend(expression_line.spans);
    framed_example_lines(Some(title), vec![line])
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

fn help_function_name_style() -> Style {
    Style::default()
        .fg(configure::themed_color(|colors| colors.help.section))
        .bold()
}

fn help_arg_style(index: usize) -> Style {
    Style::default().fg(configure::themed_color(|colors| {
        colors.chart.series[index % colors.chart.series.len()]
    }))
}

fn help_return_style() -> Style {
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
