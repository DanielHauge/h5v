use std::{
    cell::RefCell,
    fs,
    rc::Rc,
    sync::{Arc, RwLock},
    time::{Duration, Instant, SystemTime},
};

use arboard::Clipboard;
use hdf5_metno::File;

use crate::{
    configure,
    configure::registry::ContentModeHandle,
    data::{PreviewSelection, SliceSelection},
    error::AppError,
    h5f::{H5FNode, HasPath, Node},
    search::Searcher,
    ui::mchart::{
        CapturedMultiChartItem, ChartSource, DatasetChartKind, DatasetChartSource,
        MultiChartLoadRequest, MultiChartLoadSource, MultiChartLoadState, MultiChartState,
    },
};

use super::{
    command::{execute_command, CommandState},
    input::EventResult,
    preview::chart::MAX_PAGE_SIZE,
    tree_view::TreeItem,
};

mod content_modes;
mod core;
mod heatmap;
mod help_state;
mod movement;
mod navigation;
mod preview;
mod selection;
mod ui_layout;
#[allow(unused_imports)]
pub use core::{
    AppToast, AttributeCreateDialogState, AttributeCreateField, AttributeCursor,
    AttributeDeleteDialogState, AttributeEditRequest, AttributeViewSelection, ContentShowMode,
    FileWatchState, FixedStringOverflowChoice, FixedStringOverflowDialogState, Focus, LastFocused,
    LogLevelFilter, LogsFilterFocus, LogsViewState, MatrixViewState, Mode, PendingChord,
};
#[allow(unused_imports)]
use heatmap::heatmap_anchor_fraction;
#[allow(unused_imports)]
pub use heatmap::{
    HeatmapCachedPage, HeatmapColormap, HeatmapCustomRangeMode, HeatmapDragState,
    HeatmapLegendSummary, HeatmapLineProfile, HeatmapLineSelection, HeatmapLoadPriority,
    HeatmapLoadRequest, HeatmapLoadedPage, HeatmapNormalization, HeatmapPageAxis,
    HeatmapPageWindow, HeatmapProfileSample, HeatmapRangeBound, HeatmapRangeMode,
    HeatmapRegionSelection, HeatmapRenderKey, HeatmapRenderState, HeatmapSelectedCells,
    HeatmapSettingField, HeatmapSettings, HeatmapSliceSummary, HeatmapStoredFloat, HeatmapViewport,
    HEATMAP_SETTING_FIELDS,
};
pub use help_state::{
    HelpCommandSection, HelpCustomizationSection, HelpKeymapSection, HelpMultiChartSection,
    HelpTab, HelpViewState,
};
pub use preview::{
    ChartPreviewKey, ChartPreviewLoadRequest, ChartPreviewSource, ChartPreviwState,
    ClipboardImageData, DatasetImageLoadRequest, ImageLoadKey, ImageWindowAxis, ImageWindowState,
    ImgState, PageState, PageType, PreviewChartRoi, PreviewChartViewport, PreviewChartZoomMode,
    PreviewExpressionKey, PreviewExpressionRequest, PreviewExpressionResult,
    PreviewExpressionState, RawImageLoadRequest, VarLenImageLoadRequest,
    CHART_PREVIEW_CACHE_CAPACITY, PREVIEW_CHART_VISIBLE_POINT_LIMIT,
};
pub use ui_layout::{
    AttributesHitbox, ContentTabHitbox, HeatmapSettingHitbox, HelpScrollbarHitbox,
    HelpSidebarHitbox, HelpSidebarTarget, HelpTabHitbox, LogsFilterHitbox, LogsFilterTarget,
    MatrixCellHitbox, MatrixRowHitbox, MetadataCellHitbox, TreeHitbox, UiLayoutState,
};

pub struct AppState<'a> {
    pub readonly: bool,
    pub root: Rc<RefCell<H5FNode>>,
    pub treeview: Vec<TreeItem<'a>>,
    pub file: Option<File>,
    pub editing: bool,
    pub edit_pause: Arc<RwLock<()>>,
    pub tree_view_cursor: usize,
    pub clipboard: Option<Clipboard>,
    pub clipboard_init_error: Option<String>,
    pub copying: bool,
    pub toast: AppToast,
    pub toast_expires_at: Option<Instant>,
    pub configuration_warning: Option<String>,
    pub file_watch: FileWatchState,
    pub compatibility_mode: bool,
    pub focus: Focus,
    pub multi_chart: MultiChartState,
    pub mode: Mode,
    pub command_return_mode: Mode,
    pub help_return_mode: Mode,
    pub logs_return_mode: Mode,
    pub searcher: Option<Searcher>,
    pub help: HelpViewState,
    pub logs: LogsViewState,
    pub pending_chord: Option<PendingChord>,
    pub binding_command_depth: usize,
    pub show_tree_view: bool,
    pub stacked_tree_layout: bool,
    pub image_protocol_enabled: bool,
    pub image_cell_size: (u16, u16),
    pub preview_debounce_generation: u64,
    pub preview_debounce_until: Option<Instant>,
    pub preview_debounce_path: Option<String>,
    pub content_mode: ContentModeHandle,
    pub img_state: ImgState,
    pub matrix_view_state: MatrixViewState,
    pub heatmap_viewport_region: Option<HeatmapRegionSelection>,
    pub heatmap_region: Option<HeatmapRegionSelection>,
    pub heatmap_render: HeatmapRenderState,
    pub chart_preview_state: ChartPreviwState,
    pub preview_expression_state: PreviewExpressionState,
    pub page_state: PageState,
    pub command_state: CommandState,
    pub attribute_create_dialog: Option<AttributeCreateDialogState>,
    pub attribute_delete_dialog: Option<AttributeDeleteDialogState>,
    pub fixed_string_overflow_dialog: Option<FixedStringOverflowDialogState>,
    pub ui_layout: UiLayoutState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SeekAxis {
    Row,
    Col,
}

fn preferred_seek_axis(row_seekable: bool, col_seekable: bool) -> SeekAxis {
    if !row_seekable && col_seekable {
        SeekAxis::Col
    } else {
        SeekAxis::Row
    }
}

fn clamp_absolute_seek_start(target: usize, total: usize, visible: usize) -> usize {
    target.min(total.saturating_sub(visible.max(1)))
}

pub(crate) fn preview_selection_for_node(
    node: &mut H5FNode,
    shape: &[usize],
    page_idx: i32,
) -> Option<PreviewSelection> {
    let total_dims = shape.len();
    node.sync_selection_rank(total_dims);
    let x_selectable_dims: Vec<usize> = shape
        .iter()
        .enumerate()
        .filter(|(_, len)| **len > 1)
        .map(|(idx, _)| idx)
        .collect();

    if x_selectable_dims.is_empty() {
        return None;
    }

    for (idx, selected_index) in node.selected_indexes.iter_mut().enumerate() {
        if !x_selectable_dims.contains(&idx) {
            *selected_index = 0;
        }
    }

    if !x_selectable_dims.contains(&node.selected_x) {
        let first_selectable_dim = x_selectable_dims.first().copied()?;
        node.selected_x = first_selectable_dim;
    }
    if node.selected_dim == node.selected_x {
        node.selected_dim = x_selectable_dims
            .iter()
            .find(|&&x| x != node.selected_x)
            .copied()
            .unwrap_or(0);
    }

    let slice = if shape[node.selected_x] > MAX_PAGE_SIZE {
        let start = MAX_PAGE_SIZE * page_idx.max(0) as usize;
        let end = (start + MAX_PAGE_SIZE).min(shape[node.selected_x]);
        SliceSelection::FromTo(start, end)
    } else {
        SliceSelection::All
    };

    Some(PreviewSelection {
        x: node.selected_x,
        index: node.selected_indexes.get(..total_dims)?.to_vec(),
        slice,
    })
}

type Result<T> = std::result::Result<T, AppError>;
impl AppState<'_> {
    const PREVIEW_DEBOUNCE_DELAY: Duration = Duration::from_millis(90);
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::expect_used)]
mod tests {
    use super::{
        heatmap_anchor_fraction, preferred_seek_axis, HeatmapPageAxis, HeatmapPageWindow,
        HelpCommandSection, HelpCustomizationSection, HelpKeymapSection, HelpTab, HelpViewState,
        SeekAxis,
    };

    #[test]
    fn heatmap_anchor_fraction_uses_display_position_when_not_inverted() {
        assert!((heatmap_anchor_fraction(0, 10, false) - 0.05).abs() < f64::EPSILON);
        assert!((heatmap_anchor_fraction(9, 10, false) - 0.95).abs() < f64::EPSILON);
    }

    #[test]
    fn heatmap_anchor_fraction_flips_for_inverted_axes() {
        assert!((heatmap_anchor_fraction(0, 10, true) - 0.95).abs() < f64::EPSILON);
        assert!((heatmap_anchor_fraction(9, 10, true) - 0.05).abs() < f64::EPSILON);
    }

    #[test]
    fn help_tab_navigation_wraps() {
        assert_eq!(HelpTab::Keymap.step(-1), HelpTab::Configuration);
        assert_eq!(HelpTab::Configuration.step(1), HelpTab::Keymap);
    }

    #[test]
    fn help_sidebar_navigation_wraps() {
        assert_eq!(
            HelpKeymapSection::Global.step(-1),
            HelpKeymapSection::MultiChart
        );
        assert_eq!(
            HelpCommandSection::Input.step(1),
            HelpCommandSection::Navigation
        );
        assert_eq!(
            HelpCustomizationSection::Configuration.step(-1),
            HelpCustomizationSection::Scripting
        );
    }

    #[test]
    fn help_view_defaults_to_keymap_navigation() {
        let help = HelpViewState::default();
        assert_eq!(help.selected_tab, HelpTab::Keymap);
        assert_eq!(help.keymap_section, HelpKeymapSection::Global);
        assert_eq!(help.command_section, HelpCommandSection::Navigation);
        assert_eq!(
            help.customization_section,
            HelpCustomizationSection::Configuration
        );
    }

    #[test]
    fn preferred_seek_axis_switches_to_cols_when_rows_are_fully_covered() {
        assert_eq!(preferred_seek_axis(false, true), SeekAxis::Col);
        assert_eq!(preferred_seek_axis(true, true), SeekAxis::Row);
        assert_eq!(preferred_seek_axis(false, false), SeekAxis::Row);
    }

    #[test]
    fn heatmap_page_for_target_picks_page_covering_target() {
        let window = HeatmapPageWindow {
            ds_path: "test.h5:/data".to_string(),
            axis: HeatmapPageAxis::Cols,
            len: 100,
            total: 1_000,
            page: 0,
            page_count: 19,
        };
        assert_eq!(window.page_for_target(0), 0);
        assert_eq!(window.page_for_target(75), 0);
        assert_eq!(window.page_for_target(125), 1);
        assert_eq!(window.page_for_target(950), 18);
    }
}
