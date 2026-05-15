use crate::data::{PreviewSelection, SliceSelection};

pub type Point = (f64, f64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChartXAxisPolicy {
    SampleIndex,
}

impl ChartXAxisPolicy {
    pub(super) fn label(self) -> &'static str {
        match self {
            ChartXAxisPolicy::SampleIndex => "x values",
        }
    }

    pub(super) fn description(self) -> &'static str {
        match self {
            ChartXAxisPolicy::SampleIndex => {
                "derived inputs align by sample index; each series is plotted with its own x values"
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ChartItemId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DerivedExpressionKind {
    YSeries,
    XySeries,
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
            return format!("load({})", self.display_path);
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
        format!("load({})[{}]", self.display_path, selectors.join(","))
    }

    pub fn slice_summary(&self) -> String {
        match self.selection.slice {
            SliceSelection::All => "slice=all".to_string(),
            SliceSelection::FromTo(start, end) => format!("slice={start}..{end}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ChartSource {
    DatasetSelection(DatasetChartSource),
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
            ChartSource::DerivedExpression { .. } => false,
        }
    }

    pub(super) fn label(&self) -> String {
        match self {
            ChartSource::DatasetSelection(source) => source.compact_selection_summary(),
            ChartSource::DerivedExpression { expression, .. } => expression.clone(),
        }
    }

    pub(super) fn source_kind_label(&self) -> &'static str {
        match self {
            ChartSource::DatasetSelection(source) => source.kind_label(),
            ChartSource::DerivedExpression { kind, .. } => match kind {
                DerivedExpressionKind::YSeries => "expression",
                DerivedExpressionKind::XySeries => "expression x/y",
            },
        }
    }

    pub(super) fn dataset_source(&self) -> Option<&DatasetChartSource> {
        match self {
            ChartSource::DatasetSelection(source) => Some(source),
            ChartSource::DerivedExpression { .. } => None,
        }
    }

    pub(super) fn editable_expression(&self) -> Option<String> {
        match self {
            ChartSource::DatasetSelection(source) => Some(source.expression_reference()),
            ChartSource::DerivedExpression { expression, .. } => Some(expression.clone()),
        }
    }

    pub(super) fn input_ids(&self) -> &[ChartItemId] {
        match self {
            ChartSource::DerivedExpression { input_ids, .. } => input_ids,
            ChartSource::DatasetSelection(_) => &[],
        }
    }
}

#[derive(Debug, Clone)]
pub struct ChartSeries {
    pub(super) points: Vec<Point>,
    pub(super) y_max: f64,
    pub(super) y_min: f64,
}

fn is_finite_chart_point((x, y): Point) -> bool {
    x.is_finite() && y.is_finite()
}

pub(super) fn sanitize_chart_points(points: Vec<Point>) -> Vec<Point> {
    points
        .into_iter()
        .filter(|point| is_finite_chart_point(*point))
        .collect()
}

impl ChartSeries {
    pub(super) fn from_points(points: Vec<Point>) -> Option<Self> {
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
        Some(Self {
            points,
            y_max,
            y_min,
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
    pub detail_series: Option<ChartSeries>,
    pub detail_window: Option<ChartLodWindow>,
    pub pending_detail_window: Option<ChartLodWindow>,
    pub detail_generation: u64,
    pub source_len: usize,
    pub sampled: bool,
    pub load_state: MultiChartLoadState,
    pub visible: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChartLodWindow {
    pub start: usize,
    pub end: usize,
    pub sample_cap: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MultiChartLoadState {
    Queued,
    Sampling,
    Refining,
    Ready,
    Error(String),
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

    pub(super) fn editable_expression(&self) -> Option<String> {
        self.source.editable_expression()
    }

    pub fn list_label(&self) -> String {
        match &self.source {
            ChartSource::DatasetSelection(source) => source.expression_reference(),
            ChartSource::DerivedExpression { expression, .. } => expression.clone(),
        }
    }

    pub fn data_state_label(&self) -> String {
        match &self.load_state {
            MultiChartLoadState::Queued => "queued".to_string(),
            MultiChartLoadState::Sampling => "sampling".to_string(),
            MultiChartLoadState::Refining => "refining".to_string(),
            MultiChartLoadState::Ready => {
                if self.detail_window.is_some() {
                    format!("detail {}/{}", self.active_series().len(), self.source_len)
                } else if self.sampled {
                    format!("sampled {}/{}", self.series.len(), self.source_len)
                } else {
                    format!("ready {}", self.series.len())
                }
            }
            MultiChartLoadState::Error(message) => format!("error: {message}"),
        }
    }

    pub fn has_loaded_series(&self) -> bool {
        matches!(
            self.load_state,
            MultiChartLoadState::Ready | MultiChartLoadState::Refining
        )
    }

    pub fn active_series(&self) -> &ChartSeries {
        self.detail_series.as_ref().unwrap_or(&self.series)
    }

    pub fn overview_series(&self) -> &ChartSeries {
        &self.series
    }

    pub(super) fn clear_detail_state(&mut self, invalidate_generation: bool) {
        self.detail_series = None;
        self.detail_window = None;
        self.pending_detail_window = None;
        if invalidate_generation {
            self.detail_generation = self.detail_generation.saturating_add(1);
        }
        if self.has_loaded_series() {
            self.load_state = MultiChartLoadState::Ready;
        }
    }

    pub fn statistics(&self) -> ChartItemStats {
        let mut ys = self
            .active_series()
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
            .active_series()
            .points
            .first()
            .map(|(x, _)| *x)
            .unwrap_or_default();
        let x_max = self
            .active_series()
            .points
            .last()
            .map(|(x, _)| *x)
            .unwrap_or_default();

        ChartItemStats {
            samples,
            x_min,
            x_max,
            y_min: self.active_series().y_min,
            y_max: self.active_series().y_max,
            mean,
            median,
            stddev: variance.sqrt(),
        }
    }
}
