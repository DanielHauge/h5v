use hdf5_metno::{
    types::{FloatSize, IntSize, TypeDescriptor},
    Attribute, Dataset, File, Hyperslab, Selection, SliceOrIndex,
};
use plotters::{
    prelude::{BitMapBackend, IntoDrawingArea},
    style::{Color as _, IntoFont, RGBColor, ShapeStyle},
};
use ratatui::layout::Rect;
use ratatui_image::{picker::Picker, protocol::StatefulProtocol};
use std::ops::Range;

use crate::{
    configure,
    data::{
        validate_preview_selection_shape, DatasetPlotingData, PreviewSelection, SliceSelection,
    },
    error::log_error,
    search::full_traversal,
};

mod render;

pub type Point = (f64, f64);

#[derive(Debug, Clone, PartialEq)]
enum ExpressionPromptMode {
    New,
    EditExisting(ChartItemId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExpressionPromptMessageKind {
    Error,
    Valid,
    Hint,
}

#[derive(Debug, Clone, PartialEq)]
struct ExpressionPromptMessage {
    kind: ExpressionPromptMessageKind,
    text: String,
}

#[derive(Debug, Clone, PartialEq)]
struct ExpressionPromptSuggestion {
    symbol: String,
    insert_text: String,
    label: String,
    detail: String,
    kind: ExpressionPromptSuggestionKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExpressionPromptSuggestionKind {
    ItemRef,
    Group,
    Dataset,
    CompoundLeaf,
    Attribute,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExpressionPromptInputKind {
    Plain,
    ValidReference,
    InvalidReference,
}

#[derive(Debug, Clone, PartialEq)]
struct ExpressionPromptInputSegment {
    text: String,
    kind: ExpressionPromptInputKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExpressionAbsolutePathKind {
    Group,
    Dataset,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ExpressionAbsolutePathEntry {
    path: String,
    kind: ExpressionAbsolutePathKind,
    shape: Option<Vec<usize>>,
    detail: String,
}

#[derive(Debug, Clone, PartialEq)]
struct ExpressionPromptState {
    buffer: String,
    cursor: usize,
    mode: ExpressionPromptMode,
    messages: Vec<ExpressionPromptMessage>,
    suggestions: Vec<ExpressionPromptSuggestion>,
    selected_suggestion: Option<usize>,
    input_segments: Vec<ExpressionPromptInputSegment>,
}

impl ExpressionPromptState {
    fn new(buffer: String, cursor: usize, mode: ExpressionPromptMode) -> Self {
        Self {
            buffer,
            cursor,
            mode,
            messages: Vec::new(),
            suggestions: Vec::new(),
            selected_suggestion: None,
            input_segments: Vec::new(),
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
    ItemRef(ExpressionItemRef),
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
    ItemRef(ExpressionItemRef),
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
    Slice {
        start: Option<usize>,
        end: Option<usize>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum ExpressionObjectTarget {
    AbsolutePath(String),
    ItemRef(ChartItemId),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct ExpressionItemSlice {
    start: usize,
    end: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct ExpressionItemRef {
    id: ChartItemId,
    slice: Option<ExpressionItemSlice>,
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

impl ExpressionItemRef {
    fn render(&self) -> String {
        match &self.slice {
            Some(slice) => format!("${}[{}..{}]", self.id.0, slice.start, slice.end),
            None => format!("${}", self.id.0),
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
                        ExpressionDatasetSelector::Slice { start, end } => format!(
                            "{}..{}",
                            start.map(|value| value.to_string()).unwrap_or_default(),
                            end.map(|value| value.to_string()).unwrap_or_default()
                        ),
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
        let (x, index, slice) = match &self.selectors {
            None => {
                if shape.len() != 1 {
                    return Err(format!(
                        "Series reference {reference} needs an explicit selector like !/path[..,0] for rank-{} arrays",
                        shape.len()
                    ));
                }
                (0, vec![0], SliceSelection::All)
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
                let mut slice = SliceSelection::All;
                for (dim, selector) in selectors.iter().enumerate() {
                    match selector {
                        ExpressionDatasetSelector::All => {
                            if x.replace(dim).is_some() {
                                return Err(format!(
                                    "Dataset reference {} must contain exactly one slice axis selector",
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
                        ExpressionDatasetSelector::Slice { start, end } => {
                            if x.replace(dim).is_some() {
                                return Err(format!(
                                    "Dataset reference {} must contain exactly one slice axis selector",
                                    self.render()
                                ));
                            }
                            let start = start.unwrap_or(0);
                            let end = end.unwrap_or(shape[dim]);
                            if end <= start {
                                return Err(format!(
                                    "Dataset reference {} must use an increasing slice for dim {}",
                                    self.render(),
                                    dim
                                ));
                            }
                            if end > shape[dim] {
                                return Err(format!(
                                    "Dataset reference {} selects slice {}..{} out of bounds for dim {} with length {}",
                                    self.render(),
                                    start,
                                    end,
                                    dim,
                                    shape[dim]
                                ));
                            }
                            slice = SliceSelection::FromTo(start, end);
                        }
                    }
                }
                (
                    x.ok_or_else(|| {
                        format!(
                            "Series reference {reference} must contain exactly one slice axis selector"
                        )
                    })?,
                    index,
                    slice,
                )
            }
        };

        Ok(PreviewSelection { x, index, slice })
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

    pub fn concise_name(&self) -> String {
        self.display_path
            .rsplit('/')
            .find(|segment| !segment.is_empty())
            .unwrap_or("/")
            .to_string()
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
        if self.shape.is_empty() {
            return self.concise_name();
        }

        let axis_selector = match self.selection.slice {
            SliceSelection::All => "..".to_string(),
            SliceSelection::FromTo(start, end) => format!("{start}..{end}"),
        };
        if self.shape.len() == 1 && axis_selector == ".." {
            return self.concise_name();
        }

        let selectors = (0..self.shape.len())
            .map(|dim| {
                if dim == self.selection.x {
                    axis_selector.clone()
                } else {
                    self.selection
                        .index
                        .get(dim)
                        .copied()
                        .unwrap_or_default()
                        .to_string()
                }
            })
            .collect::<Vec<_>>();
        format!("{}[{}]", self.concise_name(), selectors.join(","))
    }

    pub fn expression_reference(&self) -> String {
        if self.shape.is_empty() {
            return format!("!{}", self.display_path);
        }
        let selectors = (0..self.shape.len())
            .map(|dim| {
                if dim == self.selection.x {
                    match self.selection.slice {
                        SliceSelection::All => "..".to_string(),
                        SliceSelection::FromTo(start, end) => format!("{start}..{end}"),
                    }
                } else {
                    self.selection.index[dim].to_string()
                }
            })
            .collect::<Vec<_>>();
        format!("!{}[{}]", self.display_path, selectors.join(","))
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
            ChartSource::DatasetSelection(source) => source.compact_selection_summary(),
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

fn is_finite_chart_point((x, y): Point) -> bool {
    x.is_finite() && y.is_finite()
}

fn sanitize_chart_points(points: Vec<Point>) -> Vec<Point> {
    points
        .into_iter()
        .filter(|point| is_finite_chart_point(*point))
        .collect()
}

impl ChartSeries {
    fn from_points(points: Vec<Point>) -> Option<Self> {
        let points = sanitize_chart_points(points);
        if points.is_empty() {
            return None;
        }
        let (_, first_y) = points[0];
        let (y_min, y_max) = points
            .iter()
            .skip(1)
            .fold((first_y, first_y), |(min, max), (_, y)| {
                (min.min(*y), max.max(*y))
            });
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

#[derive(Debug, Clone, PartialEq)]
pub struct ChartItemStats {
    pub samples: usize,
    pub x_min: f64,
    pub x_max: f64,
    pub y_min: f64,
    pub y_max: f64,
    pub mean: f64,
    pub median: f64,
    pub stddev: f64,
}

impl ChartItem {
    pub fn matches_path(&self, path: &str) -> bool {
        self.source.matches_path(path)
    }

    pub fn reference_label(&self) -> String {
        format!("${} {}", self.id.0, self.list_label())
    }

    pub fn list_label(&self) -> String {
        match &self.source {
            ChartSource::DatasetSelection(source) => source.expression_reference(),
            ChartSource::BuiltinDerived(source) => {
                format!("{} [{}]", source.expression(), source.alignment_summary())
            }
            ChartSource::DerivedExpression { expression, .. } => expression.clone(),
        }
    }

    pub fn statistics(&self) -> ChartItemStats {
        let mut ys = self
            .series
            .points
            .iter()
            .map(|(_, y)| *y)
            .collect::<Vec<_>>();
        ys.sort_by(f64::total_cmp);
        let samples = ys.len();
        let sum = ys.iter().sum::<f64>();
        let mean = if samples == 0 {
            0.0
        } else {
            sum / samples as f64
        };
        let variance = if samples <= 1 {
            0.0
        } else {
            ys.iter()
                .map(|value| {
                    let delta = *value - mean;
                    delta * delta
                })
                .sum::<f64>()
                / samples as f64
        };
        let median = match samples {
            0 => 0.0,
            n if n % 2 == 1 => ys[n / 2],
            n => (ys[n / 2 - 1] + ys[n / 2]) / 2.0,
        };
        let x_min = self
            .series
            .points
            .first()
            .map(|(x, _)| *x)
            .unwrap_or_default();
        let x_max = self
            .series
            .points
            .last()
            .map(|(x, _)| *x)
            .unwrap_or_default();

        ChartItemStats {
            samples,
            x_min,
            x_max,
            y_min: self.series.y_min,
            y_max: self.series.y_max,
            mean,
            median,
            stddev: variance.sqrt(),
        }
    }
}

#[derive(Debug, Clone)]
struct PreparedChartSeries {
    label: String,
    color_slot: usize,
    points: Vec<Point>,
    is_selected: bool,
    is_base: bool,
}

#[derive(Debug, Clone)]
struct PreparedChartData {
    plot_x_min: f64,
    plot_x_max: f64,
    y_min: f64,
    y_max: f64,
    series: Vec<PreparedChartSeries>,
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
        self.open_expression_prompt_for_selected();
    }

    pub fn open_expression_prompt_for_selected(&mut self) {
        let mode = match self.selected_item() {
            Some(ChartItem {
                id,
                source: ChartSource::DerivedExpression { .. },
                ..
            }) => ExpressionPromptMode::EditExisting(*id),
            Some(_) | None => ExpressionPromptMode::New,
        };
        let buffer = String::new();
        let cursor = buffer.len();
        self.expression_prompt = Some(ExpressionPromptState::new(buffer, cursor, mode));
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
            prompt.selected_suggestion = None;
        }
    }

    pub fn expression_backspace(&mut self) {
        if let Some(prompt) = self.expression_prompt.as_mut() {
            if prompt.cursor > 0 {
                prompt.cursor -= 1;
                prompt.buffer.remove(prompt.cursor);
                prompt.selected_suggestion = None;
            }
        }
    }

    pub fn expression_delete(&mut self) {
        if let Some(prompt) = self.expression_prompt.as_mut() {
            if prompt.cursor < prompt.buffer.len() {
                prompt.buffer.remove(prompt.cursor);
                prompt.selected_suggestion = None;
            }
        }
    }

    pub fn expression_move_left(&mut self) {
        if let Some(prompt) = self.expression_prompt.as_mut() {
            if prompt.cursor > 0 {
                prompt.cursor -= 1;
            }
            prompt.selected_suggestion = None;
        }
    }

    pub fn expression_move_right(&mut self) {
        if let Some(prompt) = self.expression_prompt.as_mut() {
            if prompt.cursor < prompt.buffer.len() {
                prompt.cursor += 1;
            }
            prompt.selected_suggestion = None;
        }
    }

    pub fn expression_move_to_start(&mut self) {
        if let Some(prompt) = self.expression_prompt.as_mut() {
            prompt.cursor = 0;
            prompt.selected_suggestion = None;
        }
    }

    pub fn expression_move_to_end(&mut self) {
        if let Some(prompt) = self.expression_prompt.as_mut() {
            prompt.cursor = prompt.buffer.len();
            prompt.selected_suggestion = None;
        }
    }

    pub fn expression_clear(&mut self) {
        if let Some(prompt) = self.expression_prompt.as_mut() {
            prompt.buffer.clear();
            prompt.cursor = 0;
            prompt.selected_suggestion = None;
        }
    }

    pub fn expression_select_next_suggestion(&mut self) {
        if let Some(prompt) = self.expression_prompt.as_mut() {
            if !prompt.suggestions.is_empty() {
                prompt.selected_suggestion = Some(match prompt.selected_suggestion {
                    Some(selected) => (selected + 1).min(prompt.suggestions.len() - 1),
                    None => 0,
                });
            }
        }
    }

    pub fn expression_select_prev_suggestion(&mut self) {
        if let Some(prompt) = self.expression_prompt.as_mut() {
            if !prompt.suggestions.is_empty() {
                prompt.selected_suggestion = Some(match prompt.selected_suggestion {
                    Some(selected) => selected.saturating_sub(1),
                    None => prompt.suggestions.len() - 1,
                });
            }
        }
    }

    pub fn expression_deselect_suggestion(&mut self) -> bool {
        let Some(prompt) = self.expression_prompt.as_mut() else {
            return false;
        };
        prompt.selected_suggestion.take().is_some()
    }

    pub fn expression_has_selected_suggestion(&self) -> bool {
        self.expression_prompt
            .as_ref()
            .and_then(|prompt| prompt.selected_suggestion)
            .is_some()
    }

    pub fn expression_apply_selected_suggestion(&mut self) -> bool {
        let Some(prompt) = self.expression_prompt.as_mut() else {
            return false;
        };
        let Some((start, end, suggestion)) = current_expression_completion(prompt)
            .map(|(start, end, _, suggestion)| (start, end, suggestion.clone()))
        else {
            return false;
        };
        prompt
            .buffer
            .replace_range(start..end, &suggestion.insert_text);
        prompt.cursor = start + suggestion.insert_text.len();
        prompt.selected_suggestion = None;
        true
    }

    pub fn refresh_expression_prompt(&mut self, file: Option<&File>) {
        let Some((buffer, cursor, selected_suggestion)) =
            self.expression_prompt.as_ref().map(|prompt| {
                (
                    prompt.buffer.clone(),
                    prompt.cursor,
                    prompt.selected_suggestion,
                )
            })
        else {
            return;
        };
        let messages = expression_prompt_messages(self, file, &buffer);
        let suggestions = expression_prompt_suggestions(self, file, &buffer, cursor);
        let input_segments = expression_prompt_input_segments(self, file, &buffer);
        if let Some(prompt) = self.expression_prompt.as_mut() {
            prompt.messages = messages;
            prompt.suggestions = suggestions;
            prompt.selected_suggestion =
                selected_suggestion.filter(|selected| *selected < prompt.suggestions.len());
            prompt.input_segments = input_segments;
        }
    }

    pub fn submit_expression_prompt(&mut self, file: Option<&File>) -> Result<(), String> {
        let (expression, mode) = self
            .expression_prompt
            .as_ref()
            .map(|prompt| (prompt.buffer.trim().to_string(), prompt.mode.clone()))
            .ok_or_else(|| "Expression prompt is not active".to_string())?;
        if expression.is_empty() {
            self.set_expression_messages(vec![ExpressionPromptMessage {
                kind: ExpressionPromptMessageKind::Error,
                text: "Enter an expression before submitting".to_string(),
            }]);
            return Ok(());
        }

        let result = match mode {
            ExpressionPromptMode::New => self
                .create_expression_derived_with_file(expression.clone(), file)
                .map(|_| ()),
            ExpressionPromptMode::EditExisting(id) => {
                self.update_expression_item_with_file(id, expression.clone(), file)
            }
        };

        match result {
            Ok(_) => {
                self.close_expression_prompt();
                Ok(())
            }
            Err(error) => {
                self.set_expression_messages(vec![ExpressionPromptMessage {
                    kind: ExpressionPromptMessageKind::Error,
                    text: error,
                }]);
                Ok(())
            }
        }
    }

    fn set_expression_messages(&mut self, messages: Vec<ExpressionPromptMessage>) {
        if let Some(prompt) = self.expression_prompt.as_mut() {
            prompt.messages = messages;
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
        let points = sanitize_chart_points(evaluated.points);
        if points.is_empty() {
            return Err("Expression resolved to no finite points".to_string());
        }
        let len = points.len();
        let source = ChartSource::DerivedExpression {
            expression: expression.to_string(),
            input_ids: evaluated.input_ids,
            len,
            kind: evaluated.kind,
        };
        Ok((source, points))
    }

    fn create_expression_derived_with_file(
        &mut self,
        expression: String,
        file: Option<&File>,
    ) -> Result<ChartItemId, String> {
        let evaluated = self.evaluate_expression_with_file(&expression, file)?;
        let points = sanitize_chart_points(evaluated.points);
        if points.is_empty() {
            return Err("Expression resolved to no finite points".to_string());
        }
        let len = points.len();
        let source = ChartSource::DerivedExpression {
            expression,
            input_ids: evaluated.input_ids,
            len,
            kind: evaluated.kind,
        };
        self.add_chart_item(source, points)
            .ok_or_else(|| "Failed to create expression-derived chart".to_string())
    }

    fn update_expression_item_with_file(
        &mut self,
        id: ChartItemId,
        expression: String,
        file: Option<&File>,
    ) -> Result<(), String> {
        let evaluated = self.evaluate_expression_with_file(&expression, file)?;
        let points = sanitize_chart_points(evaluated.points);
        if points.is_empty() {
            return Err("Expression resolved to no finite points".to_string());
        }
        let len = points.len();
        let source = ChartSource::DerivedExpression {
            expression,
            input_ids: evaluated.input_ids,
            len,
            kind: evaluated.kind,
        };
        let series = ChartSeries::from_points(points)
            .ok_or_else(|| "Expression resolved to no finite points".to_string())?;
        let index = self
            .items
            .iter()
            .position(|item| item.id == id)
            .ok_or_else(|| format!("Chart item ${} no longer exists", id.0))?;
        self.items[index].label = source.label();
        self.items[index].source = source;
        self.items[index].series = series;
        self.idx = index;
        self.modified = true;
        Ok(())
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
        refs.item_refs.sort_by(|lhs, rhs| {
            lhs.id
                .0
                .cmp(&rhs.id.0)
                .then_with(|| match (&lhs.slice, &rhs.slice) {
                    (None, None) => std::cmp::Ordering::Equal,
                    (None, Some(_)) => std::cmp::Ordering::Less,
                    (Some(_), None) => std::cmp::Ordering::Greater,
                    (Some(lhs), Some(rhs)) => lhs.start.cmp(&rhs.start).then(lhs.end.cmp(&rhs.end)),
                })
        });
        refs.item_refs.dedup();
        refs.series_refs
            .sort_by_key(|series_ref| series_ref.render());
        refs.series_refs.dedup();
        refs.scalar_refs
            .sort_by_key(|scalar_ref| scalar_ref.render());
        refs.scalar_refs.dedup();
        if refs.item_refs.is_empty() && refs.series_refs.is_empty() {
            return Err(
                "Expression must reference at least one series such as $3, !/group/ds[..,0], or !$3:ATTR"
                    .to_string(),
            );
        }

        let item_values = refs
            .item_refs
            .iter()
            .map(|item_ref| {
                resolve_expression_item_value(self, item_ref)
                    .map(|points| (item_ref.clone(), points))
            })
            .collect::<Result<std::collections::HashMap<_, _>, _>>()?;

        let external_series = resolve_expression_series_values(self, file, &refs.series_refs)?;
        let mut series_inputs = item_values
            .iter()
            .map(|(item_ref, points)| ExpressionSeriesInput {
                label: item_ref.render(),
                points: points.clone(),
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
                    let y = eval_expression_at(
                        ast,
                        idx,
                        &item_values,
                        &external_series,
                        &scalar_values,
                    )?;
                    points.push((first.points[idx].0, y));
                }
                DerivedExpressionKind::YSeries
            }
            ParsedExpression::XySeries(x_ast, y_ast) => {
                for idx in 0..expected_len {
                    let x = eval_expression_at(
                        x_ast,
                        idx,
                        &item_values,
                        &external_series,
                        &scalar_values,
                    )?;
                    let y = eval_expression_at(
                        y_ast,
                        idx,
                        &item_values,
                        &external_series,
                        &scalar_values,
                    )?;
                    points.push((x, y));
                }
                DerivedExpressionKind::XySeries
            }
        };

        let mut input_ids = refs
            .item_refs
            .iter()
            .map(|item_ref| item_ref.id)
            .collect::<Vec<_>>();
        input_ids.sort_by_key(|id| id.0);
        input_ids.dedup();

        Ok(EvaluatedExpression {
            points,
            kind,
            input_ids,
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
            return Err("Mark a base series first".to_string());
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

    fn prepared_chart_data(&self) -> Option<PreparedChartData> {
        let visible_items = self
            .items
            .iter()
            .filter(|item| item.visible)
            .collect::<Vec<_>>();
        if visible_items.is_empty() {
            return None;
        }

        let (global_x_min, global_x_max) = self.global_x_bounds().unwrap_or((0, 1));
        let (x_min, x_max) = match (self.aoi_from, self.aoi_to) {
            (None, None) => (global_x_min, global_x_max),
            (Some(from), None) => (from, global_x_max.max(from)),
            (None, Some(to)) => (global_x_min.min(to), to),
            (Some(from), Some(to)) if from < to => (from, to),
            _ => return None,
        };

        let selected_item_id = self.selected_item().map(|item| item.id);
        let mut global_y_max = f64::MIN;
        let mut global_y_min = f64::MAX;
        let mut plot_x_min = f64::MAX;
        let mut plot_x_max = f64::MIN;
        let mut series = Vec::new();

        for item in visible_items {
            if x_max <= item.series.sample_min || x_min >= item.series.sample_max {
                continue;
            }
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
            let points = item.series.points[local_x_min..local_x_max]
                .iter()
                .copied()
                .filter(|point| is_finite_chart_point(*point))
                .collect::<Vec<_>>();
            if points.is_empty() {
                continue;
            }

            for &(x, y) in &points {
                global_y_max = global_y_max.max(y);
                global_y_min = global_y_min.min(y);
                plot_x_min = plot_x_min.min(x);
                plot_x_max = plot_x_max.max(x);
            }

            series.push(PreparedChartSeries {
                label: item.label.clone(),
                color_slot: item.color_slot,
                points,
                is_selected: selected_item_id == Some(item.id),
                is_base: self.marked_base_item == Some(item.id),
            });
        }

        if series.is_empty() || !global_y_min.is_finite() || !global_y_max.is_finite() {
            return None;
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
        if !plot_x_min.is_finite() || !plot_x_max.is_finite() {
            return None;
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

        Some(PreparedChartData {
            plot_x_min,
            plot_x_max,
            y_min,
            y_max,
            series,
        })
    }

    fn render_chart_with_area(&mut self, chart_area: Option<Rect>) -> bool {
        if !self.modified {
            return false;
        }
        self.idx = self.idx.clamp(0, self.items.len().saturating_sub(1));
        self.modified = false;
        let Some(prepared) = self.prepared_chart_data() else {
            return false;
        };

        let width = self.width;
        let height = self.height;
        self.plot_buffer = vec![0; (width * height * 3) as usize];
        let root =
            BitMapBackend::with_buffer(&mut self.plot_buffer, (width, height)).into_drawing_area();
        let (bg_r, bg_g, bg_b) =
            configure::rgb_channels(configure::themed_color(|colors| colors.chart.plot_bg));
        let (grid_r, grid_g, grid_b) =
            configure::rgb_channels(configure::themed_color(|colors| colors.chart.grid));
        let (axis_r, axis_g, axis_b) =
            configure::rgb_channels(configure::themed_color(|colors| colors.chart.axis));
        let plot_bg = RGBColor(bg_r, bg_g, bg_b);
        let grid = RGBColor(grid_r, grid_g, grid_b);
        let axis = RGBColor(axis_r, axis_g, axis_b);
        if let Err(e) = root.fill(&plot_bg) {
            log_error(e);
            return false;
        }
        let y_label_area_size = format!("{:.4}", prepared.y_max).len() as u32 * 3 + 30;
        let chart = plotters::prelude::ChartBuilder::on(&root)
            .margin(10)
            .x_label_area_size(30)
            .y_label_area_size(y_label_area_size)
            .build_cartesian_2d(
                prepared.plot_x_min..prepared.plot_x_max,
                prepared.y_min..prepared.y_max,
            );

        let mut chart = match chart {
            Ok(chart) => chart,
            Err(e) => {
                log_error(e);
                return false;
            }
        };
        if let Some(chart_area) = chart_area {
            let (plot_x_range, plot_y_range) = chart.plotting_area().get_pixel_range();
            self.last_chart_area =
                chart_plot_area_in_rect(chart_area, width, height, plot_x_range, plot_y_range);
        }

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

        for series in prepared.series {
            let (r, g, b) = configure::rgb_channels(configure::themed_color(|colors| {
                colors.chart.series[series.color_slot % colors.chart.series.len()]
            }));
            let color = RGBColor(r, g, b);
            let stroke_width = if series.is_base || series.is_selected {
                4
            } else {
                3
            };
            let line_series = plotters::prelude::LineSeries::new(
                series.points.iter().copied(),
                ShapeStyle::from(&color).stroke_width(stroke_width),
            );
            let series_label = series.label.clone();
            let drawn_series = match chart.draw_series(line_series) {
                Ok(series) => series,
                Err(e) => {
                    log_error(e);
                    continue;
                }
            };
            drawn_series.label(series_label).legend(move |(x, y)| {
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
}

fn chart_plot_area_in_rect(
    outer_area: Rect,
    width_px: u32,
    height_px: u32,
    plot_x_range: Range<i32>,
    plot_y_range: Range<i32>,
) -> Option<Rect> {
    if outer_area.width == 0 || outer_area.height == 0 || width_px == 0 || height_px == 0 {
        return None;
    }
    let x_start = plot_x_range.start.max(0) as u32;
    let x_end = plot_x_range.end.max(plot_x_range.start).max(0) as u32;
    let y_start = plot_y_range.start.max(0) as u32;
    let y_end = plot_y_range.end.max(plot_y_range.start).max(0) as u32;
    if x_end <= x_start || y_end <= y_start {
        return None;
    }

    let left = x_start
        .saturating_mul(outer_area.width as u32)
        .checked_div(width_px)
        .unwrap_or(0);
    let right = ((x_end.saturating_mul(outer_area.width as u32)) + width_px.saturating_sub(1))
        .checked_div(width_px)
        .unwrap_or(outer_area.width as u32)
        .min(outer_area.width as u32);
    let top = y_start
        .saturating_mul(outer_area.height as u32)
        .checked_div(height_px)
        .unwrap_or(0);
    let bottom = ((y_end.saturating_mul(outer_area.height as u32)) + height_px.saturating_sub(1))
        .checked_div(height_px)
        .unwrap_or(outer_area.height as u32)
        .min(outer_area.height as u32);

    let width = right.saturating_sub(left).max(1) as u16;
    let height = bottom.saturating_sub(top).max(1) as u16;
    Some(Rect::new(
        outer_area.x.saturating_add(left as u16),
        outer_area.y.saturating_add(top as u16),
        width.min(outer_area.width.saturating_sub(left as u16)),
        height.min(outer_area.height.saturating_sub(top as u16)),
    ))
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
                tokens.push(ExpressionToken::ItemRef(parse_expression_item_ref(
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

fn parse_expression_item_ref(
    chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
) -> Result<ExpressionItemRef, String> {
    let id = parse_expression_item_id(chars)?;
    let slice = if chars.peek() == Some(&'[') {
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
                "Chart item reference '${}[{spec}' is missing a closing ']'",
                id.0
            ));
        }
        Some(parse_expression_item_slice(id, &spec)?)
    } else {
        None
    };
    Ok(ExpressionItemRef { id, slice })
}

fn parse_expression_item_slice(id: ChartItemId, spec: &str) -> Result<ExpressionItemSlice, String> {
    let Some((start, end)) = spec.split_once("..") else {
        return Err(format!(
            "Chart item reference '${}[{spec}]' must use a slice like [0..5]",
            id.0
        ));
    };
    let start = start.trim().parse::<usize>().map_err(|_| {
        format!(
            "Chart item reference '${}[{spec}]' has invalid slice start '{}'",
            id.0,
            start.trim()
        )
    })?;
    let end = end.trim().parse::<usize>().map_err(|_| {
        format!(
            "Chart item reference '${}[{spec}]' has invalid slice end '{}'",
            id.0,
            end.trim()
        )
    })?;
    if end <= start {
        return Err(format!(
            "Chart item reference '${}[{spec}]' must use an increasing slice",
            id.0
        ));
    }
    Ok(ExpressionItemSlice { start, end })
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
            } else if let Some((start, end)) = part.split_once("..") {
                let start = if start.trim().is_empty() {
                    None
                } else {
                    Some(start.trim().parse::<usize>().map_err(|_| {
                        format!(
                            "Series reference '{reference}[{spec}]' has invalid slice start '{start}'; use '..', 'a..b', '..b', 'a..', or a non-negative integer"
                        )
                    })?)
                };
                let end = if end.trim().is_empty() {
                    None
                } else {
                    Some(end.trim().parse::<usize>().map_err(|_| {
                        format!(
                            "Series reference '{reference}[{spec}]' has invalid slice end '{end}'; use '..', 'a..b', '..b', 'a..', or a non-negative integer"
                        )
                    })?)
                };
                if start.is_none() && end.is_none() {
                    Ok(ExpressionDatasetSelector::All)
                } else {
                    Ok(ExpressionDatasetSelector::Slice { start, end })
                }
            } else {
                part.parse::<usize>()
                    .map(ExpressionDatasetSelector::Index)
                    .map_err(|_| {
                        format!(
                            "Series reference '{reference}[{spec}]' has invalid selector '{part}'; use '..', 'a..b', '..b', 'a..', or a non-negative integer"
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
            ExpressionToken::ItemRef(item_ref) => {
                *pos += 1;
                Ok(ExpressionAst::ItemRef(item_ref.clone()))
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
    item_refs: Vec<ExpressionItemRef>,
    series_refs: Vec<ExpressionSeriesRef>,
    scalar_refs: Vec<ExpressionScalarRef>,
}

fn collect_expression_refs(expr: &ExpressionAst, out: &mut ExpressionRefs) {
    match expr {
        ExpressionAst::Number(_) => {}
        ExpressionAst::ItemRef(item_ref) => out.item_refs.push(item_ref.clone()),
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

fn resolve_expression_item_value(
    state: &MultiChartState,
    item_ref: &ExpressionItemRef,
) -> Result<Vec<Point>, String> {
    let item = state
        .item_by_id(item_ref.id)
        .ok_or_else(|| format!("Unknown chart item reference ${}", item_ref.id.0))?;
    let points = sanitize_chart_points(item.series.points.clone());
    let points = match &item_ref.slice {
        Some(slice) => {
            if slice.end > points.len() {
                return Err(format!(
                    "Chart item reference {} is out of bounds for len {}",
                    item_ref.render(),
                    points.len()
                ));
            }
            points[slice.start..slice.end].to_vec()
        }
        None => points,
    };
    if points.is_empty() {
        return Err(format!(
            "Chart item reference {} resolved to no finite points",
            item_ref.render()
        ));
    }
    Ok(points)
}

fn dataset_ploting_data_from_points(points: Vec<Point>) -> Result<DatasetPlotingData, String> {
    let points = sanitize_chart_points(points);
    let Some((_, first_y)) = points.first().copied() else {
        return Err("Cannot build a preview from a series with no finite points".to_string());
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
    item_values: &std::collections::HashMap<ExpressionItemRef, Vec<Point>>,
    series_values: &std::collections::HashMap<ExpressionSeriesRef, Vec<Point>>,
    scalar_values: &std::collections::HashMap<ExpressionScalarRef, f64>,
) -> Result<f64, String> {
    match expr {
        ExpressionAst::Number(value) => Ok(*value),
        ExpressionAst::ItemRef(item_ref) => item_values
            .get(item_ref)
            .and_then(|points| points.get(idx).map(|(_, y)| *y))
            .ok_or_else(|| {
                format!(
                    "Chart item {} is unavailable at sample index {}",
                    item_ref.render(),
                    idx
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
            item_values,
            series_values,
            scalar_values,
        )?),
        ExpressionAst::Binary { op, lhs, rhs } => {
            let lhs = eval_expression_at(lhs, idx, item_values, series_values, scalar_values)?;
            let rhs = eval_expression_at(rhs, idx, item_values, series_values, scalar_values)?;
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

fn require_finite_scalar_value(value: f64, reference: &str) -> Result<f64, String> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(format!(
            "Scalar reference {reference} resolved to a non-finite value"
        ))
    }
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
        (ExpressionObjectTarget::ItemRef(id), None) => {
            let points = state
                .item_by_id(*id)
                .map(|item| sanitize_chart_points(item.series.points.clone()))
                .ok_or_else(|| format!("Unknown chart item reference ${}", id.0))?;
            if points.is_empty() {
                Err(format!(
                    "Series reference {} resolved to no finite points",
                    series_ref.render()
                ))
            } else {
                Ok(points)
            }
        }
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

    let points = sanitize_chart_points(
        values
            .into_iter()
            .enumerate()
            .map(|(idx, value)| (idx as f64, value))
            .collect::<Vec<_>>(),
    );
    if points.is_empty() {
        return Err(format!(
            "Series reference {} resolved to no finite points",
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
            require_finite_scalar_value(
                read_expression_numeric_scalar_attr(&attr, &scalar_ref.render())?,
                &scalar_ref.render(),
            )
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
            require_finite_scalar_value(
                read_expression_numeric_scalar_dataset(&dataset, &scalar_ref.render())?,
                &scalar_ref.render(),
            )
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

fn expression_prompt_messages(
    state: &MultiChartState,
    file: Option<&File>,
    buffer: &str,
) -> Vec<ExpressionPromptMessage> {
    let trimmed = buffer.trim();
    if trimmed.is_empty() {
        return vec![
            ExpressionPromptMessage {
                kind: ExpressionPromptMessageKind::Hint,
                text: "Use $1, !/path[..,0], #/path:ATTR, or ($1, !/time[..])".to_string(),
            },
            ExpressionPromptMessage {
                kind: ExpressionPromptMessageKind::Hint,
                text: "Tab applies the selected suggestion.".to_string(),
            },
        ];
    }

    if expression_prompt_has_pending_completion(state, file, trimmed) {
        return Vec::new();
    }

    match state.evaluate_expression_with_file(trimmed, file) {
        Ok(evaluated) => {
            let result_kind = match evaluated.kind {
                DerivedExpressionKind::YSeries => "y-series",
                DerivedExpressionKind::XySeries => "x/y series",
            };
            vec![ExpressionPromptMessage {
                kind: ExpressionPromptMessageKind::Valid,
                text: format!(
                    "Valid {result_kind} with {} samples",
                    evaluated.points.len()
                ),
            }]
        }
        Err(error) => vec![ExpressionPromptMessage {
            kind: ExpressionPromptMessageKind::Error,
            text: error,
        }],
    }
}

fn expression_prompt_has_pending_completion(
    state: &MultiChartState,
    file: Option<&File>,
    buffer: &str,
) -> bool {
    let Some((_, end, fragment)) = current_expression_fragment(buffer, buffer.len()) else {
        return false;
    };
    if end != buffer.len() || fragment.is_empty() {
        return false;
    }
    if fragment.starts_with('$') {
        return state.items.iter().any(|item| {
            let candidate = format!("${}", item.id.0);
            candidate.starts_with(&fragment)
        });
    }
    if fragment.starts_with('!') || fragment.starts_with('#') {
        let Some(file) = file else {
            return false;
        };
        return !expression_path_suggestions(file, &fragment).is_empty();
    }
    false
}

fn expression_prompt_input_segments(
    state: &MultiChartState,
    file: Option<&File>,
    buffer: &str,
) -> Vec<ExpressionPromptInputSegment> {
    let mut segments = Vec::new();
    let chars: Vec<(usize, char)> = buffer.char_indices().collect();
    let mut idx = 0;
    let mut plain_start = 0;

    while idx < chars.len() {
        let (start, ch) = chars[idx];
        if !matches!(ch, '$' | '!' | '#') {
            idx += 1;
            continue;
        }
        let end = consume_expression_reference_fragment(buffer, &chars, idx);
        if end <= start + ch.len_utf8() {
            idx += 1;
            continue;
        }
        if plain_start < start {
            segments.push(ExpressionPromptInputSegment {
                text: buffer[plain_start..start].to_string(),
                kind: ExpressionPromptInputKind::Plain,
            });
        }
        let fragment = &buffer[start..end];
        let kind = if end == buffer.len() {
            ExpressionPromptInputKind::Plain
        } else {
            match validate_expression_reference_fragment(state, file, fragment) {
                Ok(()) => ExpressionPromptInputKind::ValidReference,
                Err(_) => ExpressionPromptInputKind::InvalidReference,
            }
        };
        segments.push(ExpressionPromptInputSegment {
            text: fragment.to_string(),
            kind,
        });
        plain_start = end;
        while idx < chars.len() && chars[idx].0 < end {
            idx += 1;
        }
    }

    if plain_start < buffer.len() {
        segments.push(ExpressionPromptInputSegment {
            text: buffer[plain_start..].to_string(),
            kind: ExpressionPromptInputKind::Plain,
        });
    }

    if segments.is_empty() {
        segments.push(ExpressionPromptInputSegment {
            text: buffer.to_string(),
            kind: ExpressionPromptInputKind::Plain,
        });
    }
    segments
}

fn consume_expression_reference_fragment(
    buffer: &str,
    chars: &[(usize, char)],
    start_idx: usize,
) -> usize {
    let start_char = chars[start_idx].1;
    let mut cursor = start_idx + 1;
    let mut bracket_depth = 0usize;
    while cursor < chars.len() {
        let ch = chars[cursor].1;
        match ch {
            '[' => bracket_depth += 1,
            ']' => bracket_depth = bracket_depth.saturating_sub(1),
            _ => {}
        }
        let is_delimiter = ch.is_whitespace()
            || (bracket_depth == 0 && matches!(ch, '+' | '-' | '*' | ',' | '(' | ')'))
            || (start_char == '$' && ch == '/');
        if is_delimiter {
            break;
        }
        cursor += 1;
    }
    chars
        .get(cursor)
        .map(|(offset, _)| *offset)
        .unwrap_or(buffer.len())
}

fn validate_expression_reference_fragment(
    state: &MultiChartState,
    file: Option<&File>,
    fragment: &str,
) -> Result<(), String> {
    match fragment.chars().next() {
        Some('$') => {
            let mut chars = fragment[1..].chars().peekable();
            let item_ref = parse_expression_item_ref(&mut chars)?;
            if chars.next().is_some() {
                return Err(format!("Invalid chart item reference {fragment}"));
            }
            let _ = resolve_expression_item_value(state, &item_ref)?;
            Ok(())
        }
        Some('!') => {
            let mut chars = fragment[1..].chars().peekable();
            let series_ref = parse_expression_series_ref(&mut chars)?;
            if chars.next().is_some() {
                return Err(format!("Invalid series reference {fragment}"));
            }
            let file = file.ok_or_else(|| "No file loaded for series references".to_string())?;
            let _ = resolve_expression_series_value(state, file, &series_ref)?;
            Ok(())
        }
        Some('#') => {
            let mut chars = fragment[1..].chars().peekable();
            let scalar_ref = parse_expression_scalar_ref(&mut chars)?;
            if chars.next().is_some() {
                return Err(format!("Invalid scalar reference {fragment}"));
            }
            let file = file.ok_or_else(|| "No file loaded for scalar references".to_string())?;
            let _ = resolve_expression_scalar_value(state, file, &scalar_ref)?;
            Ok(())
        }
        _ => Ok(()),
    }
}

fn current_expression_completion(
    prompt: &ExpressionPromptState,
) -> Option<(usize, usize, String, &ExpressionPromptSuggestion)> {
    let (start, end, fragment) = current_expression_fragment(&prompt.buffer, prompt.cursor)?;
    let suggestion = prompt
        .selected_suggestion
        .and_then(|selected| prompt.suggestions.get(selected))?;
    Some((start, end, fragment, suggestion))
}

fn current_expression_fragment(buffer: &str, cursor: usize) -> Option<(usize, usize, String)> {
    if cursor > buffer.len() {
        return None;
    }
    let chars: Vec<(usize, char)> = buffer.char_indices().collect();
    let char_cursor = chars
        .iter()
        .take_while(|(offset, _)| *offset < cursor)
        .count();
    let initial_depth = chars[..char_cursor]
        .iter()
        .fold(0usize, |depth, (_, ch)| match ch {
            '[' => depth + 1,
            ']' => depth.saturating_sub(1),
            _ => depth,
        });

    let mut start = cursor;
    let mut depth = initial_depth;
    let mut idx = char_cursor;
    while idx > 0 {
        let (offset, ch) = chars[idx - 1];
        let is_delimiter =
            depth == 0 && (ch.is_whitespace() || matches!(ch, '+' | '-' | '*' | ',' | '(' | ')'));
        if is_delimiter {
            break;
        }
        start = offset;
        match ch {
            ']' => depth += 1,
            '[' => depth = depth.saturating_sub(1),
            _ => {}
        }
        idx -= 1;
    }

    let mut end = cursor;
    let mut depth = initial_depth;
    let mut idx = char_cursor;
    while idx < chars.len() {
        let (_, ch) = chars[idx];
        let is_delimiter =
            depth == 0 && (ch.is_whitespace() || matches!(ch, '+' | '-' | '*' | ',' | '(' | ')'));
        if is_delimiter {
            break;
        }
        end = chars
            .get(idx + 1)
            .map(|(next_offset, _)| *next_offset)
            .unwrap_or(buffer.len());
        match ch {
            '[' => depth += 1,
            ']' => depth = depth.saturating_sub(1),
            _ => {}
        }
        idx += 1;
    }

    if start >= end || !matches!(buffer[start..].chars().next(), Some('$' | '!' | '#')) {
        return None;
    }
    Some((start, end, buffer[start..end].to_string()))
}

fn expression_prompt_suggestions(
    state: &MultiChartState,
    file: Option<&File>,
    buffer: &str,
    cursor: usize,
) -> Vec<ExpressionPromptSuggestion> {
    let Some((_, _, fragment)) = current_expression_fragment(buffer, cursor) else {
        return Vec::new();
    };
    if fragment.starts_with('$') {
        return state
            .items
            .iter()
            .take(8)
            .filter(|item| {
                let candidate = format!("${}", item.id.0);
                candidate.starts_with(&fragment)
            })
            .map(|item| ExpressionPromptSuggestion {
                symbol: match &item.source {
                    ChartSource::DatasetSelection(source) => match source.kind {
                        DatasetChartKind::Dataset => {
                            configure::configured_symbol(|symbols| symbols.tree.dataset_icon)
                                .to_string()
                        }
                        DatasetChartKind::CompoundLeaf => {
                            configure::configured_symbol(|symbols| symbols.tree.compound_leaf_icon)
                                .to_string()
                        }
                    },
                    ChartSource::DerivedExpression { .. } | ChartSource::BuiltinDerived(_) => {
                        configure::configured_symbol(|symbols| symbols.chart.membership_marker)
                            .to_string()
                    }
                },
                insert_text: format!("${}", item.id.0),
                label: format!("${}", item.id.0),
                detail: format!("{} | len {}", item.list_label(), item.series.len()),
                kind: match &item.source {
                    ChartSource::DatasetSelection(source) => match source.kind {
                        DatasetChartKind::Dataset => ExpressionPromptSuggestionKind::Dataset,
                        DatasetChartKind::CompoundLeaf => {
                            ExpressionPromptSuggestionKind::CompoundLeaf
                        }
                    },
                    ChartSource::DerivedExpression { .. } | ChartSource::BuiltinDerived(_) => {
                        ExpressionPromptSuggestionKind::ItemRef
                    }
                },
            })
            .collect();
    }

    if !(fragment.starts_with('!') || fragment.starts_with('#')) {
        return Vec::new();
    }

    let Some(file) = file else {
        return Vec::new();
    };

    if let Some((target, attr_prefix)) = fragment.split_once(':') {
        let object_path = resolve_completion_target_path(state, target);
        let Some(object_path) = object_path else {
            return Vec::new();
        };
        return expression_attribute_suggestions(file, &object_path, target, attr_prefix);
    }

    expression_path_suggestions(file, &fragment)
}

fn resolve_completion_target_path(state: &MultiChartState, target: &str) -> Option<String> {
    if let Some(path) = target
        .strip_prefix("!$")
        .or_else(|| target.strip_prefix("#$"))
    {
        let id = path.parse::<u64>().ok()?;
        return state
            .item_by_id(ChartItemId(id))
            .and_then(|item| item.source.dataset_source())
            .map(|source| source.dataset_path.clone());
    }
    if let Some(path) = target
        .strip_prefix('!')
        .or_else(|| target.strip_prefix('#'))
    {
        return normalize_absolute_object_path(path).ok();
    }
    None
}

fn expression_attribute_suggestions(
    file: &File,
    object_path: &str,
    target: &str,
    attr_prefix: &str,
) -> Vec<ExpressionPromptSuggestion> {
    let names = if object_path == "/" {
        file.attr_names().ok()
    } else if let Ok(group) = file.group(object_path) {
        group.attr_names().ok()
    } else if let Ok(dataset) = file.dataset(object_path) {
        dataset.attr_names().ok()
    } else {
        None
    }
    .unwrap_or_default();

    names
        .into_iter()
        .filter(|name| {
            name.to_ascii_lowercase()
                .contains(&attr_prefix.to_ascii_lowercase())
        })
        .take(8)
        .map(|name| ExpressionPromptSuggestion {
            symbol: String::new(),
            insert_text: format!("{target}:{name}"),
            label: format!("{target}:{name}"),
            detail: String::new(),
            kind: ExpressionPromptSuggestionKind::Attribute,
        })
        .collect()
}

fn expression_path_suggestions(file: &File, fragment: &str) -> Vec<ExpressionPromptSuggestion> {
    let target_kind = match fragment.chars().next() {
        Some('!') => Some(ExpressionAbsolutePathKind::Dataset),
        Some('#') => Some(ExpressionAbsolutePathKind::Dataset),
        _ => None,
    };
    let needle = fragment[1..].to_ascii_lowercase();
    expression_absolute_path_entries(file)
        .into_iter()
        .filter(|entry| {
            let kind_matches = match target_kind {
                Some(ExpressionAbsolutePathKind::Dataset) => true,
                Some(ExpressionAbsolutePathKind::Group) => {
                    entry.kind == ExpressionAbsolutePathKind::Group
                }
                None => true,
            };
            kind_matches
                && (entry.path.to_ascii_lowercase().contains(&needle)
                    || entry.path.to_ascii_lowercase().starts_with(&needle))
        })
        .take(8)
        .map(|entry| {
            let label = format!("{}{}", &fragment[..1], entry.path);
            let insert_text = match (&fragment[..1], entry.kind, entry.shape.as_ref()) {
                ("!", ExpressionAbsolutePathKind::Dataset, Some(shape)) if !shape.is_empty() => {
                    format!(
                        "{}{}[{}]",
                        &fragment[..1],
                        entry.path,
                        vec![".."; shape.len()].join(",")
                    )
                }
                _ => label.clone(),
            };
            ExpressionPromptSuggestion {
                symbol: match entry.kind {
                    ExpressionAbsolutePathKind::Group => {
                        configure::configured_symbol(|symbols| symbols.tree.folder_closed_leaf)
                            .to_string()
                    }
                    ExpressionAbsolutePathKind::Dataset => {
                        configure::configured_symbol(|symbols| symbols.tree.dataset_icon)
                            .to_string()
                    }
                },
                insert_text,
                label,
                detail: entry.detail,
                kind: match entry.kind {
                    ExpressionAbsolutePathKind::Group => ExpressionPromptSuggestionKind::Group,
                    ExpressionAbsolutePathKind::Dataset => ExpressionPromptSuggestionKind::Dataset,
                },
            }
        })
        .collect()
}

fn expression_absolute_path_entries(file: &File) -> Vec<ExpressionAbsolutePathEntry> {
    let Ok(root) = file.as_group() else {
        return Vec::new();
    };
    full_traversal(&root)
        .into_iter()
        .filter_map(|path| {
            if let Ok(dataset) = file.dataset(&path) {
                let shape = dataset.shape();
                Some(ExpressionAbsolutePathEntry {
                    detail: format_shape_suffix(&shape),
                    path,
                    kind: ExpressionAbsolutePathKind::Dataset,
                    shape: Some(shape),
                })
            } else if file.group(&path).is_ok() {
                Some(ExpressionAbsolutePathEntry {
                    path,
                    kind: ExpressionAbsolutePathKind::Group,
                    shape: None,
                    detail: String::new(),
                })
            } else {
                None
            }
        })
        .collect()
}

fn format_shape_suffix(shape: &[usize]) -> String {
    format!(
        "[{}]",
        shape
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(",")
    )
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

    let points = sanitize_chart_points(
        values
            .into_iter()
            .enumerate()
            .map(|(idx, value)| (idx as f64, value))
            .collect(),
    );
    if points.is_empty() {
        return Err(format!(
            "Series reference {reference} resolved to no finite points"
        ));
    }
    Ok(points)
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

    #[test]
    fn compact_selection_summary_uses_concise_array_notation() {
        let one_d = DatasetChartSource {
            dataset_path: "/raw/ds".to_string(),
            display_path: "/group/chunked_dataset".to_string(),
            selection: PreviewSelection {
                index: vec![0],
                x: 0,
                slice: SliceSelection::All,
            },
            shape: vec![64],
            kind: DatasetChartKind::Dataset,
        };
        assert_eq!(one_d.compact_selection_summary(), "chunked_dataset");

        let three_d = DatasetChartSource {
            dataset_path: "/raw/ds".to_string(),
            display_path: "/group/chunked_dataset".to_string(),
            selection: PreviewSelection {
                index: vec![0, 25, 1],
                x: 0,
                slice: SliceSelection::All,
            },
            shape: vec![64, 32, 8],
            kind: DatasetChartKind::Dataset,
        };
        assert_eq!(
            three_d.compact_selection_summary(),
            "chunked_dataset[..,25,1]"
        );

        let swapped = DatasetChartSource {
            selection: PreviewSelection {
                index: vec![5, 0, 0],
                x: 2,
                slice: SliceSelection::All,
            },
            ..three_d.clone()
        };
        assert_eq!(
            swapped.compact_selection_summary(),
            "chunked_dataset[5,0,..]"
        );

        let sliced = DatasetChartSource {
            selection: PreviewSelection {
                index: vec![0],
                x: 0,
                slice: SliceSelection::FromTo(5, 12),
            },
            shape: vec![64],
            ..one_d
        };
        assert_eq!(sliced.compact_selection_summary(), "chunked_dataset[5..12]");
    }

    #[test]
    fn chart_item_statistics_compute_mean_median_and_stddev() {
        let item = ChartItem {
            id: ChartItemId(1),
            color_slot: 0,
            label: "series".to_string(),
            source: ChartSource::DerivedExpression {
                expression: "series".to_string(),
                input_ids: vec![],
                len: 4,
                kind: DerivedExpressionKind::YSeries,
            },
            series: ChartSeries::from_points(vec![(1.0, 1.0), (2.0, 3.0), (3.0, 5.0), (4.0, 7.0)])
                .expect("series"),
            visible: true,
        };

        let stats = item.statistics();
        assert_eq!(stats.samples, 4);
        assert_eq!(stats.x_min, 1.0);
        assert_eq!(stats.x_max, 4.0);
        assert_eq!(stats.y_min, 1.0);
        assert_eq!(stats.y_max, 7.0);
        assert_eq!(stats.mean, 4.0);
        assert_eq!(stats.median, 4.0);
        assert!((stats.stddev - (5.0_f64).sqrt()).abs() < 1e-9);
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
    fn parses_dataset_slices_with_explicit_ranges() {
        let tokens = tokenize_expression("!/matrix[2,..10,0] + !/matrix[5,5..15,0]").unwrap();
        assert!(tokens.iter().any(|token| matches!(
            token,
            ExpressionToken::SeriesRef(ExpressionSeriesRef {
                target: ExpressionObjectTarget::AbsolutePath(path),
                attr_name: None,
                selectors: Some(selectors),
            }) if path == "/matrix"
                    && selectors
                        == &vec![
                            ExpressionDatasetSelector::Index(2),
                            ExpressionDatasetSelector::Slice { start: None, end: Some(10) },
                            ExpressionDatasetSelector::Index(0),
                        ]
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
                            ExpressionDatasetSelector::Index(5),
                            ExpressionDatasetSelector::Slice { start: Some(5), end: Some(15) },
                            ExpressionDatasetSelector::Index(0),
                        ]
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
    fn dataset_path_reference_builds_preview_selection_from_range_slice() {
        let dataset_ref = ExpressionSeriesRef {
            target: ExpressionObjectTarget::AbsolutePath("/matrix".to_string()),
            attr_name: None,
            selectors: Some(vec![
                ExpressionDatasetSelector::Index(5),
                ExpressionDatasetSelector::Slice {
                    start: Some(5),
                    end: Some(15),
                },
                ExpressionDatasetSelector::Index(0),
            ]),
        };
        let selection = dataset_ref.to_preview_selection(&[10, 20, 3]).unwrap();
        assert_eq!(selection.x, 1);
        assert_eq!(selection.index, vec![5, 0, 0]);
        assert_eq!(selection.slice, SliceSelection::FromTo(5, 15));
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
        assert!(err.contains("exactly one slice axis selector"));
    }

    #[test]
    fn current_expression_fragment_keeps_commas_inside_dataset_selectors() {
        let buffer = "!/matrix[..,2,0] + $1";
        let cursor = buffer.find(",2").unwrap() + 1;
        let (_, _, fragment) = current_expression_fragment(buffer, cursor).unwrap();
        assert_eq!(fragment, "!/matrix[..,2,0]");
    }

    #[test]
    fn consume_expression_reference_fragment_keeps_commas_inside_dataset_selectors() {
        let buffer = "!/matrix[5,5..15,0] + $1";
        let chars: Vec<(usize, char)> = buffer.char_indices().collect();
        let end = consume_expression_reference_fragment(buffer, &chars, 0);
        assert_eq!(&buffer[..end], "!/matrix[5,5..15,0]");
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
    fn chart_plot_area_conversion_respects_padding() {
        let plot_area =
            chart_plot_area_in_rect(Rect::new(10, 5, 20, 8), 200, 80, 40..180, 10..70).unwrap();
        assert_eq!(plot_area, Rect::new(14, 6, 14, 6));
    }

    #[test]
    fn zoom_at_position_ignores_chart_padding() {
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
        state.last_chart_area =
            chart_plot_area_in_rect(Rect::new(10, 5, 20, 8), 200, 80, 40..180, 10..70);

        assert!(!state.zoom_in_at_position(11, 6, 10.0));
        assert_eq!(state.aoi_from, None);
        assert_eq!(state.aoi_to, None);

        assert!(state.zoom_in_at_position(14, 6, 10.0));
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

    #[test]
    fn chart_series_filters_non_finite_points() {
        let series = ChartSeries::from_points(vec![
            (0.0, 1.0),
            (1.0, f64::NAN),
            (f64::INFINITY, 2.0),
            (2.0, 3.0),
        ])
        .expect("finite points should remain");
        assert_eq!(series.points, vec![(0.0, 1.0), (2.0, 3.0)]);
        assert_eq!(series.y_min, 1.0);
        assert_eq!(series.y_max, 3.0);
    }

    #[test]
    fn dataset_plot_preview_filters_non_finite_points() {
        let preview = dataset_ploting_data_from_points(vec![
            (0.0, f64::NAN),
            (1.0, 4.0),
            (2.0, f64::INFINITY),
            (3.0, 6.0),
        ])
        .expect("finite preview points");
        assert_eq!(preview.data, vec![(1.0, 4.0), (3.0, 6.0)]);
        assert_eq!(preview.min, 4.0);
        assert_eq!(preview.max, 6.0);
    }

    #[test]
    fn prepared_chart_data_filters_legacy_non_finite_points() {
        let mut state = make_state();
        let selection = PreviewSelection {
            index: vec![0],
            x: 0,
            slice: SliceSelection::All,
        };
        state.add_chart_item(
            source("/group/a", selection),
            vec![(0.0, 1.0), (1.0, 2.0), (2.0, 3.0)],
        );
        state.items[0].series.points[1] = (1.0, f64::NAN);

        let prepared = state.prepared_chart_data().expect("prepared chart data");
        assert_eq!(prepared.series.len(), 1);
        assert_eq!(prepared.series[0].points, vec![(0.0, 1.0), (2.0, 3.0)]);
        assert_eq!(prepared.y_min, 1.0);
        assert_eq!(prepared.y_max, 3.0);
    }

    #[test]
    fn prepared_chart_data_respects_visibility_and_viewport() {
        let mut state = make_state();
        let selection = PreviewSelection {
            index: vec![0],
            x: 0,
            slice: SliceSelection::All,
        };
        state.add_chart_item(
            source("/group/a", selection.clone()),
            (0..6).map(|i| (i as f64, i as f64)).collect(),
        );
        state.add_chart_item(
            source("/group/b", selection),
            (0..6).map(|i| (i as f64, (i * 10) as f64)).collect(),
        );
        state.items[1].visible = false;
        state.aoi_from = Some(1);
        state.aoi_to = Some(4);

        let prepared = state.prepared_chart_data().expect("prepared chart data");
        assert_eq!(prepared.series.len(), 1);
        assert_eq!(
            prepared.series[0].points,
            vec![(1.0, 1.0), (2.0, 2.0), (3.0, 3.0)]
        );
        assert_eq!(prepared.plot_x_min, 1.0);
        assert_eq!(prepared.plot_x_max, 3.0);
        assert_eq!(prepared.y_min, 1.0);
        assert_eq!(prepared.y_max, 3.0);
    }
}
