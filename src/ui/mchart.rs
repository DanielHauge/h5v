use hdf5_metno::{
    types::{FloatSize, IntSize, TypeDescriptor},
    Attribute, Dataset, File, Hyperslab, Selection, SliceOrIndex,
};
use image::{DynamicImage, ImageBuffer, Rgb};
use plotters::{
    prelude::{BitMapBackend, IntoDrawingArea},
    style::{Color as _, IntoFont, RGBColor, ShapeStyle},
};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Margin, Rect},
    style::{Color, Style, Stylize},
    text::{Line, Span, Text},
    widgets::{Block, BorderType, Borders, Paragraph, Wrap},
};
use ratatui_image::{picker::Picker, protocol::StatefulProtocol, StatefulImage};

use crate::{
    color_consts, compat,
    data::{
        validate_preview_selection_shape, DatasetPlotingData, PreviewSelection, SliceSelection,
    },
    error::log_error,
};

pub type Point = (f64, f64);

#[derive(Debug, Clone, PartialEq)]
struct ExpressionPromptState {
    buffer: String,
    cursor: usize,
    error: Option<String>,
}

impl ExpressionPromptState {
    fn new() -> Self {
        Self {
            buffer: String::new(),
            cursor: 0,
            error: None,
        }
    }
}

#[derive(Debug, Clone)]
struct ChartDragState {
    anchor_column: u16,
    viewport_from: usize,
    viewport_to: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChartXAxisPolicy {
    SampleIndex,
}

impl ChartXAxisPolicy {
    fn label(self) -> &'static str {
        match self {
            ChartXAxisPolicy::SampleIndex => "x values",
        }
    }

    fn description(self) -> &'static str {
        match self {
            ChartXAxisPolicy::SampleIndex => {
                "derived inputs align by sample index; each series is plotted with its own x values"
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ChartItemId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuiltinDerivedOp {
    Difference,
    Sum,
    Ratio,
    Product,
    Xy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExprBinaryOp {
    Add,
    Sub,
    Mul,
    Div,
}

#[derive(Debug, Clone, PartialEq)]
enum ExpressionAst {
    Number(f64),
    ItemRef(ChartItemId),
    SeriesRef(ExpressionSeriesRef),
    ScalarRef(ExpressionScalarRef),
    UnaryMinus(Box<ExpressionAst>),
    Binary {
        op: ExprBinaryOp,
        lhs: Box<ExpressionAst>,
        rhs: Box<ExpressionAst>,
    },
}

#[derive(Debug, Clone, PartialEq)]
enum ExpressionToken {
    ItemRef(ChartItemId),
    SeriesRef(ExpressionSeriesRef),
    ScalarRef(ExpressionScalarRef),
    Number(f64),
    Plus,
    Minus,
    Star,
    Slash,
    Comma,
    LParen,
    RParen,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DerivedExpressionKind {
    YSeries,
    XySeries,
}

#[derive(Debug, Clone, PartialEq)]
enum ParsedExpression {
    YSeries(ExpressionAst),
    XySeries(ExpressionAst, ExpressionAst),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum ExpressionDatasetSelector {
    All,
    Index(usize),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum ExpressionObjectTarget {
    AbsolutePath(String),
    ItemRef(ChartItemId),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct ExpressionSeriesRef {
    target: ExpressionObjectTarget,
    attr_name: Option<String>,
    selectors: Option<Vec<ExpressionDatasetSelector>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct ExpressionScalarRef {
    target: ExpressionObjectTarget,
    attr_name: Option<String>,
}

impl ExpressionObjectTarget {
    fn render(&self) -> String {
        match self {
            ExpressionObjectTarget::AbsolutePath(path) => path.clone(),
            ExpressionObjectTarget::ItemRef(id) => format!("${}", id.0),
        }
    }
}

impl ExpressionSeriesRef {
    fn render(&self) -> String {
        let base = match &self.attr_name {
            Some(attr_name) => format!("!{}:{attr_name}", self.target.render()),
            None => format!("!{}", self.target.render()),
        };
        match &self.selectors {
            None => base,
            Some(selectors) => {
                let selectors = selectors
                    .iter()
                    .map(|selector| match selector {
                        ExpressionDatasetSelector::All => "..".to_string(),
                        ExpressionDatasetSelector::Index(index) => index.to_string(),
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{base}[{selectors}]")
            }
        }
    }

    fn to_preview_selection(&self, shape: &[usize]) -> Result<PreviewSelection, String> {
        let reference = self.render();
        if shape.is_empty() {
            return Err(format!(
                "Series reference {reference} must point to a non-scalar array"
            ));
        }
        let (x, index) = match &self.selectors {
            None => {
                if shape.len() != 1 {
                    return Err(format!(
                        "Series reference {reference} needs an explicit selector like !/path[..,0] for rank-{} arrays",
                        shape.len()
                    ));
                }
                (0, vec![0])
            }
            Some(selectors) => {
                if selectors.len() != shape.len() {
                    return Err(format!(
                        "Dataset reference {} must provide exactly {} selectors",
                        self.render(),
                        shape.len()
                    ));
                }
                let mut x = None;
                let mut index = vec![0; shape.len()];
                for (dim, selector) in selectors.iter().enumerate() {
                    match selector {
                        ExpressionDatasetSelector::All => {
                            if x.replace(dim).is_some() {
                                return Err(format!(
                                    "Dataset reference {} must contain exactly one '..' axis selector",
                                    self.render()
                                ));
                            }
                        }
                        ExpressionDatasetSelector::Index(selected) => {
                            if *selected >= shape[dim] {
                                return Err(format!(
                                    "Dataset reference {} selects index {} out of bounds for dim {} with length {}",
                                    self.render(),
                                    selected,
                                    dim,
                                    shape[dim]
                                ));
                            }
                            index[dim] = *selected;
                        }
                    }
                }
                (
                    x.ok_or_else(|| {
                        format!(
                            "Series reference {reference} must contain exactly one '..' axis selector"
                        )
                    })?,
                    index,
                )
            }
        };

        Ok(PreviewSelection {
            x,
            index,
            slice: SliceSelection::All,
        })
    }
}

impl ExpressionScalarRef {
    fn render(&self) -> String {
        match &self.attr_name {
            Some(attr_name) => format!("#{}:{attr_name}", self.target.render()),
            None => format!("#{}", self.target.render()),
        }
    }
}

impl BuiltinDerivedOp {
    pub fn label(self) -> &'static str {
        match self {
            BuiltinDerivedOp::Difference => "difference",
            BuiltinDerivedOp::Sum => "sum",
            BuiltinDerivedOp::Ratio => "ratio",
            BuiltinDerivedOp::Product => "product",
            BuiltinDerivedOp::Xy => "x/y pair",
        }
    }

    pub fn symbol(self) -> &'static str {
        match self {
            BuiltinDerivedOp::Difference => "-",
            BuiltinDerivedOp::Sum => "+",
            BuiltinDerivedOp::Ratio => "/",
            BuiltinDerivedOp::Product => "*",
            BuiltinDerivedOp::Xy => "=>",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DatasetChartKind {
    Dataset,
    CompoundLeaf,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DatasetChartSource {
    pub dataset_path: String,
    pub display_path: String,
    pub selection: PreviewSelection,
    pub shape: Vec<usize>,
    pub kind: DatasetChartKind,
}

impl DatasetChartSource {
    pub fn matches_path(&self, path: &str) -> bool {
        self.display_path == path || self.dataset_path == path
    }

    pub fn kind_label(&self) -> &'static str {
        match self.kind {
            DatasetChartKind::Dataset => "dataset",
            DatasetChartKind::CompoundLeaf => "compound leaf",
        }
    }

    pub fn shape_summary(&self) -> String {
        if self.shape.is_empty() {
            "scalar".to_string()
        } else {
            self.shape
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(" x ")
        }
    }

    pub fn fixed_index_summary(&self) -> String {
        let parts = self
            .selection
            .index
            .iter()
            .enumerate()
            .filter(|(dim, _)| *dim != self.selection.x)
            .map(|(dim, index)| format!("d{dim}={index}"))
            .collect::<Vec<_>>();
        if parts.is_empty() {
            "none".to_string()
        } else {
            parts.join(", ")
        }
    }

    pub fn selection_summary(&self) -> String {
        format!(
            "x=d{} | fixed {} | {}",
            self.selection.x,
            self.fixed_index_summary(),
            self.slice_summary()
        )
    }

    pub fn compact_selection_summary(&self) -> String {
        format!("x=d{} | {}", self.selection.x, self.fixed_index_summary())
    }

    pub fn slice_summary(&self) -> String {
        match self.selection.slice {
            SliceSelection::All => "slice=all".to_string(),
            SliceSelection::FromTo(start, end) => format!("slice={start}..{end}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct BuiltinDerivedSource {
    pub operation: BuiltinDerivedOp,
    pub lhs_id: ChartItemId,
    pub rhs_id: ChartItemId,
    pub lhs_label: String,
    pub rhs_label: String,
    pub lhs_view: String,
    pub rhs_view: String,
    pub aligned_len: usize,
    pub lhs_len: usize,
    pub rhs_len: usize,
}

impl BuiltinDerivedSource {
    fn expression(&self) -> String {
        match self.operation {
            BuiltinDerivedOp::Xy => format!("x:{} | y:{}", self.lhs_label, self.rhs_label),
            _ => format!(
                "{} {} {}",
                self.lhs_label,
                self.operation.symbol(),
                self.rhs_label
            ),
        }
    }

    fn alignment_summary(&self) -> String {
        if self.operation == BuiltinDerivedOp::Xy {
            return format!("paired len {}", self.aligned_len);
        }
        if self.lhs_len == self.rhs_len {
            format!("aligned len {}", self.aligned_len)
        } else {
            format!(
                "aligned len {} (from {} vs {})",
                self.aligned_len, self.lhs_len, self.rhs_len
            )
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ChartSource {
    DatasetSelection(DatasetChartSource),
    BuiltinDerived(BuiltinDerivedSource),
    #[allow(dead_code)]
    DerivedExpression {
        expression: String,
        input_ids: Vec<ChartItemId>,
        len: usize,
        kind: DerivedExpressionKind,
    },
}

impl ChartSource {
    pub fn matches_path(&self, path: &str) -> bool {
        match self {
            ChartSource::DatasetSelection(source) => source.matches_path(path),
            ChartSource::BuiltinDerived(_) => false,
            ChartSource::DerivedExpression { .. } => false,
        }
    }

    fn label(&self) -> String {
        match self {
            ChartSource::DatasetSelection(source) => source.display_path.clone(),
            ChartSource::BuiltinDerived(source) => source.expression(),
            ChartSource::DerivedExpression { expression, .. } => expression.clone(),
        }
    }

    fn source_kind_label(&self) -> &'static str {
        match self {
            ChartSource::DatasetSelection(source) => source.kind_label(),
            ChartSource::BuiltinDerived(source) => source.operation.label(),
            ChartSource::DerivedExpression { kind, .. } => match kind {
                DerivedExpressionKind::YSeries => "expression",
                DerivedExpressionKind::XySeries => "expression x/y",
            },
        }
    }

    fn dataset_source(&self) -> Option<&DatasetChartSource> {
        match self {
            ChartSource::DatasetSelection(source) => Some(source),
            ChartSource::BuiltinDerived(_) | ChartSource::DerivedExpression { .. } => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ChartSeries {
    points: Vec<Point>,
    y_max: f64,
    y_min: f64,
    sample_max: usize,
    sample_min: usize,
}

impl ChartSeries {
    fn from_points(points: Vec<Point>) -> Option<Self> {
        if points.is_empty() {
            return None;
        }
        let y_max = points.iter().map(|(_, y)| *y).fold(f64::MIN, f64::max);
        let y_min = points.iter().map(|(_, y)| *y).fold(f64::MAX, f64::min);
        let points_len = points.len();
        Some(Self {
            points,
            y_max,
            y_min,
            sample_min: 0,
            sample_max: points_len,
        })
    }

    pub fn len(&self) -> usize {
        self.points.len()
    }
}

#[derive(Debug, Clone)]
pub struct ChartItem {
    pub id: ChartItemId,
    pub color_slot: usize,
    pub label: String,
    pub source: ChartSource,
    pub series: ChartSeries,
    pub visible: bool,
}

impl ChartItem {
    pub fn matches_path(&self, path: &str) -> bool {
        self.source.matches_path(path)
    }

    pub fn rgb_color(&self) -> (u8, u8, u8) {
        color_consts::rgb_channels(color_consts::chart_series_color(self.color_slot))
    }

    pub fn reference_label(&self) -> String {
        format!("${} {}", self.id.0, self.list_label())
    }

    pub fn list_label(&self) -> String {
        match &self.source {
            ChartSource::DatasetSelection(source) => {
                format!("{} [{}]", self.label, source.compact_selection_summary())
            }
            ChartSource::BuiltinDerived(source) => {
                format!("{} [{}]", source.expression(), source.alignment_summary())
            }
            ChartSource::DerivedExpression { expression, .. } => expression.clone(),
        }
    }

    pub fn stats_summary(&self) -> String {
        format!(
            "len {} | y [{:.4}, {:.4}]",
            self.series.len(),
            self.series.y_min,
            self.series.y_max
        )
    }
}

pub struct MultiChartState {
    items: Vec<ChartItem>,
    pub modified: bool,
    pub height: u32,
    pub width: u32,
    pub plot_buffer: Vec<u8>,
    pub picker: Picker,
    pub idx: usize,
    pub aoi_from: Option<usize>,
    pub aoi_to: Option<usize>,
    stateful_protocol: Option<StatefulProtocol>,
    next_id: u64,
    next_color_slot: usize,
    x_axis_policy: ChartXAxisPolicy,
    marked_base_item: Option<ChartItemId>,
    expression_prompt: Option<ExpressionPromptState>,
    last_chart_area: Option<Rect>,
    drag_state: Option<ChartDragState>,
}

impl MultiChartState {
    pub fn new(picker: Picker) -> Self {
        Self {
            items: Vec::new(),
            modified: false,
            idx: 0,
            height: 0,
            width: 0,
            plot_buffer: Vec::new(),
            picker,
            aoi_from: None,
            aoi_to: None,
            stateful_protocol: None,
            next_id: 1,
            next_color_slot: 0,
            x_axis_policy: ChartXAxisPolicy::SampleIndex,
            marked_base_item: None,
            expression_prompt: None,
            last_chart_area: None,
            drag_state: None,
        }
    }

    pub fn chart_items(&self) -> &[ChartItem] {
        &self.items
    }

    #[cfg(test)]
    pub fn source_item_count(&self, path: &str) -> usize {
        self.items
            .iter()
            .filter(|item| item.matches_path(path))
            .count()
    }

    pub fn visible_item_count(&self) -> usize {
        self.items.iter().filter(|item| item.visible).count()
    }

    pub fn selected_item(&self) -> Option<&ChartItem> {
        self.items.get(self.idx)
    }

    pub fn is_expression_prompt_active(&self) -> bool {
        self.expression_prompt.is_some()
    }

    fn item_by_id(&self, id: ChartItemId) -> Option<&ChartItem> {
        self.items.iter().find(|item| item.id == id)
    }

    fn marked_base_item_ref(&self) -> Option<&ChartItem> {
        self.marked_base_item.and_then(|id| self.item_by_id(id))
    }

    pub fn open_expression_prompt(&mut self) {
        self.expression_prompt = Some(ExpressionPromptState::new());
        self.modified = true;
    }

    pub fn close_expression_prompt(&mut self) {
        self.expression_prompt = None;
        self.modified = true;
    }

    pub fn expression_insert_char(&mut self, ch: char) {
        if let Some(prompt) = self.expression_prompt.as_mut() {
            prompt.buffer.insert(prompt.cursor, ch);
            prompt.cursor += 1;
            prompt.error = None;
        }
    }

    pub fn expression_backspace(&mut self) {
        if let Some(prompt) = self.expression_prompt.as_mut() {
            if prompt.cursor > 0 {
                prompt.cursor -= 1;
                prompt.buffer.remove(prompt.cursor);
                prompt.error = None;
            }
        }
    }

    pub fn expression_delete(&mut self) {
        if let Some(prompt) = self.expression_prompt.as_mut() {
            if prompt.cursor < prompt.buffer.len() {
                prompt.buffer.remove(prompt.cursor);
                prompt.error = None;
            }
        }
    }

    pub fn expression_move_left(&mut self) {
        if let Some(prompt) = self.expression_prompt.as_mut() {
            if prompt.cursor > 0 {
                prompt.cursor -= 1;
            }
        }
    }

    pub fn expression_move_right(&mut self) {
        if let Some(prompt) = self.expression_prompt.as_mut() {
            if prompt.cursor < prompt.buffer.len() {
                prompt.cursor += 1;
            }
        }
    }

    pub fn expression_move_to_start(&mut self) {
        if let Some(prompt) = self.expression_prompt.as_mut() {
            prompt.cursor = 0;
        }
    }

    pub fn expression_move_to_end(&mut self) {
        if let Some(prompt) = self.expression_prompt.as_mut() {
            prompt.cursor = prompt.buffer.len();
        }
    }

    pub fn expression_clear(&mut self) {
        if let Some(prompt) = self.expression_prompt.as_mut() {
            prompt.buffer.clear();
            prompt.cursor = 0;
            prompt.error = None;
        }
    }

    pub fn submit_expression_prompt(&mut self, file: Option<&File>) -> Result<(), String> {
        let expression = self
            .expression_prompt
            .as_ref()
            .map(|prompt| prompt.buffer.trim().to_string())
            .ok_or_else(|| "Expression prompt is not active".to_string())?;
        if expression.is_empty() {
            self.set_expression_error("Enter an expression before submitting".to_string());
            return Ok(());
        }

        match self.create_expression_derived_with_file(expression.clone(), file) {
            Ok(_) => {
                self.close_expression_prompt();
                Ok(())
            }
            Err(error) => {
                self.set_expression_error(error);
                Ok(())
            }
        }
    }

    fn set_expression_error(&mut self, error: String) {
        if let Some(prompt) = self.expression_prompt.as_mut() {
            prompt.error = Some(error);
        }
    }

    #[cfg(test)]
    fn create_expression_derived(&mut self, expression: String) -> Result<ChartItemId, String> {
        self.create_expression_derived_with_file(expression, None)
    }

    pub fn evaluate_expression_preview(
        &self,
        expression: &str,
        file: Option<&File>,
    ) -> Result<DatasetPlotingData, String> {
        let evaluated = self.evaluate_expression_with_file(expression, file)?;
        dataset_ploting_data_from_points(evaluated.points)
    }

    pub fn capture_expression_chart_item(
        &self,
        expression: &str,
        file: Option<&File>,
    ) -> Result<(ChartSource, Vec<Point>), String> {
        let evaluated = self.evaluate_expression_with_file(expression, file)?;
        let len = evaluated.points.len();
        let source = ChartSource::DerivedExpression {
            expression: expression.to_string(),
            input_ids: evaluated.input_ids,
            len,
            kind: evaluated.kind,
        };
        Ok((source, evaluated.points))
    }

    fn create_expression_derived_with_file(
        &mut self,
        expression: String,
        file: Option<&File>,
    ) -> Result<ChartItemId, String> {
        let evaluated = self.evaluate_expression_with_file(&expression, file)?;
        let len = evaluated.points.len();
        let source = ChartSource::DerivedExpression {
            expression,
            input_ids: evaluated.input_ids,
            len,
            kind: evaluated.kind,
        };
        self.add_chart_item(source, evaluated.points)
            .ok_or_else(|| "Failed to create expression-derived chart".to_string())
    }

    fn evaluate_expression_with_file(
        &self,
        expression: &str,
        file: Option<&File>,
    ) -> Result<EvaluatedExpression, String> {
        let tokens = tokenize_expression(expression)?;
        let parsed = parse_derived_expression(&tokens)?;
        let mut refs = ExpressionRefs::default();
        collect_parsed_expression_refs(&parsed, &mut refs);
        refs.item_ids.sort_by_key(|id| id.0);
        refs.item_ids.dedup();
        refs.series_refs
            .sort_by_key(|series_ref| series_ref.render());
        refs.series_refs.dedup();
        refs.scalar_refs
            .sort_by_key(|scalar_ref| scalar_ref.render());
        refs.scalar_refs.dedup();
        if refs.item_ids.is_empty() && refs.series_refs.is_empty() {
            return Err(
                "Expression must reference at least one series such as $3, !/group/ds[..,0], or !$3:ATTR"
                    .to_string(),
            );
        }

        let referenced = refs
            .item_ids
            .iter()
            .map(|id| {
                self.item_by_id(*id)
                    .cloned()
                    .ok_or_else(|| format!("Unknown chart item reference ${}", id.0))
            })
            .collect::<Result<Vec<_>, _>>()?;

        let external_series = resolve_expression_series_values(self, file, &refs.series_refs)?;
        let mut series_inputs = referenced
            .iter()
            .map(|item| ExpressionSeriesInput {
                label: format!("${}", item.id.0),
                points: item.series.points.clone(),
            })
            .collect::<Vec<_>>();
        for series_ref in &refs.series_refs {
            let points = external_series.get(series_ref).cloned().ok_or_else(|| {
                format!("Series reference {} was not resolved", series_ref.render())
            })?;
            series_inputs.push(ExpressionSeriesInput {
                label: series_ref.render(),
                points,
            });
        }

        let first = series_inputs.first().ok_or_else(|| {
            "Expression must reference at least one chart item or dataset".to_string()
        })?;
        let expected_len = first.points.len();
        if expected_len == 0 {
            return Err("Cannot build an expression from empty series".to_string());
        }
        let require_matching_x = matches!(parsed, ParsedExpression::YSeries(_));
        validate_expression_series_compatibility(&series_inputs, expected_len, require_matching_x)?;

        let scalar_values = resolve_expression_scalar_values(self, file, &refs.scalar_refs)?;

        let mut points = Vec::with_capacity(expected_len);
        let kind = match &parsed {
            ParsedExpression::YSeries(ast) => {
                for idx in 0..expected_len {
                    let y = eval_expression_at(ast, idx, self, &external_series, &scalar_values)?;
                    points.push((first.points[idx].0, y));
                }
                DerivedExpressionKind::YSeries
            }
            ParsedExpression::XySeries(x_ast, y_ast) => {
                for idx in 0..expected_len {
                    let x = eval_expression_at(x_ast, idx, self, &external_series, &scalar_values)?;
                    let y = eval_expression_at(y_ast, idx, self, &external_series, &scalar_values)?;
                    points.push((x, y));
                }
                DerivedExpressionKind::XySeries
            }
        };

        Ok(EvaluatedExpression {
            points,
            kind,
            input_ids: refs.item_ids,
        })
    }

    pub fn add_chart_item(
        &mut self,
        source: ChartSource,
        points: Vec<Point>,
    ) -> Option<ChartItemId> {
        let series = ChartSeries::from_points(points)?;
        if let Some((idx, item)) = self
            .items
            .iter_mut()
            .enumerate()
            .find(|(_, item)| item.source == source)
        {
            item.series = series;
            item.visible = true;
            self.idx = idx;
            self.modified = true;
            return Some(item.id);
        }

        let id = ChartItemId(self.next_id);
        self.next_id += 1;
        let color_slot = self.next_color_slot;
        self.next_color_slot += 1;
        self.items.push(ChartItem {
            id,
            color_slot,
            label: source.label(),
            source,
            series,
            visible: true,
        });
        self.idx = self.items.len().saturating_sub(1);
        self.modified = true;
        Some(id)
    }

    pub fn toggle_marked_base(&mut self) -> Result<String, String> {
        let Some(selected) = self.selected_item() else {
            return Err("No chart item selected".to_string());
        };
        let selected_id = selected.id;
        let selected_label = selected.label.clone();
        if self.marked_base_item == Some(selected_id) {
            self.marked_base_item = None;
            self.modified = true;
            return Ok(format!("Cleared marked base series '{}'", selected_label));
        }
        self.marked_base_item = Some(selected_id);
        self.modified = true;
        Ok(format!(
            "Marked '{}' as the base series for derived comparisons",
            selected_label
        ))
    }

    pub fn create_builtin_derived(
        &mut self,
        operation: BuiltinDerivedOp,
    ) -> Result<ChartItemId, String> {
        let Some(base_id) = self.marked_base_item else {
            return Err("Mark a base series with Space first".to_string());
        };
        let selected = self
            .selected_item()
            .cloned()
            .ok_or_else(|| "No chart item selected".to_string())?;
        if selected.id == base_id {
            return Err(
                "Select a second chart item before creating a derived comparison".to_string(),
            );
        }
        let base = self
            .item_by_id(base_id)
            .cloned()
            .ok_or_else(|| "The marked base series no longer exists".to_string())?;

        let aligned_len = match operation {
            BuiltinDerivedOp::Xy => {
                if base.series.len() != selected.series.len() {
                    return Err(format!(
                        "Cannot create x/y series: base length {} does not match selected length {}",
                        base.series.len(),
                        selected.series.len()
                    ));
                }
                base.series.len()
            }
            _ => base.series.len().min(selected.series.len()),
        };
        if aligned_len == 0 {
            return Err("Cannot derive a chart from empty input series".to_string());
        }

        let mut points = Vec::with_capacity(aligned_len);
        for idx in 0..aligned_len {
            let lhs = base.series.points[idx].1;
            let rhs = selected.series.points[idx].1;
            let point = match operation {
                BuiltinDerivedOp::Difference => (idx as f64, lhs - rhs),
                BuiltinDerivedOp::Sum => (idx as f64, lhs + rhs),
                BuiltinDerivedOp::Product => (idx as f64, lhs * rhs),
                BuiltinDerivedOp::Ratio => {
                    if rhs == 0.0 {
                        return Err(format!(
                            "Cannot compute ratio: divisor is zero at aligned sample index {}",
                            idx
                        ));
                    }
                    (idx as f64, lhs / rhs)
                }
                BuiltinDerivedOp::Xy => (lhs, rhs),
            };
            points.push(point);
        }

        let source = ChartSource::BuiltinDerived(BuiltinDerivedSource {
            operation,
            lhs_id: base.id,
            rhs_id: selected.id,
            lhs_label: base.label.clone(),
            rhs_label: selected.label.clone(),
            lhs_view: base.list_label(),
            rhs_view: selected.list_label(),
            aligned_len,
            lhs_len: base.series.len(),
            rhs_len: selected.series.len(),
        });
        self.add_chart_item(source, points)
            .ok_or_else(|| "Failed to create derived chart".to_string())
    }

    pub fn move_up(&mut self) {
        self.idx = self.idx.saturating_sub(1);
    }

    pub fn move_down(&mut self) {
        self.idx = self
            .idx
            .saturating_add(1)
            .clamp(0, self.items.len().saturating_sub(1));
    }

    pub fn toggle_selected_visible(&mut self) {
        if let Some(item) = self.items.get_mut(self.idx) {
            item.visible = !item.visible;
            self.modified = true;
        }
    }

    pub fn set_selected_visible(&mut self, visible: bool) {
        if let Some(item) = self.items.get_mut(self.idx) {
            if item.visible != visible {
                item.visible = visible;
                self.modified = true;
            }
        }
    }

    pub fn clear_selected(&mut self) {
        if self.idx < self.items.len() {
            let removed = self.items.remove(self.idx);
            if self.marked_base_item == Some(removed.id) {
                self.marked_base_item = None;
            }
            self.idx = self.idx.clamp(0, self.items.len().saturating_sub(1));
            if self.items.is_empty() {
                self.clear_zoom();
                self.stateful_protocol = None;
            }
            self.modified = true;
        }
    }

    pub fn clear_all(&mut self) {
        self.items.clear();
        self.idx = 0;
        self.clear_zoom();
        self.stateful_protocol = None;
        self.marked_base_item = None;
        self.modified = true;
    }

    pub fn clear_marked_base(&mut self) {
        if self.marked_base_item.take().is_some() {
            self.modified = true;
        }
    }

    pub fn create_expression_derived_command(
        &mut self,
        expression: String,
        file: Option<&File>,
    ) -> Result<ChartItemId, String> {
        self.create_expression_derived_with_file(expression, file)
    }

    pub fn add_dataset_reference_command(
        &mut self,
        dataset_spec: &str,
        file: Option<&File>,
    ) -> Result<ChartItemId, String> {
        let normalized = dataset_spec.trim();
        if normalized.is_empty() {
            return Err("Dataset reference cannot be empty".to_string());
        }
        let prefixed = if normalized.starts_with('!') {
            normalized.to_string()
        } else {
            format!("!{normalized}")
        };
        let tokens = tokenize_expression(&prefixed)?;
        let Some(ExpressionToken::SeriesRef(series_ref)) = tokens.first() else {
            return Err(format!(
                "Dataset reference '{}' must look like !/path or !/path[..,0]",
                dataset_spec
            ));
        };
        if tokens.len() != 1 {
            return Err(format!(
                "Dataset reference '{}' must contain only a single dataset selector",
                dataset_spec
            ));
        }
        let file = file.ok_or_else(|| {
            "Adding a dataset by path requires an open file handle, but no file is loaded"
                .to_string()
        })?;
        if !matches!(series_ref.target, ExpressionObjectTarget::AbsolutePath(_))
            || series_ref.attr_name.is_some()
        {
            return Err(format!(
                "Dataset reference '{}' must look like !/path or !/path[..,0]",
                dataset_spec
            ));
        }
        let ExpressionObjectTarget::AbsolutePath(path) = &series_ref.target else {
            unreachable!();
        };
        let dataset = file.dataset(path).map_err(|error| {
            format!(
                "Dataset reference {} could not be opened: {}",
                series_ref.render(),
                error
            )
        })?;
        let shape = dataset.shape();
        let selection = series_ref.to_preview_selection(&shape)?;
        let points = read_expression_dataset_points(&dataset, series_ref)?;
        let source = ChartSource::DatasetSelection(DatasetChartSource {
            dataset_path: dataset.name(),
            display_path: dataset.name(),
            selection,
            shape,
            kind: DatasetChartKind::Dataset,
        });
        self.add_chart_item(source, points)
            .ok_or_else(|| "Failed to add dataset to multichart".to_string())
    }

    fn global_x_bounds(&self) -> Option<(usize, usize)> {
        let mut visible = self.items.iter().filter(|item| item.visible);
        let first = visible.next()?;
        let mut min_x = first.series.sample_min;
        let mut max_x = first.series.sample_max;
        for item in visible {
            min_x = min_x.min(item.series.sample_min);
            max_x = max_x.max(item.series.sample_max);
        }
        Some((min_x, max_x))
    }

    fn current_sample_bounds(&self) -> Option<(usize, usize, usize, usize)> {
        let (global_min, global_max) = self.global_x_bounds()?;
        let actual_min = global_min.max(self.aoi_from.unwrap_or(global_min));
        let actual_max = global_max.min(self.aoi_to.unwrap_or(global_max));
        (actual_min < actual_max).then_some((global_min, global_max, actual_min, actual_max))
    }

    fn set_viewport_bounds(
        &mut self,
        global_min: usize,
        global_max: usize,
        from: usize,
        to: usize,
    ) {
        let next_from = (from > global_min).then_some(from);
        let next_to = (to < global_max).then_some(to);
        if self.aoi_from == next_from && self.aoi_to == next_to {
            return;
        }
        self.aoi_from = next_from;
        self.aoi_to = next_to;
        self.modified = true;
    }

    fn shift_viewport_by_samples(
        &mut self,
        global_min: usize,
        global_max: usize,
        from: usize,
        to: usize,
        delta: isize,
    ) {
        let range = to.saturating_sub(from);
        if range == 0 {
            return;
        }

        let mut next_from = from as isize - delta;
        let mut next_to = to as isize - delta;
        let global_min = global_min as isize;
        let global_max = global_max as isize;
        if next_from < global_min {
            next_to += global_min - next_from;
            next_from = global_min;
        }
        if next_to > global_max {
            let overflow = next_to - global_max;
            next_from -= overflow;
            next_to = global_max;
        }
        next_from = next_from.max(global_min);
        next_to = next_to.min(global_max);
        if next_to <= next_from {
            return;
        }

        self.set_viewport_bounds(
            global_min as usize,
            global_max as usize,
            next_from as usize,
            next_to as usize,
        );
    }

    fn zoom_with_anchor_ratio(&mut self, percent: f64, anchor_ratio: f64, zoom_in: bool) {
        let Some((global_min, global_max, actual_min, actual_max)) = self.current_sample_bounds()
        else {
            return;
        };
        if !zoom_in && self.aoi_from.is_none() && self.aoi_to.is_none() {
            return;
        }

        let range = actual_max.saturating_sub(actual_min);
        if range <= 1 && zoom_in {
            return;
        }
        if range == 0 {
            return;
        }

        let anchor_ratio = anchor_ratio.clamp(0.0, 1.0);
        let delta = range as f64 * percent / 100.0;
        let new_range = if zoom_in {
            (range as f64 - 2.0 * delta).max(1.0)
        } else {
            (range as f64 + 2.0 * delta).min((global_max - global_min) as f64)
        };
        let anchor = actual_min as f64 + range as f64 * anchor_ratio;
        let mut new_from = anchor - new_range * anchor_ratio;
        let mut new_to = new_from + new_range;
        let global_min_f = global_min as f64;
        let global_max_f = global_max as f64;

        if new_from < global_min_f {
            new_to += global_min_f - new_from;
            new_from = global_min_f;
        }
        if new_to > global_max_f {
            let overflow = new_to - global_max_f;
            new_from -= overflow;
            new_to = global_max_f;
        }
        new_from = new_from.max(global_min_f);
        new_to = new_to.min(global_max_f);

        if global_max <= global_min {
            return;
        }

        let mut from = (new_from.round() as usize).clamp(global_min, global_max.saturating_sub(1));
        let mut to = (new_to.round() as usize).clamp(from.saturating_add(1), global_max);
        if to <= from {
            if global_max.saturating_sub(global_min) <= 1 {
                return;
            }
            from = from.min(global_max - 1);
            to = global_max;
        }

        self.set_viewport_bounds(global_min, global_max, from, to);
    }

    pub fn zoom_in(&mut self, percent: f64) {
        self.zoom_with_anchor_ratio(percent, 0.5, true);
    }

    pub fn clear_zoom(&mut self) {
        self.aoi_from = None;
        self.aoi_to = None;
        self.modified = true;
    }

    pub fn zoom_out(&mut self, percent: f64) {
        self.zoom_with_anchor_ratio(percent, 0.5, false);
    }

    pub fn zoom_in_at_position(&mut self, column: u16, row: u16, percent: f64) -> bool {
        let Some(chart_area) = self.last_chart_area else {
            return false;
        };
        if !point_in_rect(chart_area, column, row) {
            return false;
        }
        let relative_x = column.saturating_sub(chart_area.x) as f64;
        let denom = chart_area.width.saturating_sub(1).max(1) as f64;
        let before = (self.aoi_from, self.aoi_to);
        self.zoom_with_anchor_ratio(percent, relative_x / denom, true);
        (self.aoi_from, self.aoi_to) != before
    }

    pub fn zoom_out_at_position(&mut self, column: u16, row: u16, percent: f64) -> bool {
        let Some(chart_area) = self.last_chart_area else {
            return false;
        };
        if !point_in_rect(chart_area, column, row) {
            return false;
        }
        let relative_x = column.saturating_sub(chart_area.x) as f64;
        let denom = chart_area.width.saturating_sub(1).max(1) as f64;
        let before = (self.aoi_from, self.aoi_to);
        self.zoom_with_anchor_ratio(percent, relative_x / denom, false);
        (self.aoi_from, self.aoi_to) != before
    }

    pub fn start_drag_at_position(&mut self, column: u16, row: u16) -> bool {
        let Some(chart_area) = self.last_chart_area else {
            return false;
        };
        if !point_in_rect(chart_area, column, row) || self.visible_item_count() == 0 {
            return false;
        }
        let Some((_, _, viewport_from, viewport_to)) = self.current_sample_bounds() else {
            return false;
        };
        self.drag_state = Some(ChartDragState {
            anchor_column: column,
            viewport_from,
            viewport_to,
        });
        true
    }

    fn apply_drag_position(&mut self, column: u16) -> bool {
        let Some(chart_area) = self.last_chart_area else {
            return false;
        };
        let Some(drag_state) = self.drag_state.as_ref() else {
            return false;
        };
        let Some((global_min, global_max, _, _)) = self.current_sample_bounds() else {
            return false;
        };
        if chart_area.width <= 1 {
            return false;
        }
        let viewport_range = drag_state
            .viewport_to
            .saturating_sub(drag_state.viewport_from);
        if viewport_range == 0 {
            return false;
        }

        let delta_columns = column as i32 - drag_state.anchor_column as i32;
        let sample_delta = ((delta_columns as f64 / chart_area.width.saturating_sub(1) as f64)
            * viewport_range as f64)
            .round() as isize;
        let before = (self.aoi_from, self.aoi_to);
        self.shift_viewport_by_samples(
            global_min,
            global_max,
            drag_state.viewport_from,
            drag_state.viewport_to,
            sample_delta,
        );
        (self.aoi_from, self.aoi_to) != before
    }

    pub fn drag_to_position(&mut self, column: u16) -> bool {
        let Some(_drag_state) = self.drag_state.as_mut() else {
            return false;
        };
        let _ = column;
        false
    }

    pub fn finish_drag_at_position(&mut self, column: u16) -> bool {
        let changed = self.apply_drag_position(column);
        self.drag_state = None;
        changed
    }

    pub fn end_drag(&mut self) {
        self.drag_state = None;
    }

    pub fn pan_left(&mut self, percent: f64) {
        let Some((min_x, max_x)) = self.global_x_bounds() else {
            return;
        };
        if self.aoi_from.is_none() && self.aoi_to.is_none() {
            return;
        }
        let actual_min = self.aoi_from.unwrap_or(min_x).max(min_x);
        let actual_max = self.aoi_to.unwrap_or(max_x).min(max_x);
        let range = actual_max.saturating_sub(actual_min);
        if range == 0 {
            return;
        }
        let delta = (range as f64 * percent / 100.0).round() as usize;
        let new_min = actual_min.saturating_sub(delta);
        let new_max = actual_max.saturating_sub(delta);
        self.aoi_from = (new_min > min_x).then_some(new_min);
        self.aoi_to = (new_max < max_x).then_some(new_max);
        self.modified = true;
    }

    pub fn pan_right(&mut self, percent: f64) {
        let Some((min_x, max_x)) = self.global_x_bounds() else {
            return;
        };
        if self.aoi_from.is_none() && self.aoi_to.is_none() {
            return;
        }
        let actual_min = self.aoi_from.unwrap_or(min_x).max(min_x);
        let actual_max = self.aoi_to.unwrap_or(max_x).min(max_x);
        let range = actual_max.saturating_sub(actual_min);
        if range == 0 {
            return;
        }
        let delta = (range as f64 * percent / 100.0).round() as usize;
        let new_min = actual_min.saturating_add(delta);
        let new_max = actual_max.saturating_add(delta);
        self.aoi_from = (new_min > min_x).then_some(new_min);
        self.aoi_to = (new_max < max_x).then_some(new_max);
        self.modified = true;
    }

    fn render_chart(&mut self) -> bool {
        if !self.modified {
            return false;
        }
        self.idx = self.idx.clamp(0, self.items.len().saturating_sub(1));
        self.modified = false;

        let visible_items = self
            .items
            .iter()
            .filter(|item| item.visible)
            .collect::<Vec<_>>();
        if visible_items.is_empty() {
            return false;
        }
        let (global_x_min, global_x_max) = self.global_x_bounds().unwrap_or((0, 1));

        let width = self.width;
        let height = self.height;
        let selected_item_id = self.selected_item().map(|item| item.id);
        self.plot_buffer = vec![0; (width * height * 3) as usize];
        let root =
            BitMapBackend::with_buffer(&mut self.plot_buffer, (width, height)).into_drawing_area();
        let (bg_r, bg_g, bg_b) = color_consts::rgb_channels(color_consts::CHART_PLOT_BG_COLOR);
        let (grid_r, grid_g, grid_b) = color_consts::rgb_channels(color_consts::CHART_GRID_COLOR);
        let (axis_r, axis_g, axis_b) = color_consts::rgb_channels(color_consts::CHART_AXIS_COLOR);
        let plot_bg = RGBColor(bg_r, bg_g, bg_b);
        let grid = RGBColor(grid_r, grid_g, grid_b);
        let axis = RGBColor(axis_r, axis_g, axis_b);
        if let Err(e) = root.fill(&plot_bg) {
            log_error(e);
            return false;
        }

        let (x_min, x_max) = match (self.aoi_from, self.aoi_to) {
            (None, None) => (global_x_min, global_x_max),
            (Some(from), None) => (from, global_x_max.max(from)),
            (None, Some(to)) => (global_x_min.min(to), to),
            (Some(from), Some(to)) if from < to => (from, to),
            _ => return false,
        };

        let mut global_y_max = f64::MIN;
        let mut global_y_min = f64::MAX;
        for item in &visible_items {
            if x_max <= item.series.sample_min || x_min >= item.series.sample_max {
                continue;
            }
            let from = item.series.sample_min.max(x_min);
            let to = item.series.sample_max.min(x_max);
            for &(_, y) in &item.series.points[from..to] {
                global_y_max = global_y_max.max(y);
                global_y_min = global_y_min.min(y);
            }
        }
        if !global_y_min.is_finite() || !global_y_max.is_finite() {
            return false;
        }
        let (y_min, y_max) = if (global_y_max - global_y_min).abs() < f64::EPSILON {
            let pad = if global_y_min == 0.0 {
                1.0
            } else {
                global_y_min.abs() * 0.05
            };
            (global_y_min - pad, global_y_max + pad)
        } else {
            (global_y_min, global_y_max)
        };

        let mut plot_x_min = f64::MAX;
        let mut plot_x_max = f64::MIN;
        for item in &visible_items {
            if x_max <= item.series.sample_min || x_min >= item.series.sample_max {
                continue;
            }
            let from = item.series.sample_min.max(x_min);
            let to = item.series.sample_max.min(x_max);
            for &(x, _) in &item.series.points[from..to] {
                plot_x_min = plot_x_min.min(x);
                plot_x_max = plot_x_max.max(x);
            }
        }
        if !plot_x_min.is_finite() || !plot_x_max.is_finite() {
            return false;
        }
        let (plot_x_min, plot_x_max) = if (plot_x_max - plot_x_min).abs() < f64::EPSILON {
            let pad = if plot_x_min == 0.0 {
                1.0
            } else {
                plot_x_min.abs() * 0.05
            };
            (plot_x_min - pad, plot_x_max + pad)
        } else {
            (plot_x_min, plot_x_max)
        };

        let data_series = visible_items.iter().map(|item| {
            let local_x_min = item
                .series
                .sample_min
                .max(x_min)
                .clamp(item.series.sample_min, item.series.sample_max);
            let local_x_max = item
                .series
                .sample_max
                .min(x_max)
                .clamp(item.series.sample_min, item.series.sample_max);
            let data_points = item.series.points[local_x_min..local_x_max]
                .iter()
                .map(|(x, y)| (*x, *y));
            (item.id, item.label.clone(), item.color_slot, data_points)
        });

        let y_label_area_size = format!("{y_max:.4}").len() as u32 * 3 + 30;
        let chart = plotters::prelude::ChartBuilder::on(&root)
            .margin(10)
            .x_label_area_size(30)
            .y_label_area_size(y_label_area_size)
            .build_cartesian_2d(plot_x_min..plot_x_max, y_min..y_max);

        let mut chart = match chart {
            Ok(chart) => chart,
            Err(e) => {
                log_error(e);
                return false;
            }
        };

        if let Err(e) = chart
            .configure_mesh()
            .x_desc(self.x_axis_policy.label())
            .y_label_style(("sans-serif", 18).into_font().color(&axis))
            .x_label_style(("sans-serif", 18).into_font().color(&axis))
            .axis_style(ShapeStyle::from(&axis).stroke_width(2))
            .light_line_style(grid.mix(0.35))
            .bold_line_style(grid.mix(0.55))
            .draw()
        {
            log_error(e);
        }

        for (item_id, label, color_slot, data) in data_series {
            let (r, g, b) =
                color_consts::rgb_channels(color_consts::chart_series_color(color_slot));
            let color = RGBColor(r, g, b);
            let stroke_width =
                if self.marked_base_item == Some(item_id) || selected_item_id == Some(item_id) {
                    4
                } else {
                    3
                };
            let line_series = plotters::prelude::LineSeries::new(
                data,
                ShapeStyle::from(&color).stroke_width(stroke_width),
            );
            let series = match chart.draw_series(line_series) {
                Ok(series) => series,
                Err(e) => {
                    log_error(e);
                    continue;
                }
            };
            series.label(label).legend(move |(x, y)| {
                plotters::prelude::PathElement::new(
                    vec![(x, y), (x + 20, y)],
                    plotters::prelude::ShapeStyle {
                        filled: true,
                        stroke_width,
                        color: plotters::style::Color::to_rgba(&color),
                    },
                )
            });
        }

        if let Err(e) = root.present() {
            log_error(e);
        }

        true
    }

    pub(crate) fn render(&mut self, f: &mut ratatui::Frame<'_>, area: Rect) {
        let header_block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Green))
            .border_type(BorderType::Rounded)
            .title("Multi-Chart Comparison Workspace")
            .bg(color_consts::BG_COLOR)
            .title_style(Style::default().fg(color_consts::TITLE).bold())
            .title_alignment(Alignment::Center);
        f.render_widget(header_block, area);

        let inner_area = area.inner(Margin {
            horizontal: 1,
            vertical: 1,
        });
        let (workspace_area, prompt_area) = if self.expression_prompt.is_some() {
            let split = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(10), Constraint::Length(6)])
                .split(inner_area);
            (split[0], Some(split[1]))
        } else {
            (inner_area, None)
        };

        if self.items.is_empty() {
            self.render_empty(f, workspace_area);
            if let Some(prompt_area) = prompt_area {
                self.render_expression_prompt(f, prompt_area);
            }
            return;
        }

        let panes = if workspace_area.width < 110 {
            Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(11), Constraint::Min(12)])
                .split(workspace_area)
        } else {
            Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Length(42), Constraint::Min(20)])
                .split(workspace_area)
        };

        let (sidebar_area, chart_area) = (panes[0], panes[1]);
        let sidebar_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(5), Constraint::Length(6)])
            .split(sidebar_area);
        self.render_item_list(f, sidebar_chunks[0]);
        self.render_selected_details(f, sidebar_chunks[1]);
        self.render_chart_panel(f, chart_area);
        if let Some(prompt_area) = prompt_area {
            self.render_expression_prompt(f, prompt_area);
        }
    }

    fn render_empty(&mut self, f: &mut ratatui::Frame<'_>, area: Rect) {
        self.last_chart_area = None;
        self.drag_state = None;
        let no_data_message = concat!(
            "No chart items yet.\n\n",
            "Press 'm' on any previewable dataset view to add it here.\n",
            "The same dataset can appear multiple times with different x dimensions or fixed indices.\n",
            "Use Space to mark a base series, then D/S/R/P to derive difference, sum, ratio, or product."
        );
        let paragraph = Paragraph::new(no_data_message)
            .alignment(Alignment::Center)
            .style(Style::default().fg(color_consts::TITLE))
            .wrap(Wrap { trim: true });
        f.render_widget(paragraph, area);
    }

    fn render_item_list(&self, f: &mut ratatui::Frame<'_>, area: Rect) {
        let block = Block::default()
            .title(format!(
                "Items ({}/{} visible)",
                self.visible_item_count(),
                self.items.len()
            ))
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(color_consts::BREAK_COLOR))
            .title_style(Style::default().fg(color_consts::TITLE).bold());
        let inner = block.inner(area);
        f.render_widget(block, area);

        let available_rows = inner.height as usize;
        if available_rows == 0 {
            return;
        }

        let half = available_rows / 2;
        let start = self.idx.saturating_sub(half);
        let end = usize::min(start + available_rows, self.items.len());
        let start = end.saturating_sub(available_rows);

        let lines = self.items[start..end]
            .iter()
            .enumerate()
            .map(|(offset, item)| {
                let absolute_idx = start + offset;
                let (r, g, b) =
                    color_consts::rgb_channels(color_consts::chart_series_color(item.color_slot));
                let marker = compat::chart_visibility_marker(item.visible);
                let prefix = if absolute_idx == self.idx { "> " } else { "  " };
                let is_selected = absolute_idx == self.idx;
                let is_base = self.marked_base_item == Some(item.id);
                let id_style = if is_selected {
                    Style::default()
                        .fg(color_consts::VARIABLE_BLUE_BUILTIN)
                        .bold()
                } else {
                    Style::default().fg(color_consts::VARIABLE_BLUE_BUILTIN)
                };
                let label_style = match (is_selected, item.visible) {
                    (true, true) => Style::default().fg(color_consts::TITLE).bold(),
                    (true, false) => Style::default().fg(color_consts::TITLE).bold().dim(),
                    (false, true) => Style::default().fg(color_consts::BUILT_IN_VALUE_COLOR),
                    (false, false) => Style::default().fg(color_consts::TYPE_DESC_COLOR).dim(),
                };
                let label_style = if is_base {
                    label_style.underlined()
                } else {
                    label_style
                };
                Line::from(vec![
                    Span::styled(
                        prefix,
                        if is_selected {
                            Style::default().fg(color_consts::TITLE).bold()
                        } else {
                            Style::default().fg(color_consts::BREAK_COLOR)
                        },
                    ),
                    Span::styled(marker, Style::default().fg(Color::Rgb(r, g, b)).bold()),
                    Span::raw(" "),
                    Span::styled(format!("(${}) ", item.id.0), id_style),
                    Span::styled(item.list_label(), label_style),
                ])
            })
            .collect::<Vec<_>>();
        f.render_widget(
            Paragraph::new(Text::from(lines)).wrap(Wrap { trim: true }),
            inner,
        );
    }

    fn render_selected_details(&self, f: &mut ratatui::Frame<'_>, area: Rect) {
        let block = Block::default()
            .title("Active item")
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(color_consts::VARIABLE_BLUE_BUILTIN))
            .title_style(Style::default().fg(color_consts::TITLE).bold());
        let inner = block.inner(area);
        f.render_widget(block, area);

        let Some(item) = self.selected_item() else {
            return;
        };

        let viewport = match (self.aoi_from, self.aoi_to) {
            (None, None) => "full range".to_string(),
            (Some(from), Some(to)) => format!("{from}..{to}"),
            (Some(from), None) => format!("{from}..end"),
            (None, Some(to)) => format!("start..{to}"),
        };
        let base_line = self
            .marked_base_item_ref()
            .map(|item| item.reference_label())
            .unwrap_or_else(|| "none".to_string());

        let lines = match &item.source {
            ChartSource::DatasetSelection(source) => vec![
                Line::from(vec![
                    Span::styled(
                        "base ",
                        Style::default().fg(color_consts::VARIABLE_BLUE_BUILTIN),
                    ),
                    Span::raw(base_line),
                ]),
                Line::from(vec![
                    Span::styled(
                        "path ",
                        Style::default().fg(color_consts::VARIABLE_BLUE_BUILTIN),
                    ),
                    Span::raw(source.display_path.clone()),
                ]),
                Line::from(vec![
                    Span::styled(
                        "type ",
                        Style::default().fg(color_consts::VARIABLE_BLUE_BUILTIN),
                    ),
                    Span::raw(source.kind_label()),
                    Span::raw("  "),
                    Span::styled(
                        "shape ",
                        Style::default().fg(color_consts::VARIABLE_BLUE_BUILTIN),
                    ),
                    Span::raw(source.shape_summary()),
                ]),
                Line::from(vec![
                    Span::styled(
                        "view ",
                        Style::default().fg(color_consts::VARIABLE_BLUE_BUILTIN),
                    ),
                    Span::raw(source.selection_summary()),
                ]),
                Line::from(vec![
                    Span::styled(
                        "stats ",
                        Style::default().fg(color_consts::VARIABLE_BLUE_BUILTIN),
                    ),
                    Span::raw(item.stats_summary()),
                    Span::raw("  "),
                    Span::styled(
                        "align ",
                        Style::default().fg(color_consts::VARIABLE_BLUE_BUILTIN),
                    ),
                    Span::raw(self.x_axis_policy.label()),
                    Span::raw("  "),
                    Span::styled(
                        "zoom ",
                        Style::default().fg(color_consts::VARIABLE_BLUE_BUILTIN),
                    ),
                    Span::raw(viewport),
                ]),
            ],
            ChartSource::BuiltinDerived(source) => vec![
                Line::from(vec![
                    Span::styled(
                        "expr ",
                        Style::default().fg(color_consts::VARIABLE_BLUE_BUILTIN),
                    ),
                    Span::raw(source.expression()),
                ]),
                Line::from(vec![
                    Span::styled(
                        "lhs ",
                        Style::default().fg(color_consts::VARIABLE_BLUE_BUILTIN),
                    ),
                    Span::raw(source.lhs_view.clone()),
                ]),
                Line::from(vec![
                    Span::styled(
                        "rhs ",
                        Style::default().fg(color_consts::VARIABLE_BLUE_BUILTIN),
                    ),
                    Span::raw(source.rhs_view.clone()),
                ]),
                Line::from(vec![
                    Span::styled(
                        "align ",
                        Style::default().fg(color_consts::VARIABLE_BLUE_BUILTIN),
                    ),
                    Span::raw(source.alignment_summary()),
                    Span::raw("  "),
                    Span::styled(
                        "zoom ",
                        Style::default().fg(color_consts::VARIABLE_BLUE_BUILTIN),
                    ),
                    Span::raw(viewport),
                ]),
            ],
            ChartSource::DerivedExpression { expression, .. } => vec![
                Line::from(vec![
                    Span::styled(
                        "expr ",
                        Style::default().fg(color_consts::VARIABLE_BLUE_BUILTIN),
                    ),
                    Span::raw(expression.clone()),
                ]),
                Line::from(vec![
                    Span::styled(
                        "base ",
                        Style::default().fg(color_consts::VARIABLE_BLUE_BUILTIN),
                    ),
                    Span::raw(base_line),
                ]),
                Line::from(vec![
                    Span::styled(
                        "type ",
                        Style::default().fg(color_consts::VARIABLE_BLUE_BUILTIN),
                    ),
                    Span::raw(item.source.source_kind_label()),
                ]),
                Line::from(vec![
                    Span::styled(
                        "stats ",
                        Style::default().fg(color_consts::VARIABLE_BLUE_BUILTIN),
                    ),
                    Span::raw(item.stats_summary()),
                    Span::raw("  "),
                    Span::styled(
                        "align ",
                        Style::default().fg(color_consts::VARIABLE_BLUE_BUILTIN),
                    ),
                    Span::raw(self.x_axis_policy.label()),
                ]),
                Line::from(vec![
                    Span::styled(
                        "zoom ",
                        Style::default().fg(color_consts::VARIABLE_BLUE_BUILTIN),
                    ),
                    Span::raw(viewport),
                ]),
            ],
        };

        f.render_widget(
            Paragraph::new(Text::from(lines)).wrap(Wrap { trim: true }),
            inner,
        );
    }

    fn render_chart_panel(&mut self, f: &mut ratatui::Frame<'_>, area: Rect) {
        let block = Block::default()
            .title(format!("Overlay chart [{}]", self.x_axis_policy.label()))
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(color_consts::BREAK_COLOR))
            .title_style(Style::default().fg(color_consts::TITLE).bold());
        let chart_area = block.inner(area);
        self.last_chart_area =
            (chart_area.width > 0 && chart_area.height > 0).then_some(chart_area);
        f.render_widget(block, area);

        if self.visible_item_count() == 0 {
            let paragraph = Paragraph::new(
                format!(
                    "All chart items are hidden.\nPress 'v' to toggle the selected item back on.\nCurrent alignment: {}.",
                    self.x_axis_policy.description()
                ),
            )
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true });
            f.render_widget(paragraph, chart_area);
            return;
        }
        if chart_area.width == 0 || chart_area.height == 0 {
            return;
        }

        let (cell_w, cell_h) = self.picker.font_size();
        let new_height = chart_area.height as u32 * cell_h as u32;
        let new_width = chart_area.width as u32 * cell_w as u32;
        if new_height != self.height || new_width != self.width {
            self.height = new_height;
            self.width = new_width;
            self.modified = true;
            self.stateful_protocol = None;
        }

        if self.render_chart() {
            let image = ImageBuffer::<Rgb<u8>, _>::from_raw(
                self.width,
                self.height,
                self.plot_buffer.clone(),
            );
            let Some(image) = image else {
                log_error("Failed to create image buffer from plot buffer");
                return;
            };
            let dyn_img = DynamicImage::ImageRgb8(image);
            self.stateful_protocol = Some(self.picker.new_resize_protocol(dyn_img));
        }

        match self.stateful_protocol {
            None => {
                let paragraph = Paragraph::new("Rendering failed")
                    .alignment(Alignment::Center)
                    .wrap(Wrap { trim: true });
                f.render_widget(paragraph, chart_area);
            }
            Some(ref mut protocol) => {
                f.render_stateful_widget(StatefulImage::default(), chart_area, protocol);
            }
        }
    }

    fn render_expression_prompt(&self, f: &mut ratatui::Frame<'_>, area: Rect) {
        let Some(prompt) = self.expression_prompt.as_ref() else {
            return;
        };
        let title = match &prompt.error {
            Some(_) => "Expression prompt [invalid]",
            None => "Expression prompt",
        };
        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(color_consts::BREAK_COLOR))
            .title_style(Style::default().fg(color_consts::TITLE).bold());
        let inner = block.inner(area);
        f.render_widget(block, area);

        let mut lines = vec![
            Line::from(vec![
                Span::styled("= ", Style::default().fg(color_consts::TITLE).bold()),
                Span::raw(prompt.buffer.clone()),
            ]),
            Line::from(vec![
                Span::styled("Syntax ", Style::default().fg(color_consts::VARIABLE_BLUE_BUILTIN)),
                Span::raw(
                    "$1 + !/ds[..,0] * #/cal:scale   or   (!/x_ticks, $2 + #/cal/offset)",
                ),
            ]),
            Line::from(vec![
                Span::styled("Rules ", Style::default().fg(color_consts::VARIABLE_BLUE_BUILTIN)),
                Span::raw(
                    "single expr => y-series; (x,y) => x/y series. Use $id or !/path[..] for series, #/path for scalar datasets, and :ATTR on ! or # for explicit attributes",
                ),
            ]),
        ];
        if let Some(error) = &prompt.error {
            lines.push(Line::from(vec![
                Span::styled(
                    "Error ",
                    Style::default().fg(color_consts::ERROR_COLOR).bold(),
                ),
                Span::raw(error.clone()),
            ]));
        } else {
            lines.push(Line::from(vec![
                Span::styled(
                    "Keys ",
                    Style::default().fg(color_consts::VARIABLE_BLUE_BUILTIN),
                ),
                Span::raw("Enter create  Esc cancel"),
            ]));
        }

        f.render_widget(
            Paragraph::new(Text::from(lines)).wrap(Wrap { trim: true }),
            inner,
        );
        let cursor = ratatui::layout::Position::new(
            inner.x.saturating_add(2 + prompt.cursor as u16),
            inner.y,
        );
        f.set_cursor_position(cursor);
    }
}

fn point_in_rect(rect: Rect, column: u16, row: u16) -> bool {
    column >= rect.x
        && column < rect.x.saturating_add(rect.width)
        && row >= rect.y
        && row < rect.y.saturating_add(rect.height)
}

fn tokenize_expression(input: &str) -> Result<Vec<ExpressionToken>, String> {
    let mut chars = input.chars().peekable();
    let mut tokens = Vec::new();

    while let Some(&ch) = chars.peek() {
        match ch {
            ' ' | '\t' => {
                chars.next();
            }
            '$' => {
                chars.next();
                tokens.push(ExpressionToken::ItemRef(parse_expression_item_id(
                    &mut chars,
                )?));
            }
            '!' => {
                chars.next();
                tokens.push(ExpressionToken::SeriesRef(parse_expression_series_ref(
                    &mut chars,
                )?));
            }
            '#' => {
                chars.next();
                tokens.push(ExpressionToken::ScalarRef(parse_expression_scalar_ref(
                    &mut chars,
                )?));
            }
            '0'..='9' | '.' => {
                let mut number = String::new();
                let mut seen_dot = false;
                while let Some(next) = chars.peek() {
                    if next.is_ascii_digit() {
                        number.push(*next);
                        chars.next();
                    } else if *next == '.' && !seen_dot {
                        seen_dot = true;
                        number.push(*next);
                        chars.next();
                    } else {
                        break;
                    }
                }
                let value = number
                    .parse::<f64>()
                    .map_err(|_| format!("Invalid numeric literal '{number}'"))?;
                tokens.push(ExpressionToken::Number(value));
            }
            '+' => {
                chars.next();
                tokens.push(ExpressionToken::Plus);
            }
            '-' => {
                chars.next();
                tokens.push(ExpressionToken::Minus);
            }
            '*' => {
                chars.next();
                tokens.push(ExpressionToken::Star);
            }
            '/' => {
                chars.next();
                tokens.push(ExpressionToken::Slash);
            }
            ',' => {
                chars.next();
                tokens.push(ExpressionToken::Comma);
            }
            '(' => {
                chars.next();
                tokens.push(ExpressionToken::LParen);
            }
            ')' => {
                chars.next();
                tokens.push(ExpressionToken::RParen);
            }
            other => {
                return Err(format!(
                    "Unsupported character '{}' in expression. Use $id item references, !series references, #scalar references, numbers, + - * /, commas, and parentheses",
                    other
                ));
            }
        }
    }

    Ok(tokens)
}

fn parse_expression_item_id(
    chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
) -> Result<ChartItemId, String> {
    let mut digits = String::new();
    while let Some(next) = chars.peek() {
        if next.is_ascii_digit() {
            digits.push(*next);
            chars.next();
        } else {
            break;
        }
    }
    if digits.is_empty() {
        return Err("Expected digits after '$' item reference".to_string());
    }
    let id = digits
        .parse::<u64>()
        .map_err(|_| format!("Invalid chart item reference '${digits}'"))?;
    Ok(ChartItemId(id))
}

fn parse_expression_absolute_path(
    chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
) -> Result<String, String> {
    let mut path = String::new();
    while let Some(&next) = chars.peek() {
        if next == '['
            || next == ':'
            || next.is_whitespace()
            || matches!(next, '+' | '-' | '*' | ',' | '(' | ')')
        {
            break;
        }
        path.push(next);
        chars.next();
    }
    if path.is_empty() {
        return Err("Expected an absolute HDF5 path beginning with '/'".to_string());
    }
    Ok(path)
}

fn parse_expression_object_target(
    chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
    prefix: char,
) -> Result<ExpressionObjectTarget, String> {
    match chars.peek().copied() {
        Some('/') => Ok(ExpressionObjectTarget::AbsolutePath(parse_expression_absolute_path(
            chars,
        )?)),
        Some('$') => {
            chars.next();
            Ok(ExpressionObjectTarget::ItemRef(parse_expression_item_id(chars)?))
        }
        _ => Err(match prefix {
            '!' => {
                "Series references must use an absolute path like !/group/dataset or an item-backed attribute like !$1:ATTR"
                    .to_string()
            }
            '#' => {
                "Scalar references must use an absolute path like #/group/scalar or an item-backed attribute like #$1:ATTR"
                    .to_string()
            }
            _ => "Invalid expression reference".to_string(),
        }),
    }
}

fn parse_expression_series_ref(
    chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
) -> Result<ExpressionSeriesRef, String> {
    let target = parse_expression_object_target(chars, '!')?;
    let attr_name = if chars.peek() == Some(&':') {
        chars.next();
        let attr_name = parse_expression_attribute_name(chars);
        if attr_name.is_empty() {
            return Err(format!(
                "Expected an attribute name after '{}:' in series reference",
                target.render()
            ));
        }
        Some(attr_name)
    } else {
        None
    };

    if attr_name.is_some() && chars.peek() == Some(&'[') {
        return Err(
            "Series attribute references currently use the full attribute value and do not support selectors"
                .to_string(),
        );
    }

    let selectors = if chars.peek() == Some(&'[') {
        chars.next();
        let mut spec = String::new();
        let mut closed = false;
        for next in chars.by_ref() {
            if next == ']' {
                closed = true;
                break;
            }
            spec.push(next);
        }
        if !closed {
            return Err(format!(
                "Series reference '{}[{spec}' is missing a closing ']'",
                match &attr_name {
                    Some(attr_name) => format!("!{}:{attr_name}", target.render()),
                    None => format!("!{}", target.render()),
                }
            ));
        }
        Some(parse_expression_dataset_selectors(
            &match &attr_name {
                Some(attr_name) => format!("!{}:{attr_name}", target.render()),
                None => format!("!{}", target.render()),
            },
            &spec,
        )?)
    } else {
        None
    };

    Ok(ExpressionSeriesRef {
        target,
        attr_name,
        selectors,
    })
}

fn parse_expression_scalar_ref(
    chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
) -> Result<ExpressionScalarRef, String> {
    let target = parse_expression_object_target(chars, '#')?;
    let attr_name = if chars.peek() == Some(&':') {
        chars.next();
        let attr_name = parse_expression_attribute_name(chars);
        if attr_name.is_empty() {
            return Err(format!(
                "Expected an attribute name after '{}:' in scalar reference",
                target.render()
            ));
        }
        Some(attr_name)
    } else {
        None
    };

    if matches!(target, ExpressionObjectTarget::ItemRef(_)) && attr_name.is_none() {
        return Err("Scalar item references must name an attribute like #$1:OFFSET".to_string());
    }
    if chars.peek() == Some(&'[') {
        return Err("Scalar references cannot use series selectors".to_string());
    }

    Ok(ExpressionScalarRef { target, attr_name })
}

fn parse_expression_dataset_selectors(
    reference: &str,
    spec: &str,
) -> Result<Vec<ExpressionDatasetSelector>, String> {
    let parts = spec
        .split(',')
        .map(str::trim)
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    if parts.is_empty() || parts.iter().any(|part| part.is_empty()) {
        return Err(format!(
            "Series reference '{reference}[{spec}]' must use comma-separated selectors like [..,0]"
        ));
    }

    parts
        .into_iter()
        .map(|part| {
            if part == ".." {
                Ok(ExpressionDatasetSelector::All)
            } else {
                part.parse::<usize>()
                    .map(ExpressionDatasetSelector::Index)
                    .map_err(|_| {
                        format!(
                            "Series reference '{reference}[{spec}]' has invalid selector '{part}'; use '..' or a non-negative integer"
                        )
                    })
            }
        })
        .collect()
}

fn parse_expression_attribute_name(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) -> String {
    let mut attr_name = String::new();
    while let Some(&next) = chars.peek() {
        if next == '[' || next.is_whitespace() || matches!(next, '+' | '-' | '*' | '/' | '(' | ')')
        {
            break;
        }
        attr_name.push(next);
        chars.next();
    }
    attr_name
}

fn parse_expression(tokens: &[ExpressionToken]) -> Result<ExpressionAst, String> {
    fn parse_expr(tokens: &[ExpressionToken], pos: &mut usize) -> Result<ExpressionAst, String> {
        let mut expr = parse_term(tokens, pos)?;
        while *pos < tokens.len() {
            let op = match tokens[*pos] {
                ExpressionToken::Plus => ExprBinaryOp::Add,
                ExpressionToken::Minus => ExprBinaryOp::Sub,
                _ => break,
            };
            *pos += 1;
            let rhs = parse_term(tokens, pos)?;
            expr = ExpressionAst::Binary {
                op,
                lhs: Box::new(expr),
                rhs: Box::new(rhs),
            };
        }
        Ok(expr)
    }

    fn parse_term(tokens: &[ExpressionToken], pos: &mut usize) -> Result<ExpressionAst, String> {
        let mut expr = parse_factor(tokens, pos)?;
        while *pos < tokens.len() {
            let op = match tokens[*pos] {
                ExpressionToken::Star => ExprBinaryOp::Mul,
                ExpressionToken::Slash => ExprBinaryOp::Div,
                _ => break,
            };
            *pos += 1;
            let rhs = parse_factor(tokens, pos)?;
            expr = ExpressionAst::Binary {
                op,
                lhs: Box::new(expr),
                rhs: Box::new(rhs),
            };
        }
        Ok(expr)
    }

    fn parse_factor(tokens: &[ExpressionToken], pos: &mut usize) -> Result<ExpressionAst, String> {
        if *pos >= tokens.len() {
            return Err("Unexpected end of expression".to_string());
        }
        match &tokens[*pos] {
            ExpressionToken::Number(value) => {
                *pos += 1;
                Ok(ExpressionAst::Number(*value))
            }
            ExpressionToken::ItemRef(id) => {
                *pos += 1;
                Ok(ExpressionAst::ItemRef(*id))
            }
            ExpressionToken::SeriesRef(series_ref) => {
                *pos += 1;
                Ok(ExpressionAst::SeriesRef(series_ref.clone()))
            }
            ExpressionToken::ScalarRef(scalar_ref) => {
                *pos += 1;
                Ok(ExpressionAst::ScalarRef(scalar_ref.clone()))
            }
            ExpressionToken::Minus => {
                *pos += 1;
                Ok(ExpressionAst::UnaryMinus(Box::new(parse_factor(
                    tokens, pos,
                )?)))
            }
            ExpressionToken::LParen => {
                *pos += 1;
                let expr = parse_expr(tokens, pos)?;
                if *pos >= tokens.len() || !matches!(tokens[*pos], ExpressionToken::RParen) {
                    return Err("Missing closing ')' in expression".to_string());
                }
                *pos += 1;
                Ok(expr)
            }
            other => Err(format!("Unexpected token '{other:?}' in expression")),
        }
    }

    let mut pos = 0;
    let expr = parse_expr(tokens, &mut pos)?;
    if pos != tokens.len() {
        return Err("Unexpected trailing tokens in expression".to_string());
    }
    Ok(expr)
}

fn parse_derived_expression(tokens: &[ExpressionToken]) -> Result<ParsedExpression, String> {
    if let Some((x_tokens, y_tokens)) = split_top_level_tuple(tokens) {
        let x_expr = parse_expression(x_tokens)?;
        let y_expr = parse_expression(y_tokens)?;
        return Ok(ParsedExpression::XySeries(x_expr, y_expr));
    }
    Ok(ParsedExpression::YSeries(parse_expression(tokens)?))
}

fn split_top_level_tuple(
    tokens: &[ExpressionToken],
) -> Option<(&[ExpressionToken], &[ExpressionToken])> {
    if tokens.len() < 5
        || !matches!(tokens.first(), Some(ExpressionToken::LParen))
        || !matches!(tokens.last(), Some(ExpressionToken::RParen))
    {
        return None;
    }

    let mut depth = 0usize;
    let mut comma_index = None;
    for (idx, token) in tokens.iter().enumerate() {
        match token {
            ExpressionToken::LParen => depth += 1,
            ExpressionToken::RParen => {
                depth = depth.saturating_sub(1);
                if depth == 0 && idx != tokens.len() - 1 {
                    return None;
                }
            }
            ExpressionToken::Comma if depth == 1 => {
                if comma_index.replace(idx).is_some() {
                    return None;
                }
            }
            _ => {}
        }
    }

    let comma_index = comma_index?;
    let x_tokens = &tokens[1..comma_index];
    let y_tokens = &tokens[comma_index + 1..tokens.len() - 1];
    if x_tokens.is_empty() || y_tokens.is_empty() {
        return None;
    }
    Some((x_tokens, y_tokens))
}

#[derive(Debug, Default)]
struct ExpressionRefs {
    item_ids: Vec<ChartItemId>,
    series_refs: Vec<ExpressionSeriesRef>,
    scalar_refs: Vec<ExpressionScalarRef>,
}

fn collect_expression_refs(expr: &ExpressionAst, out: &mut ExpressionRefs) {
    match expr {
        ExpressionAst::Number(_) => {}
        ExpressionAst::ItemRef(id) => out.item_ids.push(*id),
        ExpressionAst::SeriesRef(series_ref) => out.series_refs.push(series_ref.clone()),
        ExpressionAst::ScalarRef(scalar_ref) => out.scalar_refs.push(scalar_ref.clone()),
        ExpressionAst::UnaryMinus(inner) => collect_expression_refs(inner, out),
        ExpressionAst::Binary { lhs, rhs, .. } => {
            collect_expression_refs(lhs, out);
            collect_expression_refs(rhs, out);
        }
    }
}

fn collect_parsed_expression_refs(expr: &ParsedExpression, out: &mut ExpressionRefs) {
    match expr {
        ParsedExpression::YSeries(ast) => collect_expression_refs(ast, out),
        ParsedExpression::XySeries(x_ast, y_ast) => {
            collect_expression_refs(x_ast, out);
            collect_expression_refs(y_ast, out);
        }
    }
}

#[derive(Debug, Clone)]
struct ExpressionSeriesInput {
    label: String,
    points: Vec<Point>,
}

struct EvaluatedExpression {
    points: Vec<Point>,
    kind: DerivedExpressionKind,
    input_ids: Vec<ChartItemId>,
}

fn validate_expression_series_compatibility(
    referenced: &[ExpressionSeriesInput],
    expected_len: usize,
    require_matching_x: bool,
) -> Result<(), String> {
    let Some(first) = referenced.first() else {
        return Err("Expression must reference at least one chart item".to_string());
    };
    for item in &referenced[1..] {
        if item.points.len() != expected_len {
            return Err(format!(
                "Expression series lengths must match exactly: {} has len {}, but {} has len {}",
                first.label,
                expected_len,
                item.label,
                item.points.len()
            ));
        }
        if require_matching_x {
            for idx in 0..expected_len {
                if item.points[idx].0 != first.points[idx].0 {
                    return Err(format!(
                        "Expression x-values must match exactly across referenced items; mismatch at sample index {}",
                        idx
                    ));
                }
            }
        }
    }
    Ok(())
}

fn dataset_ploting_data_from_points(points: Vec<Point>) -> Result<DatasetPlotingData, String> {
    let Some((_, first_y)) = points.first().copied() else {
        return Err("Cannot build a preview from an empty series".to_string());
    };
    let (min, max) = points
        .iter()
        .fold((first_y, first_y), |(min, max), (_, y)| {
            (min.min(*y), max.max(*y))
        });
    Ok(DatasetPlotingData {
        length: points.len(),
        min,
        max,
        data: points,
    })
}

fn eval_expression_at(
    expr: &ExpressionAst,
    idx: usize,
    state: &MultiChartState,
    series_values: &std::collections::HashMap<ExpressionSeriesRef, Vec<Point>>,
    scalar_values: &std::collections::HashMap<ExpressionScalarRef, f64>,
) -> Result<f64, String> {
    match expr {
        ExpressionAst::Number(value) => Ok(*value),
        ExpressionAst::ItemRef(id) => state
            .item_by_id(*id)
            .and_then(|item| item.series.points.get(idx).map(|(_, y)| *y))
            .ok_or_else(|| {
                format!(
                    "Chart item ${} is unavailable at sample index {}",
                    id.0, idx
                )
            }),
        ExpressionAst::SeriesRef(series_ref) => series_values
            .get(series_ref)
            .and_then(|points| points.get(idx).map(|(_, y)| *y))
            .ok_or_else(|| {
                format!(
                    "Series reference {} is unavailable at sample index {}",
                    series_ref.render(),
                    idx
                )
            }),
        ExpressionAst::ScalarRef(scalar_ref) => scalar_values
            .get(scalar_ref)
            .copied()
            .ok_or_else(|| format!("Scalar reference {} was not resolved", scalar_ref.render())),
        ExpressionAst::UnaryMinus(inner) => Ok(-eval_expression_at(
            inner,
            idx,
            state,
            series_values,
            scalar_values,
        )?),
        ExpressionAst::Binary { op, lhs, rhs } => {
            let lhs = eval_expression_at(lhs, idx, state, series_values, scalar_values)?;
            let rhs = eval_expression_at(rhs, idx, state, series_values, scalar_values)?;
            match op {
                ExprBinaryOp::Add => Ok(lhs + rhs),
                ExprBinaryOp::Sub => Ok(lhs - rhs),
                ExprBinaryOp::Mul => Ok(lhs * rhs),
                ExprBinaryOp::Div => {
                    if rhs == 0.0 {
                        Err(format!(
                            "Cannot divide by zero in expression at sample index {}",
                            idx
                        ))
                    } else {
                        Ok(lhs / rhs)
                    }
                }
            }
        }
    }
}

fn resolve_expression_scalar_values(
    state: &MultiChartState,
    file: Option<&File>,
    refs: &[ExpressionScalarRef],
) -> Result<std::collections::HashMap<ExpressionScalarRef, f64>, String> {
    if refs.is_empty() {
        return Ok(std::collections::HashMap::new());
    }
    let file = file.ok_or_else(|| {
        "Scalar references require an open file handle, but no file is loaded".to_string()
    })?;
    let mut values = std::collections::HashMap::with_capacity(refs.len());
    for scalar_ref in refs {
        let value = resolve_expression_scalar_value(state, file, scalar_ref)?;
        values.insert(scalar_ref.clone(), value);
    }
    Ok(values)
}

fn resolve_expression_series_values(
    state: &MultiChartState,
    file: Option<&File>,
    refs: &[ExpressionSeriesRef],
) -> Result<std::collections::HashMap<ExpressionSeriesRef, Vec<Point>>, String> {
    if refs.is_empty() {
        return Ok(std::collections::HashMap::new());
    }
    let file = file.ok_or_else(|| {
        "Series references require an open file handle, but no file is loaded".to_string()
    })?;
    let mut series = std::collections::HashMap::with_capacity(refs.len());
    for series_ref in refs {
        let points = resolve_expression_series_value(state, file, series_ref)?;
        series.insert(series_ref.clone(), points);
    }
    Ok(series)
}

fn resolve_expression_series_value(
    state: &MultiChartState,
    file: &File,
    series_ref: &ExpressionSeriesRef,
) -> Result<Vec<Point>, String> {
    match (&series_ref.target, &series_ref.attr_name) {
        (ExpressionObjectTarget::ItemRef(id), None) => state
            .item_by_id(*id)
            .map(|item| item.series.points.clone())
            .ok_or_else(|| format!("Unknown chart item reference ${}", id.0)),
        (target, Some(attr_name)) => {
            let object_path = resolve_expression_target_path(state, target, &series_ref.render())?;
            let attr = open_expression_attribute(file, &object_path, attr_name)?;
            read_expression_numeric_series_attr(&attr, &series_ref.render())
        }
        (ExpressionObjectTarget::AbsolutePath(path), None) => {
            let object_path = normalize_absolute_object_path(path)?;
            let dataset = file.dataset(&object_path).map_err(|error| {
                format!(
                    "Series reference {} could not open dataset '{}': {}",
                    series_ref.render(),
                    object_path,
                    error
                )
            })?;
            read_expression_dataset_points(&dataset, series_ref)
        }
    }
}

fn read_expression_dataset_points(
    dataset: &Dataset,
    series_ref: &ExpressionSeriesRef,
) -> Result<Vec<Point>, String> {
    let shape = dataset.shape();
    let preview_selection = series_ref.to_preview_selection(&shape)?;
    let selection = preview_selection_to_hyperslab(&shape, &preview_selection)?;
    let dtype = dataset.dtype().map_err(|error| {
        format!(
            "Failed to inspect dataset type for {}: {}",
            series_ref.render(),
            error
        )
    })?;
    let type_desc = dtype.to_descriptor().map_err(|error| {
        format!(
            "Failed to inspect dataset type for {}: {}",
            series_ref.render(),
            error
        )
    })?;

    let values = match type_desc {
        TypeDescriptor::Integer(IntSize::U1) => dataset
            .read_slice_1d::<i8, _>(selection.clone())
            .map(|values| {
                values
                    .into_iter()
                    .map(|value| value as f64)
                    .collect::<Vec<_>>()
            }),
        TypeDescriptor::Integer(IntSize::U2) => dataset
            .read_slice_1d::<i16, _>(selection.clone())
            .map(|values| {
                values
                    .into_iter()
                    .map(|value| value as f64)
                    .collect::<Vec<_>>()
            }),
        TypeDescriptor::Integer(IntSize::U4) => dataset
            .read_slice_1d::<i32, _>(selection.clone())
            .map(|values| {
                values
                    .into_iter()
                    .map(|value| value as f64)
                    .collect::<Vec<_>>()
            }),
        TypeDescriptor::Integer(IntSize::U8) => dataset
            .read_slice_1d::<i64, _>(selection.clone())
            .map(|values| {
                values
                    .into_iter()
                    .map(|value| value as f64)
                    .collect::<Vec<_>>()
            }),
        TypeDescriptor::Unsigned(IntSize::U1) => dataset
            .read_slice_1d::<u8, _>(selection.clone())
            .map(|values| {
                values
                    .into_iter()
                    .map(|value| value as f64)
                    .collect::<Vec<_>>()
            }),
        TypeDescriptor::Unsigned(IntSize::U2) => dataset
            .read_slice_1d::<u16, _>(selection.clone())
            .map(|values| {
                values
                    .into_iter()
                    .map(|value| value as f64)
                    .collect::<Vec<_>>()
            }),
        TypeDescriptor::Unsigned(IntSize::U4) => dataset
            .read_slice_1d::<u32, _>(selection.clone())
            .map(|values| {
                values
                    .into_iter()
                    .map(|value| value as f64)
                    .collect::<Vec<_>>()
            }),
        TypeDescriptor::Unsigned(IntSize::U8) => dataset
            .read_slice_1d::<u64, _>(selection.clone())
            .map(|values| {
                values
                    .into_iter()
                    .map(|value| value as f64)
                    .collect::<Vec<_>>()
            }),
        TypeDescriptor::Float(FloatSize::U4) => dataset
            .read_slice_1d::<f32, _>(selection.clone())
            .map(|values| {
                values
                    .into_iter()
                    .map(|value| value as f64)
                    .collect::<Vec<_>>()
            }),
        TypeDescriptor::Float(FloatSize::U8) => dataset
            .read_slice_1d::<f64, _>(selection.clone())
            .map(|values| values.into_iter().collect::<Vec<_>>()),
        TypeDescriptor::Boolean => dataset.read_slice_1d::<bool, _>(selection).map(|values| {
            values
                .into_iter()
                .map(|value| if value { 1.0 } else { 0.0 })
                .collect::<Vec<_>>()
        }),
        other => {
            return Err(format!(
                "Series reference {} must be numeric; got {}",
                series_ref.render(),
                other
            ))
        }
    }
    .map_err(|error| format!("Failed reading {}: {}", series_ref.render(), error))?;

    let points = values
        .into_iter()
        .enumerate()
        .map(|(idx, value)| (idx as f64, value))
        .collect::<Vec<_>>();
    if points.is_empty() {
        return Err(format!(
            "Series reference {} resolved to an empty series",
            series_ref.render()
        ));
    }
    Ok(points)
}

fn preview_selection_to_hyperslab(
    shape: &[usize],
    selection: &PreviewSelection,
) -> Result<Selection, String> {
    validate_preview_selection_shape(shape, selection).map_err(|error| error.to_string())?;
    let slice = match selection.slice {
        SliceSelection::All => 0..shape[selection.x],
        SliceSelection::FromTo(a, b) => a..b,
    };

    let mut slice_selections = Vec::new();
    for idx in 0..shape.len() {
        if idx == selection.x {
            slice_selections.push(SliceOrIndex::SliceTo {
                start: slice.start,
                step: 1,
                end: slice.end,
                block: 1,
            });
        } else {
            slice_selections.push(SliceOrIndex::Index(selection.index[idx]));
        }
    }

    Ok(Selection::Hyperslab(Hyperslab::from(slice_selections)))
}

fn resolve_expression_scalar_value(
    state: &MultiChartState,
    file: &File,
    scalar_ref: &ExpressionScalarRef,
) -> Result<f64, String> {
    let object_path =
        resolve_expression_target_path(state, &scalar_ref.target, &scalar_ref.render())?;
    match &scalar_ref.attr_name {
        Some(attr_name) => {
            let attr = open_expression_attribute(file, &object_path, attr_name)?;
            read_expression_numeric_scalar_attr(&attr, &scalar_ref.render())
        }
        None => {
            let dataset = file.dataset(&object_path).map_err(|error| {
                format!(
                    "Scalar reference {} could not open dataset '{}': {}",
                    scalar_ref.render(),
                    object_path,
                    error
                )
            })?;
            read_expression_numeric_scalar_dataset(&dataset, &scalar_ref.render())
        }
    }
}

fn normalize_absolute_object_path(path: &str) -> Result<String, String> {
    if !path.starts_with('/') {
        return Err(format!("Absolute path '{path}' must start with '/'"));
    }
    let mut components = Vec::new();
    for segment in path.split('/') {
        if segment.is_empty() {
            continue;
        }
        match segment {
            "." => {}
            ".." => {
                if components.pop().is_none() {
                    return Err(format!("Absolute path '{path}' escapes above root"));
                }
            }
            other => components.push(other),
        }
    }
    if components.is_empty() {
        Ok("/".to_string())
    } else {
        Ok(format!("/{}", components.join("/")))
    }
}

fn resolve_expression_target_path(
    state: &MultiChartState,
    target: &ExpressionObjectTarget,
    reference: &str,
) -> Result<String, String> {
    match target {
        ExpressionObjectTarget::AbsolutePath(path) => normalize_absolute_object_path(path),
        ExpressionObjectTarget::ItemRef(id) => state
            .item_by_id(*id)
            .and_then(|item| item.source.dataset_source())
            .map(|dataset_source| dataset_source.dataset_path.clone())
            .ok_or_else(|| {
                format!(
                    "Reference {} requires chart item ${} to be dataset-backed",
                    reference, id.0
                )
            }),
    }
}

fn open_expression_attribute(
    file: &File,
    object_path: &str,
    attr_name: &str,
) -> Result<Attribute, String> {
    if object_path == "/" {
        return file
            .attr(attr_name)
            .map_err(|error| format!("Failed to read attribute '#/:{}': {}", attr_name, error));
    }

    if let Ok(group) = file.group(object_path) {
        return group.attr(attr_name).map_err(|error| {
            format!(
                "Failed to read attribute '#{}:{}': {}",
                object_path, attr_name, error
            )
        });
    }

    if let Ok(dataset) = file.dataset(object_path) {
        return dataset.attr(attr_name).map_err(|error| {
            format!(
                "Failed to read attribute '#{}:{}': {}",
                object_path, attr_name, error
            )
        });
    }

    Err(format!(
        "Attribute path '{}' does not resolve to a dataset or group in the file",
        object_path
    ))
}

fn read_expression_numeric_scalar_attr(attr: &Attribute, reference: &str) -> Result<f64, String> {
    if !attr.is_scalar() {
        return Err(format!(
            "Attribute reference {reference} must resolve to a scalar numeric attribute"
        ));
    }
    let dtype = attr.dtype().map_err(|error| {
        format!("Failed to inspect scalar attribute type for {reference}: {error}")
    })?;
    let type_desc = dtype.to_descriptor().map_err(|error| {
        format!("Failed to inspect scalar attribute type for {reference}: {error}")
    })?;
    match type_desc {
        TypeDescriptor::Integer(IntSize::U1) => attr
            .read_scalar::<i8>()
            .map(|value| value as f64)
            .map_err(|error| format!("Failed reading {reference}: {error}")),
        TypeDescriptor::Integer(IntSize::U2) => attr
            .read_scalar::<i16>()
            .map(|value| value as f64)
            .map_err(|error| format!("Failed reading {reference}: {error}")),
        TypeDescriptor::Integer(IntSize::U4) => attr
            .read_scalar::<i32>()
            .map(|value| value as f64)
            .map_err(|error| format!("Failed reading {reference}: {error}")),
        TypeDescriptor::Integer(IntSize::U8) => attr
            .read_scalar::<i64>()
            .map(|value| value as f64)
            .map_err(|error| format!("Failed reading {reference}: {error}")),
        TypeDescriptor::Unsigned(IntSize::U1) => attr
            .read_scalar::<u8>()
            .map(|value| value as f64)
            .map_err(|error| format!("Failed reading {reference}: {error}")),
        TypeDescriptor::Unsigned(IntSize::U2) => attr
            .read_scalar::<u16>()
            .map(|value| value as f64)
            .map_err(|error| format!("Failed reading {reference}: {error}")),
        TypeDescriptor::Unsigned(IntSize::U4) => attr
            .read_scalar::<u32>()
            .map(|value| value as f64)
            .map_err(|error| format!("Failed reading {reference}: {error}")),
        TypeDescriptor::Unsigned(IntSize::U8) => attr
            .read_scalar::<u64>()
            .map(|value| value as f64)
            .map_err(|error| format!("Failed reading {reference}: {error}")),
        TypeDescriptor::Float(FloatSize::U4) => attr
            .read_scalar::<f32>()
            .map(|value| value as f64)
            .map_err(|error| format!("Failed reading {reference}: {error}")),
        TypeDescriptor::Float(FloatSize::U8) => attr
            .read_scalar::<f64>()
            .map_err(|error| format!("Failed reading {reference}: {error}")),
        other => Err(format!(
            "Attribute reference {reference} must be numeric; got {other}"
        )),
    }
}

fn read_expression_numeric_series_attr(
    attr: &Attribute,
    reference: &str,
) -> Result<Vec<Point>, String> {
    if attr.is_scalar() {
        return Err(format!(
            "Series reference {reference} must resolve to a non-scalar numeric attribute"
        ));
    }
    if attr.shape().len() != 1 {
        return Err(format!(
            "Series reference {reference} currently supports only rank-1 numeric attributes"
        ));
    }

    let dtype = attr.dtype().map_err(|error| {
        format!("Failed to inspect series attribute type for {reference}: {error}")
    })?;
    let type_desc = dtype.to_descriptor().map_err(|error| {
        format!("Failed to inspect series attribute type for {reference}: {error}")
    })?;
    let values = match type_desc {
        TypeDescriptor::Integer(IntSize::U1) => attr.read_1d::<i8>().map(|values| {
            values
                .into_iter()
                .map(|value| value as f64)
                .collect::<Vec<_>>()
        }),
        TypeDescriptor::Integer(IntSize::U2) => attr.read_1d::<i16>().map(|values| {
            values
                .into_iter()
                .map(|value| value as f64)
                .collect::<Vec<_>>()
        }),
        TypeDescriptor::Integer(IntSize::U4) => attr.read_1d::<i32>().map(|values| {
            values
                .into_iter()
                .map(|value| value as f64)
                .collect::<Vec<_>>()
        }),
        TypeDescriptor::Integer(IntSize::U8) => attr.read_1d::<i64>().map(|values| {
            values
                .into_iter()
                .map(|value| value as f64)
                .collect::<Vec<_>>()
        }),
        TypeDescriptor::Unsigned(IntSize::U1) => attr.read_1d::<u8>().map(|values| {
            values
                .into_iter()
                .map(|value| value as f64)
                .collect::<Vec<_>>()
        }),
        TypeDescriptor::Unsigned(IntSize::U2) => attr.read_1d::<u16>().map(|values| {
            values
                .into_iter()
                .map(|value| value as f64)
                .collect::<Vec<_>>()
        }),
        TypeDescriptor::Unsigned(IntSize::U4) => attr.read_1d::<u32>().map(|values| {
            values
                .into_iter()
                .map(|value| value as f64)
                .collect::<Vec<_>>()
        }),
        TypeDescriptor::Unsigned(IntSize::U8) => attr.read_1d::<u64>().map(|values| {
            values
                .into_iter()
                .map(|value| value as f64)
                .collect::<Vec<_>>()
        }),
        TypeDescriptor::Float(FloatSize::U4) => attr.read_1d::<f32>().map(|values| {
            values
                .into_iter()
                .map(|value| value as f64)
                .collect::<Vec<_>>()
        }),
        TypeDescriptor::Float(FloatSize::U8) => attr
            .read_1d::<f64>()
            .map(|values| values.into_iter().collect::<Vec<_>>()),
        other => {
            return Err(format!(
                "Series reference {reference} must be numeric; got {other}"
            ))
        }
    }
    .map_err(|error| format!("Failed reading {reference}: {error}"))?;

    if values.is_empty() {
        return Err(format!(
            "Series reference {reference} resolved to an empty series"
        ));
    }
    Ok(values
        .into_iter()
        .enumerate()
        .map(|(idx, value)| (idx as f64, value))
        .collect())
}

fn read_expression_numeric_scalar_dataset(
    dataset: &Dataset,
    reference: &str,
) -> Result<f64, String> {
    if !dataset.is_scalar() {
        return Err(format!(
            "Scalar reference {reference} must resolve to a scalar numeric dataset"
        ));
    }

    let dtype = dataset.dtype().map_err(|error| {
        format!("Failed to inspect scalar dataset type for {reference}: {error}")
    })?;
    let type_desc = dtype.to_descriptor().map_err(|error| {
        format!("Failed to inspect scalar dataset type for {reference}: {error}")
    })?;
    match type_desc {
        TypeDescriptor::Integer(IntSize::U1) => dataset
            .read_scalar::<i8>()
            .map(|value| value as f64)
            .map_err(|error| format!("Failed reading {reference}: {error}")),
        TypeDescriptor::Integer(IntSize::U2) => dataset
            .read_scalar::<i16>()
            .map(|value| value as f64)
            .map_err(|error| format!("Failed reading {reference}: {error}")),
        TypeDescriptor::Integer(IntSize::U4) => dataset
            .read_scalar::<i32>()
            .map(|value| value as f64)
            .map_err(|error| format!("Failed reading {reference}: {error}")),
        TypeDescriptor::Integer(IntSize::U8) => dataset
            .read_scalar::<i64>()
            .map(|value| value as f64)
            .map_err(|error| format!("Failed reading {reference}: {error}")),
        TypeDescriptor::Unsigned(IntSize::U1) => dataset
            .read_scalar::<u8>()
            .map(|value| value as f64)
            .map_err(|error| format!("Failed reading {reference}: {error}")),
        TypeDescriptor::Unsigned(IntSize::U2) => dataset
            .read_scalar::<u16>()
            .map(|value| value as f64)
            .map_err(|error| format!("Failed reading {reference}: {error}")),
        TypeDescriptor::Unsigned(IntSize::U4) => dataset
            .read_scalar::<u32>()
            .map(|value| value as f64)
            .map_err(|error| format!("Failed reading {reference}: {error}")),
        TypeDescriptor::Unsigned(IntSize::U8) => dataset
            .read_scalar::<u64>()
            .map(|value| value as f64)
            .map_err(|error| format!("Failed reading {reference}: {error}")),
        TypeDescriptor::Float(FloatSize::U4) => dataset
            .read_scalar::<f32>()
            .map(|value| value as f64)
            .map_err(|error| format!("Failed reading {reference}: {error}")),
        TypeDescriptor::Float(FloatSize::U8) => dataset
            .read_scalar::<f64>()
            .map_err(|error| format!("Failed reading {reference}: {error}")),
        other => Err(format!(
            "Scalar reference {reference} must be numeric; got {other}"
        )),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::expect_used)]
#[allow(clippy::panic)]
mod tests {
    use super::*;
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    use hdf5_metno::File;
    use ndarray::Array;

    #[allow(deprecated)]
    fn make_state() -> MultiChartState {
        MultiChartState::new(Picker::from_fontsize((7, 14)))
    }

    fn source(path: &str, selection: PreviewSelection) -> ChartSource {
        ChartSource::DatasetSelection(DatasetChartSource {
            dataset_path: "/raw/ds".to_string(),
            display_path: path.to_string(),
            selection,
            shape: vec![4, 8],
            kind: DatasetChartKind::Dataset,
        })
    }

    fn temp_hdf5_path(name: &str) -> std::path::PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time before unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("h5v-{name}-{unique}.h5"))
    }

    fn make_attribute_test_file() -> (File, std::path::PathBuf) {
        let path = temp_hdf5_path("mchart-attr");
        let file = File::create(&path).expect("failed creating temp hdf5 file");
        let parent = file
            .create_group("parent")
            .expect("failed creating parent group");
        let offset_attr = parent
            .new_attr_builder()
            .empty::<f64>()
            .create("OFFSET")
            .expect("failed creating parent attr");
        offset_attr
            .write_scalar(&3.0_f64)
            .expect("failed writing parent attr");
        let child = parent
            .create_group("child")
            .expect("failed creating child group");
        let child_offset_attr = child
            .new_attr_builder()
            .empty::<f64>()
            .create("CHILD_OFFSET")
            .expect("failed creating child attr");
        child_offset_attr
            .write_scalar(&3.0_f64)
            .expect("failed writing child attr");
        let dataset = child
            .new_dataset_builder()
            .with_data(&[1.0_f64, 2.0_f64])
            .create("ds")
            .expect("failed creating dataset");
        let scale_attr = dataset
            .new_attr_builder()
            .empty::<f64>()
            .create("SCALE")
            .expect("failed creating dataset attr");
        scale_attr
            .write_scalar(&2.0_f64)
            .expect("failed writing dataset attr");
        let flag_attr = dataset
            .new_attr_builder()
            .empty::<bool>()
            .create("FLAG")
            .expect("failed creating non numeric attr");
        flag_attr
            .write_scalar(&true)
            .expect("failed writing non numeric attr");
        dataset
            .new_attr_builder()
            .with_data(&[4.0_f64, 8.0_f64])
            .create("TRACE")
            .expect("failed creating series attr");
        let other = parent
            .new_dataset_builder()
            .with_data(&[0.0_f64])
            .create("otherds")
            .expect("failed creating other dataset");
        let bias_attr = other
            .new_attr_builder()
            .empty::<f64>()
            .create("BIAS")
            .expect("failed creating other dataset attr");
        bias_attr
            .write_scalar(&5.0_f64)
            .expect("failed writing other dataset attr");
        let scalar = parent
            .new_dataset_builder()
            .empty::<f64>()
            .create("scalar")
            .expect("failed creating scalar dataset");
        scalar
            .write_scalar(&7.0_f64)
            .expect("failed writing scalar dataset");
        file.flush().expect("failed flushing temp hdf5 file");
        (file, path)
    }

    fn make_dataset_ref_test_file() -> (File, std::path::PathBuf) {
        let path = temp_hdf5_path("mchart-dataset-ref");
        let file = File::create(&path).expect("failed creating temp hdf5 file");
        file.new_dataset_builder()
            .with_data(&[2.0_f64, 4.0_f64, 6.0_f64])
            .create("series")
            .expect("failed creating 1d dataset");
        let matrix = Array::from_shape_vec((3, 2), vec![10.0_f64, 1.0, 20.0, 2.0, 30.0, 3.0])
            .expect("failed creating matrix test array");
        file.new_dataset_builder()
            .with_data(matrix.view())
            .create("matrix")
            .expect("failed creating 2d dataset");
        let scalar = file
            .new_dataset_builder()
            .empty::<f64>()
            .create("scalar")
            .expect("failed creating scalar dataset");
        scalar
            .write_scalar(&1.5_f64)
            .expect("failed writing scalar dataset");
        file.flush().expect("failed flushing temp hdf5 file");
        (file, path)
    }

    #[test]
    fn reuses_exact_source_and_adds_distinct_selection_variants() {
        let mut state = make_state();
        let first_selection = PreviewSelection {
            index: vec![0, 0],
            x: 1,
            slice: SliceSelection::All,
        };
        let second_selection = PreviewSelection {
            index: vec![1, 0],
            x: 1,
            slice: SliceSelection::All,
        };

        state.add_chart_item(
            source("/group/ds", first_selection.clone()),
            vec![(0.0, 1.0), (1.0, 2.0)],
        );
        state.add_chart_item(
            source("/group/ds", first_selection),
            vec![(0.0, 3.0), (1.0, 4.0)],
        );
        state.add_chart_item(
            source("/group/ds", second_selection),
            vec![(0.0, 5.0), (1.0, 6.0)],
        );

        assert_eq!(state.chart_items().len(), 2);
        assert_eq!(state.source_item_count("/group/ds"), 2);
        assert_eq!(state.chart_items()[0].series.len(), 2);
        assert_eq!(state.chart_items()[0].series.y_max, 4.0);
    }

    #[test]
    fn creates_builtin_difference_from_marked_base_and_selected_item() {
        let mut state = make_state();
        let first = PreviewSelection {
            index: vec![0, 0],
            x: 1,
            slice: SliceSelection::All,
        };
        let second = PreviewSelection {
            index: vec![1, 0],
            x: 1,
            slice: SliceSelection::All,
        };

        state.add_chart_item(source("/group/a", first), vec![(0.0, 5.0), (1.0, 7.0)]);
        state.add_chart_item(source("/group/b", second), vec![(0.0, 2.0), (1.0, 3.0)]);
        state.move_up();
        state.toggle_marked_base().unwrap();
        state.move_down();

        state
            .create_builtin_derived(BuiltinDerivedOp::Difference)
            .unwrap();

        assert_eq!(state.chart_items().len(), 3);
        let derived = state.chart_items().last().unwrap();
        assert_eq!(derived.series.len(), 2);
        assert_eq!(derived.series.y_min, 3.0);
        assert_eq!(derived.series.y_max, 4.0);
        match &derived.source {
            ChartSource::BuiltinDerived(source) => {
                assert_eq!(source.operation, BuiltinDerivedOp::Difference);
                assert_eq!(source.aligned_len, 2);
            }
            other => panic!("expected builtin derived source, got {other:?}"),
        }
    }

    #[test]
    fn builtin_derived_truncates_to_shorter_input_length() {
        let mut state = make_state();
        let selection = PreviewSelection {
            index: vec![0, 0],
            x: 1,
            slice: SliceSelection::All,
        };

        state.add_chart_item(
            source("/group/a", selection.clone()),
            vec![(0.0, 1.0), (1.0, 2.0), (2.0, 3.0)],
        );
        state.add_chart_item(source("/group/b", selection), vec![(0.0, 4.0), (1.0, 5.0)]);
        state.move_up();
        state.toggle_marked_base().unwrap();
        state.move_down();

        state.create_builtin_derived(BuiltinDerivedOp::Sum).unwrap();

        let derived = state.chart_items().last().unwrap();
        match &derived.source {
            ChartSource::BuiltinDerived(source) => {
                assert_eq!(source.aligned_len, 2);
                assert_eq!(source.lhs_len, 3);
                assert_eq!(source.rhs_len, 2);
            }
            other => panic!("expected builtin derived source, got {other:?}"),
        }
        assert_eq!(derived.series.len(), 2);
    }

    #[test]
    fn builtin_ratio_errors_on_zero_divisor() {
        let mut state = make_state();
        let selection = PreviewSelection {
            index: vec![0, 0],
            x: 1,
            slice: SliceSelection::All,
        };

        state.add_chart_item(source("/group/a", selection.clone()), vec![(0.0, 4.0)]);
        state.add_chart_item(source("/group/b", selection), vec![(0.0, 0.0)]);
        state.move_up();
        state.toggle_marked_base().unwrap();
        state.move_down();

        let err = state
            .create_builtin_derived(BuiltinDerivedOp::Ratio)
            .unwrap_err();
        assert!(err.contains("divisor is zero"));
    }

    #[test]
    fn builtin_xy_uses_base_values_as_x_and_selected_as_y() {
        let mut state = make_state();
        let selection = PreviewSelection {
            index: vec![0, 0],
            x: 1,
            slice: SliceSelection::All,
        };

        state.add_chart_item(
            source("/group/x", selection.clone()),
            vec![(0.0, 10.0), (1.0, 20.0)],
        );
        state.add_chart_item(source("/group/y", selection), vec![(0.0, 1.5), (1.0, 2.5)]);
        state.move_up();
        state.toggle_marked_base().unwrap();
        state.move_down();

        state.create_builtin_derived(BuiltinDerivedOp::Xy).unwrap();

        let derived = state.chart_items().last().unwrap();
        assert_eq!(derived.series.points, vec![(10.0, 1.5), (20.0, 2.5)]);
        match &derived.source {
            ChartSource::BuiltinDerived(source) => {
                assert_eq!(source.operation, BuiltinDerivedOp::Xy);
                assert_eq!(source.aligned_len, 2);
            }
            other => panic!("expected builtin derived source, got {other:?}"),
        }
    }

    #[test]
    fn builtin_xy_requires_matching_lengths() {
        let mut state = make_state();
        let selection = PreviewSelection {
            index: vec![0, 0],
            x: 1,
            slice: SliceSelection::All,
        };

        state.add_chart_item(
            source("/group/x", selection.clone()),
            vec![(0.0, 10.0), (1.0, 20.0), (2.0, 30.0)],
        );
        state.add_chart_item(source("/group/y", selection), vec![(0.0, 1.5), (1.0, 2.5)]);
        state.move_up();
        state.toggle_marked_base().unwrap();
        state.move_down();

        let err = state
            .create_builtin_derived(BuiltinDerivedOp::Xy)
            .unwrap_err();
        assert!(err.contains("does not match"));
    }

    #[test]
    fn expression_derived_supports_item_refs_literals_and_precedence() {
        let mut state = make_state();
        let selection = PreviewSelection {
            index: vec![0, 0],
            x: 1,
            slice: SliceSelection::All,
        };

        state.add_chart_item(
            source("/group/a", selection.clone()),
            vec![(0.0, 2.0), (1.0, 4.0)],
        );
        state.add_chart_item(source("/group/b", selection), vec![(0.0, 3.0), (1.0, 5.0)]);

        state
            .create_expression_derived("$1 + $2 * 2".to_string())
            .unwrap();

        let derived = state.chart_items().last().unwrap();
        assert_eq!(derived.series.points, vec![(0.0, 8.0), (1.0, 14.0)]);
        match &derived.source {
            ChartSource::DerivedExpression { input_ids, len, .. } => {
                assert_eq!(input_ids, &vec![ChartItemId(1), ChartItemId(2)]);
                assert_eq!(*len, 2);
            }
            other => panic!("expected expression-derived source, got {other:?}"),
        }
    }

    #[test]
    fn expression_derived_rejects_mismatched_x_values() {
        let mut state = make_state();
        let selection = PreviewSelection {
            index: vec![0, 0],
            x: 1,
            slice: SliceSelection::All,
        };

        state.add_chart_item(
            source("/group/a", selection.clone()),
            vec![(0.0, 2.0), (1.0, 4.0)],
        );
        state.add_chart_item(
            source("/group/b", selection),
            vec![(10.0, 3.0), (20.0, 5.0)],
        );

        let err = state
            .create_expression_derived("$1 + $2".to_string())
            .unwrap_err();
        assert!(err.contains("x-values must match"));
    }

    #[test]
    fn tokenizes_explicit_scalar_references() {
        let tokens =
            tokenize_expression("$1 * #/parent/scalar + #/parent/otherds:BIAS + #$1:SCALE")
                .unwrap();
        assert!(tokens.iter().any(|token| matches!(
            token,
            ExpressionToken::ScalarRef(ExpressionScalarRef {
                target: ExpressionObjectTarget::AbsolutePath(path),
                attr_name: None,
            }) if path == "/parent/scalar"
        )));
        assert!(tokens.iter().any(|token| matches!(
            token,
            ExpressionToken::ScalarRef(ExpressionScalarRef {
                target: ExpressionObjectTarget::AbsolutePath(path),
                attr_name: Some(attr_name),
            }) if path == "/parent/otherds" && attr_name == "BIAS"
        )));
        assert!(tokens.iter().any(|token| matches!(
            token,
            ExpressionToken::ScalarRef(ExpressionScalarRef {
                target: ExpressionObjectTarget::ItemRef(ChartItemId(1)),
                attr_name: Some(attr_name),
            }) if attr_name == "SCALE"
        )));
    }

    #[test]
    fn tokenizes_explicit_series_references() {
        let tokens = tokenize_expression("!/series + !/matrix[.., 1] + !$1:TRACE").unwrap();
        assert!(tokens.iter().any(|token| matches!(
            token,
            ExpressionToken::SeriesRef(ExpressionSeriesRef {
                target: ExpressionObjectTarget::AbsolutePath(path),
                attr_name: None,
                selectors: None,
            }) if path == "/series"
        )));
        assert!(tokens.iter().any(|token| matches!(
            token,
            ExpressionToken::SeriesRef(ExpressionSeriesRef {
                target: ExpressionObjectTarget::AbsolutePath(path),
                attr_name: None,
                selectors: Some(selectors),
            }) if path == "/matrix"
                    && selectors
                        == &vec![
                            ExpressionDatasetSelector::All,
                            ExpressionDatasetSelector::Index(1),
                        ]
        )));
        assert!(tokens.iter().any(|token| matches!(
            token,
            ExpressionToken::SeriesRef(ExpressionSeriesRef {
                target: ExpressionObjectTarget::ItemRef(ChartItemId(1)),
                attr_name: Some(attr_name),
                selectors: None,
            }) if attr_name == "TRACE"
        )));
    }

    #[test]
    fn dataset_path_reference_builds_preview_selection() {
        let dataset_ref = ExpressionSeriesRef {
            target: ExpressionObjectTarget::AbsolutePath("/matrix".to_string()),
            attr_name: None,
            selectors: Some(vec![
                ExpressionDatasetSelector::Index(1),
                ExpressionDatasetSelector::All,
                ExpressionDatasetSelector::Index(2),
                ExpressionDatasetSelector::Index(3),
            ]),
        };
        let selection = dataset_ref.to_preview_selection(&[4, 5, 6, 7]).unwrap();
        assert_eq!(selection.x, 1);
        assert_eq!(selection.index, vec![1, 0, 2, 3]);
    }

    #[test]
    fn dataset_path_reference_requires_exactly_one_axis_selector() {
        let dataset_ref = ExpressionSeriesRef {
            target: ExpressionObjectTarget::AbsolutePath("/matrix".to_string()),
            attr_name: None,
            selectors: Some(vec![
                ExpressionDatasetSelector::Index(0),
                ExpressionDatasetSelector::Index(1),
            ]),
        };
        let err = dataset_ref.to_preview_selection(&[3, 4]).unwrap_err();
        assert!(err.contains("exactly one '..'"));
    }

    #[test]
    fn parses_top_level_xy_expression_tuple() {
        let tokens = tokenize_expression("($1 * 2, $2 + #/calibration/offset)").unwrap();
        let parsed = parse_derived_expression(&tokens).unwrap();
        match parsed {
            ParsedExpression::XySeries(_, _) => {}
            other => panic!("expected xy parsed expression, got {other:?}"),
        }
    }

    #[test]
    fn normalizes_absolute_expression_paths() {
        assert_eq!(
            normalize_absolute_object_path("/parent/otherds").unwrap(),
            "/parent/otherds"
        );
        assert!(normalize_absolute_object_path("/../../../../").is_err());
    }

    #[test]
    fn rejects_implicit_context_scalar_attributes() {
        let err = tokenize_expression("$1 + #SCALE").unwrap_err();
        assert!(err.contains("Scalar references must use an absolute path"));
    }

    #[test]
    fn expression_prompt_edits_do_not_invalidate_chart_render() {
        let mut state = make_state();
        state.modified = false;
        state.open_expression_prompt();
        assert!(state.modified);

        state.modified = false;
        state.expression_insert_char('x');
        assert!(!state.modified);

        state.expression_move_left();
        assert!(!state.modified);

        state.expression_backspace();
        assert!(!state.modified);
    }

    #[test]
    fn expression_derived_supports_dataset_path_series_inputs() {
        let (file, path) = make_dataset_ref_test_file();
        let mut state = make_state();

        state
            .create_expression_derived_with_file(
                "!/series + !/matrix[..,1]".to_string(),
                Some(&file),
            )
            .unwrap();

        let derived = state.chart_items().last().unwrap();
        assert_eq!(
            derived.series.points,
            vec![(0.0, 3.0), (1.0, 6.0), (2.0, 9.0)]
        );

        drop(file);
        fs::remove_file(path).expect("failed removing temp hdf5 file");
    }

    #[test]
    fn expression_derived_supports_scalar_dataset_inputs() {
        let (file, path) = make_dataset_ref_test_file();
        let mut state = make_state();

        state
            .create_expression_derived_with_file("!/series + #/scalar".to_string(), Some(&file))
            .unwrap();

        let derived = state.chart_items().last().unwrap();
        assert_eq!(
            derived.series.points,
            vec![(0.0, 3.5), (1.0, 5.5), (2.0, 7.5)]
        );

        drop(file);
        fs::remove_file(path).expect("failed removing temp hdf5 file");
    }

    #[test]
    fn expression_derived_dataset_path_refs_validate_series_lengths() {
        let (file, path) = make_dataset_ref_test_file();
        let mut state = make_state();

        let err = state
            .create_expression_derived_with_file(
                "!/series + !/matrix[1,..]".to_string(),
                Some(&file),
            )
            .unwrap_err();
        assert!(err.contains("lengths must match"));

        drop(file);
        fs::remove_file(path).expect("failed removing temp hdf5 file");
    }

    #[test]
    fn expression_derived_xy_tuple_creates_xy_series() {
        let mut state = make_state();
        let selection = PreviewSelection {
            index: vec![0, 0],
            x: 1,
            slice: SliceSelection::All,
        };

        state.add_chart_item(
            source("/group/a", selection.clone()),
            vec![(10.0, 2.0), (20.0, 4.0)],
        );
        state.add_chart_item(
            source("/group/b", selection),
            vec![(100.0, 3.0), (200.0, 5.0)],
        );

        state
            .create_expression_derived("($1 * 10, $2 + 1)".to_string())
            .unwrap();

        let derived = state.chart_items().last().unwrap();
        assert_eq!(derived.series.points, vec![(20.0, 4.0), (40.0, 6.0)]);
        match &derived.source {
            ChartSource::DerivedExpression {
                input_ids,
                len,
                kind,
                ..
            } => {
                assert_eq!(input_ids, &vec![ChartItemId(1), ChartItemId(2)]);
                assert_eq!(*len, 2);
                assert_eq!(*kind, DerivedExpressionKind::XySeries);
            }
            other => panic!("expected expression-derived source, got {other:?}"),
        }
    }

    #[test]
    fn expression_derived_xy_tuple_requires_matching_lengths() {
        let mut state = make_state();
        let selection = PreviewSelection {
            index: vec![0, 0],
            x: 1,
            slice: SliceSelection::All,
        };

        state.add_chart_item(
            source("/group/a", selection.clone()),
            vec![(0.0, 2.0), (1.0, 4.0), (2.0, 6.0)],
        );
        state.add_chart_item(source("/group/b", selection), vec![(0.0, 3.0), (1.0, 5.0)]);

        let err = state
            .create_expression_derived("($1, $2 + 1)".to_string())
            .unwrap_err();
        assert!(err.contains("lengths must match"));
    }

    #[test]
    #[ignore = "real HDF5 attribute reads are unstable in the default parallel test environment"]
    fn expression_derived_supports_scalar_attributes_from_dataset_and_paths() {
        let (file, path) = make_attribute_test_file();
        let mut state = make_state();
        let selection = PreviewSelection {
            index: vec![0],
            x: 0,
            slice: SliceSelection::All,
        };

        state.add_chart_item(
            ChartSource::DatasetSelection(DatasetChartSource {
                dataset_path: "/parent/child/ds".to_string(),
                display_path: "/parent/child/ds".to_string(),
                selection,
                shape: vec![2],
                kind: DatasetChartKind::Dataset,
            }),
            vec![(0.0, 1.0), (1.0, 2.0)],
        );

        state
            .create_expression_derived_with_file(
                "$1 * #$1:SCALE + #/parent/child:CHILD_OFFSET + #/parent/otherds:BIAS + #/parent/scalar"
                    .to_string(),
                Some(&file),
            )
            .unwrap();

        let derived = state.chart_items().last().unwrap();
        assert_eq!(derived.series.points, vec![(0.0, 17.0), (1.0, 19.0)]);

        drop(file);
        fs::remove_file(path).expect("failed removing temp hdf5 file");
    }

    #[test]
    #[ignore = "real HDF5 attribute reads are unstable in the default parallel test environment"]
    fn expression_derived_rejects_non_numeric_scalar_attribute() {
        let (file, path) = make_attribute_test_file();
        let mut state = make_state();
        let selection = PreviewSelection {
            index: vec![0],
            x: 0,
            slice: SliceSelection::All,
        };

        state.add_chart_item(
            ChartSource::DatasetSelection(DatasetChartSource {
                dataset_path: "/parent/child/ds".to_string(),
                display_path: "/parent/child/ds".to_string(),
                selection,
                shape: vec![2],
                kind: DatasetChartKind::Dataset,
            }),
            vec![(0.0, 1.0), (1.0, 2.0)],
        );

        let err = state
            .create_expression_derived_with_file("$1 + #$1:FLAG".to_string(), Some(&file))
            .unwrap_err();
        assert!(err.contains("must be numeric"));

        drop(file);
        fs::remove_file(path).expect("failed removing temp hdf5 file");
    }

    #[test]
    #[ignore = "real HDF5 attribute reads are unstable in the default parallel test environment"]
    fn expression_derived_supports_series_attributes_on_dataset_items() {
        let (file, path) = make_attribute_test_file();
        let mut state = make_state();
        let selection = PreviewSelection {
            index: vec![0],
            x: 0,
            slice: SliceSelection::All,
        };

        state.add_chart_item(
            ChartSource::DatasetSelection(DatasetChartSource {
                dataset_path: "/parent/child/ds".to_string(),
                display_path: "/parent/child/ds".to_string(),
                selection,
                shape: vec![2],
                kind: DatasetChartKind::Dataset,
            }),
            vec![(0.0, 1.0), (1.0, 2.0)],
        );

        state
            .create_expression_derived_with_file(
                "!$1:TRACE + #/parent/scalar".to_string(),
                Some(&file),
            )
            .unwrap();

        let derived = state.chart_items().last().unwrap();
        assert_eq!(derived.series.points, vec![(0.0, 11.0), (1.0, 15.0)]);

        drop(file);
        fs::remove_file(path).expect("failed removing temp hdf5 file");
    }

    #[test]
    fn zoom_in_anchor_ratio_biases_toward_hovered_side() {
        let mut state = make_state();
        let selection = PreviewSelection {
            index: vec![0],
            x: 0,
            slice: SliceSelection::All,
        };
        state.add_chart_item(
            source("/group/a", selection),
            (0..100).map(|i| (i as f64, i as f64)).collect(),
        );

        state.zoom_with_anchor_ratio(10.0, 0.0, true);
        assert_eq!(state.aoi_from, None);
        assert_eq!(state.aoi_to, Some(80));

        state.clear_zoom();
        state.zoom_with_anchor_ratio(10.0, 1.0, true);
        assert_eq!(state.aoi_from, Some(20));
        assert_eq!(state.aoi_to, None);

        state.clear_zoom();
        state.zoom_with_anchor_ratio(10.0, 0.5, true);
        assert_eq!(state.aoi_from, Some(10));
        assert_eq!(state.aoi_to, Some(90));
    }

    #[test]
    fn zoom_at_position_only_applies_inside_chart_area() {
        let mut state = make_state();
        let selection = PreviewSelection {
            index: vec![0],
            x: 0,
            slice: SliceSelection::All,
        };
        state.add_chart_item(
            source("/group/a", selection),
            (0..100).map(|i| (i as f64, i as f64)).collect(),
        );
        state.last_chart_area = Some(Rect::new(10, 5, 20, 8));

        assert!(!state.zoom_in_at_position(5, 6, 10.0));
        assert_eq!(state.aoi_from, None);
        assert_eq!(state.aoi_to, None);

        assert!(state.zoom_in_at_position(10, 6, 10.0));
        assert_eq!(state.aoi_from, None);
        assert_eq!(state.aoi_to, Some(80));
    }

    #[test]
    fn drag_pan_moves_zoomed_viewport_horizontally() {
        let mut state = make_state();
        let selection = PreviewSelection {
            index: vec![0],
            x: 0,
            slice: SliceSelection::All,
        };
        state.add_chart_item(
            source("/group/a", selection),
            (0..100).map(|i| (i as f64, i as f64)).collect(),
        );
        state.aoi_from = Some(20);
        state.aoi_to = Some(80);
        state.last_chart_area = Some(Rect::new(10, 5, 20, 8));

        assert!(state.start_drag_at_position(20, 6));
        assert!(!state.drag_to_position(15));
        assert_eq!(state.aoi_from, Some(20));
        assert_eq!(state.aoi_to, Some(80));

        assert!(state.finish_drag_at_position(15));
        assert_eq!(state.aoi_from, Some(36));
        assert_eq!(state.aoi_to, Some(96));

        state.end_drag();
        assert!(!state.drag_to_position(25));
        assert_eq!(state.aoi_from, Some(36));
        assert_eq!(state.aoi_to, Some(96));
    }

    #[test]
    fn drag_pan_only_starts_inside_chart_area() {
        let mut state = make_state();
        let selection = PreviewSelection {
            index: vec![0],
            x: 0,
            slice: SliceSelection::All,
        };
        state.add_chart_item(
            source("/group/a", selection),
            (0..100).map(|i| (i as f64, i as f64)).collect(),
        );
        state.aoi_from = Some(20);
        state.aoi_to = Some(80);
        state.last_chart_area = Some(Rect::new(10, 5, 20, 8));

        assert!(!state.start_drag_at_position(5, 6));
        assert!(!state.drag_to_position(15));
        assert_eq!(state.aoi_from, Some(20));
        assert_eq!(state.aoi_to, Some(80));
    }

    #[test]
    fn drag_pan_applies_only_on_release() {
        let mut state = make_state();
        let selection = PreviewSelection {
            index: vec![0],
            x: 0,
            slice: SliceSelection::All,
        };
        state.add_chart_item(
            source("/group/a", selection),
            (0..100).map(|i| (i as f64, i as f64)).collect(),
        );
        state.aoi_from = Some(20);
        state.aoi_to = Some(80);
        state.last_chart_area = Some(Rect::new(10, 5, 20, 8));

        assert!(state.start_drag_at_position(20, 6));
        assert!(!state.drag_to_position(18));
        assert_eq!(state.aoi_from, Some(20));
        assert_eq!(state.aoi_to, Some(80));
        assert!(!state.drag_to_position(15));
        assert_eq!(state.aoi_from, Some(20));
        assert_eq!(state.aoi_to, Some(80));

        assert!(state.finish_drag_at_position(15));
        assert_eq!(state.aoi_from, Some(36));
        assert_eq!(state.aoi_to, Some(96));
    }
}
