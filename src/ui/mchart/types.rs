use ratatui::layout::Rect;

use super::{
    model::{ChartSource, MultiChartLoadState, MultiChartViewMode, Point},
    MultiChartLoadRequest,
};

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct ChartViewport {
    pub(super) x_min: f64,
    pub(super) x_max: f64,
    pub(super) y_min: f64,
    pub(super) y_max: f64,
}

#[derive(Debug, Clone, Copy)]
pub(in crate::ui) struct MultiChartItemHitbox {
    pub(in crate::ui) area: Rect,
    pub(in crate::ui) index: usize,
}

#[derive(Debug, Clone, Copy)]
pub(in crate::ui) struct MultiChartEditorHitbox {
    pub(in crate::ui) area: Rect,
    pub(in crate::ui) name_area: Rect,
    pub(in crate::ui) expression_area: Rect,
}

#[derive(Debug, Clone, Copy)]
pub(in crate::ui) struct MultiChartViewModeHitbox {
    pub(in crate::ui) area: Rect,
    pub(in crate::ui) mode: MultiChartViewMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ChartZoomMode {
    Uniform,
    XOnly,
    YOnly,
}

#[derive(Debug, Clone)]
pub(super) struct ChartDragState {
    pub(super) anchor_column: u16,
    pub(super) anchor_row: u16,
    pub(super) viewport: ChartViewport,
}

#[derive(Debug, Clone)]
pub(super) struct PreparedLineChartSeries {
    pub(super) label: String,
    pub(super) color_slot: usize,
    pub(super) points: Vec<Point>,
    pub(super) is_selected: bool,
}

#[derive(Debug, Clone)]
pub(super) struct PreparedLineChartData {
    pub(super) plot_x_min: f64,
    pub(super) plot_x_max: f64,
    pub(super) y_min: f64,
    pub(super) y_max: f64,
    pub(super) series: Vec<PreparedLineChartSeries>,
}

#[derive(Debug, Clone)]
pub(super) struct PreparedHistogramBin {
    pub(super) start: f64,
    pub(super) end: f64,
    pub(super) count: f64,
}

#[derive(Debug, Clone)]
pub(super) struct PreparedHistogramSeries {
    pub(super) label: String,
    pub(super) color_slot: usize,
    pub(super) bins: Vec<PreparedHistogramBin>,
    pub(super) is_selected: bool,
}

#[derive(Debug, Clone)]
pub(super) struct PreparedHistogramData {
    pub(super) value_min: f64,
    pub(super) value_max: f64,
    pub(super) count_max: f64,
    pub(super) bin_count: usize,
    pub(super) series: Vec<PreparedHistogramSeries>,
}

#[derive(Debug, Clone)]
pub(super) struct PreparedBoxPlotSeries {
    pub(super) label: String,
    pub(super) color_slot: usize,
    pub(super) x_index: usize,
    pub(super) q1: f64,
    pub(super) median: f64,
    pub(super) q3: f64,
    pub(super) whisker_low: f64,
    pub(super) whisker_high: f64,
    pub(super) outliers: Vec<f64>,
    pub(super) is_selected: bool,
}

#[derive(Debug, Clone)]
pub(super) struct PreparedBoxPlotData {
    pub(super) value_min: f64,
    pub(super) value_max: f64,
    pub(super) series: Vec<PreparedBoxPlotSeries>,
}

#[derive(Debug, Clone)]
pub(super) struct PreparedComparisonScatterData {
    pub(super) label: String,
    pub(super) x_label: String,
    pub(super) y_label: String,
    pub(super) color_slot: usize,
    pub(super) points: Vec<Point>,
    pub(super) x_min: f64,
    pub(super) x_max: f64,
    pub(super) y_min: f64,
    pub(super) y_max: f64,
    pub(super) truncation_note: Option<String>,
}

pub(crate) struct ChartItemStatus {
    pub source: ChartSource,
    pub points: Option<Vec<Point>>,
    pub scalar_value: Option<f64>,
    pub source_len: usize,
    pub load_state: MultiChartLoadState,
    pub sampled: bool,
}

#[derive(Debug, Clone)]
pub(super) enum PreparedChartData {
    Line(PreparedLineChartData),
    Histogram(PreparedHistogramData),
    BoxPlot(PreparedBoxPlotData),
    ComparisonScatter(PreparedComparisonScatterData),
}

#[derive(Debug, Clone)]
pub struct MultiChartRenderRequest {
    pub(super) generation: u64,
    pub(super) chart_area: Rect,
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) prepared: PreparedChartData,
}

#[derive(Debug, Clone)]
pub enum MultiChartRenderResult {
    Success {
        generation: u64,
        chart_area: Rect,
        width: u32,
        height: u32,
        rgb_bytes: Vec<u8>,
        plot_x_range: std::ops::Range<i32>,
        plot_y_range: std::ops::Range<i32>,
    },
    Failure {
        generation: u64,
        message: String,
    },
}

#[derive(Debug, Clone)]
pub struct CapturedMultiChartItem {
    pub source: ChartSource,
    pub initial_points: Option<Vec<Point>>,
    pub source_len: usize,
    pub load_state: MultiChartLoadState,
    pub request: Option<MultiChartLoadRequest>,
}
