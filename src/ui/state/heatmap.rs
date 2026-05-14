use std::{
    collections::{HashSet, VecDeque},
    sync::mpsc::Sender,
};

use hdf5_metno::Dataset;
use ratatui_image::protocol::StatefulProtocol;

use crate::h5f::DatasetMeta;

#[derive(Debug, Clone, PartialEq)]
pub struct HeatmapRegionSelection {
    pub x: usize,
    pub y: usize,
    pub width: usize,
    pub height: usize,
    pub min: f64,
    pub max: f64,
    pub mean: f64,
    pub stddev: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HeatmapSelectedCells {
    pub row_start: usize,
    pub row_end: usize,
    pub col_start: usize,
    pub col_end: usize,
}

impl HeatmapSelectedCells {
    pub fn single(row: usize, col: usize) -> Self {
        Self {
            row_start: row,
            row_end: row,
            col_start: col,
            col_end: col,
        }
    }

    pub fn normalized(a_row: usize, a_col: usize, b_row: usize, b_col: usize) -> Self {
        Self {
            row_start: a_row.min(b_row),
            row_end: a_row.max(b_row),
            col_start: a_col.min(b_col),
            col_end: a_col.max(b_col),
        }
    }

    pub fn is_single(self) -> bool {
        self.row_start == self.row_end && self.col_start == self.col_end
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HeatmapViewport {
    pub row_start: usize,
    pub row_len: usize,
    pub col_start: usize,
    pub col_len: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HeatmapSliceSummary {
    pub min: f64,
    pub max: f64,
    pub has_finite: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HeatmapLegendSummary {
    pub min: f64,
    pub max: f64,
    pub has_finite: bool,
    pub histogram: Vec<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HeatmapColormap {
    Turbo,
    Grayscale,
    Inferno,
}

impl HeatmapColormap {
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "turbo" => Some(Self::Turbo),
            "grayscale" | "greyscale" | "gray" | "grey" => Some(Self::Grayscale),
            "inferno" => Some(Self::Inferno),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Turbo => "turbo",
            Self::Grayscale => "grayscale",
            Self::Inferno => "inferno",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Turbo => "Turbo",
            Self::Grayscale => "Gray",
            Self::Inferno => "Inferno",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HeatmapStoredFloat(pub u64);

impl HeatmapStoredFloat {
    pub fn from_f64(value: f64) -> Option<Self> {
        value.is_finite().then_some(Self(value.to_bits()))
    }

    pub fn to_f64(self) -> f64 {
        f64::from_bits(self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HeatmapRangeBound {
    Exact(HeatmapStoredFloat),
    Percentile(u16),
}

impl HeatmapRangeBound {
    pub fn parse(token: &str) -> std::result::Result<Self, String> {
        let trimmed = token.trim();
        if let Some(percent) = trimmed.strip_suffix('%') {
            let value = percent.parse::<f64>().map_err(|_| {
                format!("Invalid heatmap percentile bound '{trimmed}'. Use values like 5% or 99%")
            })?;
            if !(0.0..=100.0).contains(&value) {
                return Err(format!(
                    "Heatmap percentile bound '{trimmed}' must be between 0% and 100%"
                ));
            }
            Ok(Self::Percentile((value * 100.0).round() as u16))
        } else {
            let value = trimmed.parse::<f64>().map_err(|_| {
                format!("Invalid heatmap exact bound '{trimmed}'. Use a number or percentage")
            })?;
            let stored = HeatmapStoredFloat::from_f64(value)
                .ok_or_else(|| format!("Heatmap exact bound '{trimmed}' must be finite"))?;
            Ok(Self::Exact(stored))
        }
    }

    pub fn label(self) -> String {
        match self {
            Self::Exact(value) => format_heatmap_scalar(value.to_f64()),
            Self::Percentile(bps) => format!("{}%", format_heatmap_percent(bps)),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct HeatmapCustomRangeMode {
    pub label: String,
    pub lower: HeatmapRangeBound,
    pub upper: HeatmapRangeBound,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum HeatmapRangeMode {
    Auto,
    MinMax,
    Percentile { lower_bps: u16, upper_bps: u16 },
    SigmaClip { sigma_milli: u16 },
    Winsorized { lower_bps: u16, upper_bps: u16 },
    Custom(HeatmapCustomRangeMode),
}

impl HeatmapRangeMode {
    pub fn default_modes() -> Vec<Self> {
        vec![
            Self::Auto,
            Self::MinMax,
            Self::Percentile {
                lower_bps: 100,
                upper_bps: 9900,
            },
            Self::SigmaClip { sigma_milli: 2000 },
            Self::Winsorized {
                lower_bps: 200,
                upper_bps: 9800,
            },
        ]
    }

    pub fn custom(
        lower: HeatmapRangeBound,
        upper: HeatmapRangeBound,
        label: Option<String>,
    ) -> Self {
        let label = label
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| format!("{}..{}", lower.label(), upper.label()));
        Self::Custom(HeatmapCustomRangeMode {
            label,
            lower,
            upper,
        })
    }

    pub fn label(&self) -> String {
        match self {
            Self::Auto => "Auto".to_string(),
            Self::MinMax => "MIN/MAX".to_string(),
            Self::Percentile {
                lower_bps,
                upper_bps,
            } => format!(
                "Clip {}-{}%",
                format_heatmap_percent(*lower_bps),
                format_heatmap_percent(*upper_bps)
            ),
            Self::SigmaClip { sigma_milli } => {
                format!("Sigma ±{}σ", format_heatmap_thousandths(*sigma_milli))
            }
            Self::Winsorized {
                lower_bps,
                upper_bps,
            } => format!(
                "Winsor {}-{}%",
                format_heatmap_percent(*lower_bps),
                format_heatmap_percent(*upper_bps)
            ),
            Self::Custom(mode) => mode.label.clone(),
        }
    }

    pub fn selector_matches(&self, selector: &str) -> bool {
        let selector = normalize_heatmap_range_selector(selector);
        if normalize_heatmap_range_selector(&self.label()) == selector {
            return true;
        }
        match self {
            Self::Auto => selector == "auto",
            Self::MinMax => matches!(selector.as_str(), "min/max" | "minmax" | "min-max" | "type"),
            Self::Percentile {
                lower_bps: 100,
                upper_bps: 9900,
            } => matches!(
                selector.as_str(),
                "clip-1-99%" | "clip-1-99" | "1-99%" | "1-99" | "percentile-1-99"
            ),
            Self::SigmaClip { sigma_milli: 2000 } => {
                matches!(
                    selector.as_str(),
                    "sigma" | "sigma-2" | "sigma-2.0" | "2-sigma"
                )
            }
            Self::Winsorized {
                lower_bps: 200,
                upper_bps: 9800,
            } => matches!(
                selector.as_str(),
                "winsor" | "winsor-2-98%" | "winsor-2-98" | "winsorized-2-98"
            ),
            Self::Custom(mode) => normalize_heatmap_range_selector(&mode.label) == selector,
            _ => false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HeatmapNormalization {
    Linear,
    Log,
    Sqrt,
}

impl HeatmapNormalization {
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "linear" => Some(Self::Linear),
            "log" | "log10" | "logarithmic" => Some(Self::Log),
            "sqrt" | "square-root" | "square_root" => Some(Self::Sqrt),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Linear => "linear",
            Self::Log => "log",
            Self::Sqrt => "sqrt",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Linear => "Linear",
            Self::Log => "Log",
            Self::Sqrt => "Sqrt",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeatmapSettingField {
    Colormap,
    Range,
    InvertX,
    InvertY,
    InvertC,
    Normalization,
}

pub const HEATMAP_SETTING_FIELDS: [HeatmapSettingField; 6] = [
    HeatmapSettingField::Colormap,
    HeatmapSettingField::Range,
    HeatmapSettingField::InvertX,
    HeatmapSettingField::InvertY,
    HeatmapSettingField::InvertC,
    HeatmapSettingField::Normalization,
];

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct HeatmapSettings {
    pub colormap: HeatmapColormap,
    pub range: HeatmapRangeMode,
    pub invert_x: bool,
    pub invert_y: bool,
    pub invert_c: bool,
    pub normalization: HeatmapNormalization,
}

impl Default for HeatmapSettings {
    fn default() -> Self {
        Self {
            colormap: HeatmapColormap::Turbo,
            range: HeatmapRangeMode::Auto,
            invert_x: false,
            invert_y: false,
            invert_c: false,
            normalization: HeatmapNormalization::Linear,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HeatmapSegmentAxis {
    Rows,
    Cols,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeatmapPageWindow {
    pub ds_path: String,
    pub axis: HeatmapSegmentAxis,
    pub len: usize,
    pub total: usize,
    pub page: i32,
    pub page_count: i32,
}

impl HeatmapPageWindow {
    pub fn step_len(&self) -> usize {
        (self.len / 2).max(1)
    }

    pub fn start_for_page(&self, page: i32) -> usize {
        let page = page.clamp(0, self.page_count.saturating_sub(1));
        (page.max(0) as usize) * self.step_len()
    }

    pub fn range_for_page(&self, page: i32) -> (usize, usize) {
        let start = self.start_for_page(page);
        let end = (start + self.len).min(self.total);
        (start, end)
    }

    pub fn current_range(&self) -> (usize, usize) {
        self.range_for_page(self.page)
    }

    pub fn label(&self) -> &'static str {
        match self.axis {
            HeatmapSegmentAxis::Rows => "rows",
            HeatmapSegmentAxis::Cols => "cols",
        }
    }
}

impl HeatmapSettings {
    pub fn adjust(&mut self, field: HeatmapSettingField, delta: isize) {
        match field {
            HeatmapSettingField::Colormap => {
                self.colormap = match (self.colormap, delta.signum()) {
                    (HeatmapColormap::Turbo, d) if d < 0 => HeatmapColormap::Inferno,
                    (HeatmapColormap::Turbo, _) => HeatmapColormap::Grayscale,
                    (HeatmapColormap::Grayscale, d) if d < 0 => HeatmapColormap::Turbo,
                    (HeatmapColormap::Grayscale, _) => HeatmapColormap::Inferno,
                    (HeatmapColormap::Inferno, d) if d < 0 => HeatmapColormap::Grayscale,
                    (HeatmapColormap::Inferno, _) => HeatmapColormap::Turbo,
                };
            }
            HeatmapSettingField::Range => {}
            HeatmapSettingField::InvertX => self.invert_x = !self.invert_x,
            HeatmapSettingField::InvertY => self.invert_y = !self.invert_y,
            HeatmapSettingField::InvertC => self.invert_c = !self.invert_c,
            HeatmapSettingField::Normalization => {
                self.normalization = match (self.normalization, delta.signum()) {
                    (HeatmapNormalization::Linear, d) if d < 0 => HeatmapNormalization::Sqrt,
                    (HeatmapNormalization::Linear, _) => HeatmapNormalization::Log,
                    (HeatmapNormalization::Log, d) if d < 0 => HeatmapNormalization::Linear,
                    (HeatmapNormalization::Log, _) => HeatmapNormalization::Sqrt,
                    (HeatmapNormalization::Sqrt, d) if d < 0 => HeatmapNormalization::Log,
                    (HeatmapNormalization::Sqrt, _) => HeatmapNormalization::Linear,
                };
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct HeatmapRenderKey {
    pub ds_path: String,
    pub width: u16,
    pub height: u16,
    pub cell_width: u16,
    pub cell_height: u16,
    pub viewport: Option<HeatmapViewport>,
    pub segment_axis: Option<HeatmapSegmentAxis>,
    pub segment_start: usize,
    pub segment_len: usize,
    pub selected_row: usize,
    pub selected_col: usize,
    pub selected_indexes: Vec<usize>,
    pub selected_cells: Option<HeatmapSelectedCells>,
    pub settings: HeatmapSettings,
}

pub struct HeatmapLoadRequest {
    pub key: HeatmapRenderKey,
    pub dataset: Dataset,
    pub attr: DatasetMeta,
    pub priority: HeatmapLoadPriority,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeatmapLoadPriority {
    Current,
    Prefetch,
}

pub struct HeatmapLoadedPage {
    pub key: HeatmapRenderKey,
    pub pixel_width: u32,
    pub pixel_height: u32,
    pub rgb_bytes: Vec<u8>,
    pub slice_summary: HeatmapSliceSummary,
    pub legend_summary: HeatmapLegendSummary,
    pub viewport_selection: HeatmapRegionSelection,
    pub selection: Option<HeatmapRegionSelection>,
}

pub struct HeatmapCachedPage {
    pub key: HeatmapRenderKey,
    pub protocol: StatefulProtocol,
    pub slice_summary: HeatmapSliceSummary,
    pub legend_summary: HeatmapLegendSummary,
    pub viewport_selection: HeatmapRegionSelection,
    pub selection: Option<HeatmapRegionSelection>,
}

#[derive(Debug, Clone, Copy)]
pub struct HeatmapDragState {
    pub anchor_row: usize,
    pub anchor_col: usize,
    pub visible_viewport: HeatmapViewport,
}

pub struct HeatmapRenderState {
    pub current_key: Option<HeatmapRenderKey>,
    pub current_selection: Option<HeatmapRegionSelection>,
    pub current_slice_summary: Option<HeatmapSliceSummary>,
    pub viewport: Option<HeatmapViewport>,
    pub selected_cells: Option<HeatmapSelectedCells>,
    pub drag_state: Option<HeatmapDragState>,
    pub segment: Option<HeatmapPageWindow>,
    pub cached_pages: VecDeque<HeatmapCachedPage>,
    pub pending_keys: HashSet<HeatmapRenderKey>,
    pub tx_load_heatmap: Sender<HeatmapLoadRequest>,
    pub settings: HeatmapSettings,
    pub selected_setting: usize,
    pub session_range_modes: Vec<HeatmapRangeMode>,
}

impl HeatmapRegionSelection {
    pub fn summary(&self) -> String {
        format!(
            "x={} y={} width={} height={} mean={:.6} stddev={:.6} min={:.6} max={:.6}",
            self.x, self.y, self.width, self.height, self.mean, self.stddev, self.min, self.max
        )
    }
}

pub(super) fn heatmap_partition(total: usize, cells: usize, index: usize) -> (usize, usize) {
    let start = (index * total) / cells.max(1);
    let mut end = ((index + 1) * total) / cells.max(1);
    if end <= start {
        end = (start + 1).min(total);
    }
    (start, end)
}

pub(super) fn heatmap_anchor_fraction(index: usize, cells: usize, inverted: bool) -> f64 {
    let display_fraction = (index as f64 + 0.5) / cells.max(1) as f64;
    if inverted {
        1.0 - display_fraction
    } else {
        display_fraction
    }
}

fn format_heatmap_percent(bps: u16) -> String {
    let whole = bps / 100;
    let frac = bps % 100;
    if frac == 0 {
        whole.to_string()
    } else if frac.is_multiple_of(10) {
        format!("{whole}.{}", frac / 10)
    } else {
        format!("{whole}.{frac:02}")
    }
}

fn format_heatmap_thousandths(value: u16) -> String {
    let whole = value / 1000;
    let frac = value % 1000;
    if frac == 0 {
        whole.to_string()
    } else if frac.is_multiple_of(100) {
        format!("{whole}.{}", frac / 100)
    } else if frac.is_multiple_of(10) {
        format!("{whole}.{:02}", frac / 10)
    } else {
        format!("{whole}.{frac:03}")
    }
}

fn format_heatmap_scalar(value: f64) -> String {
    format!("{value}")
}

fn normalize_heatmap_range_selector(selector: &str) -> String {
    selector
        .trim()
        .to_ascii_lowercase()
        .replace([' ', '_'], "-")
}

pub(super) fn clamp_heatmap_viewport(
    mut viewport: HeatmapViewport,
    rows: usize,
    cols: usize,
) -> HeatmapViewport {
    viewport.row_len = viewport.row_len.clamp(1, rows.max(1));
    viewport.col_len = viewport.col_len.clamp(1, cols.max(1));
    viewport.row_start = viewport
        .row_start
        .min(rows.saturating_sub(viewport.row_len));
    viewport.col_start = viewport
        .col_start
        .min(cols.saturating_sub(viewport.col_len));
    viewport
}
