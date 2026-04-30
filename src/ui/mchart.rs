use image::{DynamicImage, ImageBuffer, Rgb};
use plotters::{
    prelude::{BitMapBackend, IntoDrawingArea, WHITE},
    style::{Color as _, IntoFont, Palette},
};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Margin, Rect},
    style::{Color, Style, Stylize},
    text::{Line, Span, Text},
    widgets::{Block, BorderType, Borders, Paragraph, Wrap},
};
use ratatui_image::{picker::Picker, protocol::StatefulProtocol, StatefulImage};

use crate::{
    color_consts,
    data::{PreviewSelection, SliceSelection},
    error::log_error,
};

pub type Point = (f64, f64);

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
            ChartSource::DerivedExpression { .. } => "derived expression (phase two)",
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
        let rgb = plotters::prelude::Palette99::pick(self.color_slot).to_rgba();
        (rgb.0, rgb.1, rgb.2)
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

    fn item_by_id(&self, id: ChartItemId) -> Option<&ChartItem> {
        self.items.iter().find(|item| item.id == id)
    }

    fn marked_base_item_ref(&self) -> Option<&ChartItem> {
        self.marked_base_item.and_then(|id| self.item_by_id(id))
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

    pub fn zoom_in(&mut self, percent: f64) {
        let Some((min_x, max_x)) = self.global_x_bounds() else {
            return;
        };
        let actual_min = min_x.max(self.aoi_from.unwrap_or(min_x));
        let actual_max = max_x.min(self.aoi_to.unwrap_or(max_x));
        let range = actual_max.saturating_sub(actual_min);
        if range <= 1 {
            return;
        }
        let delta = range as f64 * percent / 100.0;
        let new_from = ((actual_min as f64 + delta).round() as usize).min(actual_max - 1);
        let new_to = ((actual_max as f64 - delta).round() as usize).max(actual_min + 1);
        self.aoi_from = Some(new_from);
        self.aoi_to = Some(new_to);
        self.modified = true;
    }

    pub fn clear_zoom(&mut self) {
        self.aoi_from = None;
        self.aoi_to = None;
        self.modified = true;
    }

    pub fn zoom_out(&mut self, percent: f64) {
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
        let delta = range as f64 * percent / 100.0;
        let new_min = (actual_min as f64 - delta).round() as usize;
        let new_max = (actual_max as f64 + delta).round() as usize;
        self.aoi_from = (new_min > min_x).then_some(new_min);
        self.aoi_to = (new_max < max_x).then_some(new_max);
        self.modified = true;
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
        self.plot_buffer = vec![0; (width * height * 3) as usize];
        let root =
            BitMapBackend::with_buffer(&mut self.plot_buffer, (width, height)).into_drawing_area();
        if let Err(e) = root.fill(&WHITE) {
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
            (item.label.clone(), item.color_slot, data_points)
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
            .y_label_style(("sans-serif", 18).into_font())
            .x_label_style(("sans-serif", 18).into_font())
            .draw()
        {
            log_error(e);
        }

        for (label, color_slot, data) in data_series {
            let color = plotters::prelude::Palette99::pick(color_slot);
            let line_series = plotters::prelude::LineSeries::new(data, &color);
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
                        stroke_width: 2,
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

        if self.items.is_empty() {
            self.render_empty(f, inner_area);
            return;
        }

        let panes = if inner_area.width < 110 {
            Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(11), Constraint::Min(12)])
                .split(inner_area)
        } else {
            Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Length(42), Constraint::Min(20)])
                .split(inner_area)
        };

        let (sidebar_area, chart_area) = (panes[0], panes[1]);
        let sidebar_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(5), Constraint::Length(6)])
            .split(sidebar_area);
        self.render_item_list(f, sidebar_chunks[0]);
        self.render_selected_details(f, sidebar_chunks[1]);
        self.render_chart_panel(f, chart_area);
    }

    fn render_empty(&self, f: &mut ratatui::Frame<'_>, area: Rect) {
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
                let color = plotters::prelude::Palette99::pick(item.color_slot).to_rgba();
                let marker = if item.visible { "●" } else { "○" };
                let prefix = if absolute_idx == self.idx { "> " } else { "  " };
                let hidden = if item.visible { "" } else { " hidden" };
                let base_marker = if self.marked_base_item == Some(item.id) {
                    " [base]"
                } else {
                    ""
                };
                Line::from(vec![
                    Span::raw(prefix),
                    Span::styled(
                        marker,
                        Style::default().fg(Color::Rgb(color.0, color.1, color.2)),
                    ),
                    Span::raw(" "),
                    Span::styled(
                        format!("{}{}{}", item.list_label(), base_marker, hidden),
                        if absolute_idx == self.idx {
                            Style::default().fg(color_consts::TITLE).bold()
                        } else {
                            Style::default().fg(color_consts::COLOR_WHITE)
                        },
                    ),
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
            .map(|item| item.list_label())
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
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
