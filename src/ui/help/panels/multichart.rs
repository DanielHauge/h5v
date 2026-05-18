use ratatui::{
    style::Style,
    text::{Line, Span},
};

use crate::{configure, ui::state::HelpMultiChartSection};

use super::{
    code_fallback_line, framed_example_lines, help_arg_style, help_desc_style,
    help_function_name_style, help_muted_style, help_return_style, highlighted_code_block,
    paragraph_line, section_title_line,
};

pub(in crate::ui::help) fn multichart_panel_text(
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
        paragraph_line("Use t / Tab to cycle line, histogram, box plot, and comparison scatter views; f / F fit the visible data; 0 / c resets the line viewport."),
        Line::raw(""),
        section_title_line("Quick flow"),
    ];
    lines.extend(highlighted_code_block(
        "expr",
        "flow",
        "1. Add raw series with m\n2. Reference them as $1, $2, $name\n3. Build derived series or scalars\n4. Switch views with t / Tab",
    ));
    lines.push(Line::raw(""));
    lines.push(paragraph_line("Use j/k to pick items, Alt+Up / Alt+Down to reorder them, Space or v to hide/show them, and e to reopen the selected expression."));
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
    multichart_function_panel(
        "Functions · reducers",
        "Reducers collapse a whole series to one scalar value. They are useful for labels, normalization, thresholds, and scalar-only derived items.",
        configure::registry::MchartFunctionCategory::Reducer,
    )
}

fn multichart_function_math_panel() -> (String, Vec<Line<'static>>) {
    multichart_function_panel(
        "Functions · math",
        "These helpers preserve shape: series stay series, scalars stay scalars. Use them for cleanup, scaling, and nonlinear transforms.",
        configure::registry::MchartFunctionCategory::Math,
    )
}

fn multichart_function_transforms_panel() -> (String, Vec<Line<'static>>) {
    multichart_function_panel(
        "Functions · transforms",
        "Transforms build new series from existing ones. rolling_* helpers work anywhere; interp(...) and slice(...) must stay at the top level of the expression.",
        configure::registry::MchartFunctionCategory::Transform,
    )
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

fn function_card(function: &configure::registry::MchartFunctionMetadata) -> Vec<Line<'static>> {
    let mut lines = vec![function_signature_line(function)];
    lines.push(paragraph_line(&function.summary));
    for (index, arg) in function.params.iter().enumerate() {
        lines.push(Line::from(vec![
            Span::styled("  ", help_muted_style()),
            Span::styled(format!("{}: ", arg.name), help_arg_style(index)),
            Span::styled(arg.detail.to_string(), help_muted_style()),
        ]));
    }
    lines.extend(multichart_prompt_example(
        7,
        &format!("{}-demo", function.name),
        &function.example,
        "prompt",
    ));
    lines
}

fn function_signature_line(
    function: &configure::registry::MchartFunctionMetadata,
) -> Line<'static> {
    let mut spans = vec![
        Span::styled(function.name.to_string(), help_function_name_style()),
        Span::styled("(".to_string(), help_muted_style()),
    ];
    for (index, arg) in function.params.iter().enumerate() {
        if index > 0 {
            spans.push(Span::styled(", ".to_string(), help_muted_style()));
        }
        spans.push(Span::styled(arg.name.to_string(), help_arg_style(index)));
        spans.push(Span::styled(": ".to_string(), help_muted_style()));
        spans.push(Span::styled(arg.kind_label.to_string(), help_desc_style()));
    }
    spans.push(Span::styled(")".to_string(), help_muted_style()));
    spans.push(Span::styled(" -> ".to_string(), help_muted_style()));
    spans.push(Span::styled(
        match function.return_kind {
            configure::registry::RegistryValueKind::Scalar => "scalar".to_string(),
            configure::registry::RegistryValueKind::Series => "series".to_string(),
            configure::registry::RegistryValueKind::Unknown => "unknown".to_string(),
            configure::registry::RegistryValueKind::Theme => "theme".to_string(),
            configure::registry::RegistryValueKind::SymbolTheme => "symbol-theme".to_string(),
            configure::registry::RegistryValueKind::Boolean => "boolean".to_string(),
            configure::registry::RegistryValueKind::Color => "color".to_string(),
            configure::registry::RegistryValueKind::Symbol => "symbol".to_string(),
            configure::registry::RegistryValueKind::ContentMode => "content-mode".to_string(),
            configure::registry::RegistryValueKind::String => "string".to_string(),
            configure::registry::RegistryValueKind::UnsignedInt => "uint".to_string(),
            configure::registry::RegistryValueKind::Float => "float".to_string(),
        },
        help_return_style(),
    ));
    Line::from(spans)
}

fn multichart_function_panel(
    title: &str,
    intro: &str,
    category: configure::registry::MchartFunctionCategory,
) -> (String, Vec<Line<'static>>) {
    let mut lines = vec![paragraph_line(intro)];
    let snapshot = configure::current_registry_snapshot();
    let functions = snapshot
        .mchart_functions()
        .filter(|function| function.category == category)
        .collect::<Vec<_>>();
    for (index, function) in functions.iter().enumerate() {
        lines.extend(function_card(function));
        if index + 1 != functions.len() {
            lines.push(Line::raw(""));
        }
    }
    (title.to_string(), lines)
}

fn multichart_prompt_example(
    item_id: usize,
    name: &str,
    expression: &str,
    title: &str,
) -> Vec<Line<'static>> {
    let expression_line = crate::ui::std_comp_render::highlighted_lines(expression, "expr")
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
