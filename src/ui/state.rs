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
    preview::chart::MAX_SEGMENT_SIZE,
    tree_view::TreeItem,
};

mod core;
mod heatmap;
mod help_state;
mod preview;
mod ui_layout;
#[allow(unused_imports)]
pub use core::{
    display_path, AppToast, AttributeCreateDialogState, AttributeCreateField, AttributeCursor,
    AttributeDeleteDialogState, AttributeEditRequest, AttributeViewSelection, ContentShowMode,
    FileWatchState, FixedStringOverflowChoice, FixedStringOverflowDialogState, Focus, LastFocused,
    MatrixViewState, Mode, PendingChord,
};
#[allow(unused_imports)]
use heatmap::heatmap_anchor_fraction;
#[allow(unused_imports)]
pub use heatmap::{
    HeatmapCachedPage, HeatmapColormap, HeatmapCustomRangeMode, HeatmapDragState,
    HeatmapLegendSummary, HeatmapLineProfile, HeatmapLineSelection, HeatmapLoadPriority,
    HeatmapLoadRequest, HeatmapLoadedPage, HeatmapNormalization, HeatmapPageWindow,
    HeatmapProfileSample, HeatmapRangeBound, HeatmapRangeMode, HeatmapRegionSelection,
    HeatmapRenderKey, HeatmapRenderState, HeatmapSegmentAxis, HeatmapSelectedCells,
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
    ImgState, PreviewExpressionKey, PreviewExpressionRequest, PreviewExpressionResult,
    PreviewExpressionState, RawImageLoadRequest, SegmentState, SegmentType, VarLenImageLoadRequest,
    CHART_PREVIEW_CACHE_CAPACITY,
};
pub use ui_layout::{
    AttributesHitbox, ContentTabHitbox, HeatmapSettingHitbox, HelpSidebarHitbox, HelpSidebarTarget,
    HelpTabHitbox, MatrixCellHitbox, MatrixRowHitbox, MetadataCellHitbox, TreeHitbox,
    UiLayoutState,
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
    pub configuration_warning: Option<String>,
    pub file_watch: FileWatchState,
    pub compatibility_mode: bool,
    pub focus: Focus,
    pub multi_chart: MultiChartState,
    pub mode: Mode,
    pub command_return_mode: Mode,
    pub help_return_mode: Mode,
    pub searcher: Option<Searcher>,
    pub help: HelpViewState,
    pub pending_chord: Option<PendingChord>,
    pub binding_command_depth: usize,
    pub show_tree_view: bool,
    pub stacked_tree_layout: bool,
    pub image_protocol_enabled: bool,
    pub image_cell_size: (u16, u16),
    pub preview_debounce_generation: u64,
    pub preview_debounce_until: Option<Instant>,
    pub preview_debounce_path: Option<String>,
    pub content_mode: ContentShowMode,
    pub img_state: ImgState,
    pub matrix_view_state: MatrixViewState,
    pub heatmap_viewport_region: Option<HeatmapRegionSelection>,
    pub heatmap_region: Option<HeatmapRegionSelection>,
    pub heatmap_render: HeatmapRenderState,
    pub chart_preview_state: ChartPreviwState,
    pub preview_expression_state: PreviewExpressionState,
    pub segment_state: SegmentState,
    pub command_state: CommandState,
    pub attribute_create_dialog: Option<AttributeCreateDialogState>,
    pub attribute_delete_dialog: Option<AttributeDeleteDialogState>,
    pub fixed_string_overflow_dialog: Option<FixedStringOverflowDialogState>,
    pub ui_layout: UiLayoutState,
}

pub(crate) fn preview_selection_for_node(
    node: &mut H5FNode,
    shape: &[usize],
    segment_idx: i32,
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

    let slice = if shape[node.selected_x] > MAX_SEGMENT_SIZE {
        let start = MAX_SEGMENT_SIZE * segment_idx.max(0) as usize;
        let end = (start + MAX_SEGMENT_SIZE).min(shape[node.selected_x]);
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

    fn normalized_node_path(path: &str) -> &str {
        path.trim_start_matches('/')
    }

    fn current_file_modified(&self) -> Option<SystemTime> {
        fs::metadata(&self.file_watch.path)
            .ok()
            .and_then(|metadata| metadata.modified().ok())
    }

    pub fn clipboard_unavailable_message(&self) -> String {
        match &self.clipboard_init_error {
            Some(error) => format!("Clipboard is unavailable on this system: {error}"),
            None => "Clipboard is unavailable on this system".to_string(),
        }
    }

    pub fn set_clipboard_text(&mut self, text: String) -> std::result::Result<(), String> {
        let Some(clipboard) = self.clipboard.as_mut() else {
            return Err(self.clipboard_unavailable_message());
        };
        clipboard.set_text(text).map_err(|error| error.to_string())
    }

    pub fn sync_file_watch(&mut self) {
        self.file_watch.last_known_modified = self.current_file_modified();
        self.file_watch.pending_external_change = false;
    }

    pub fn acknowledge_file_write(&mut self) {
        self.sync_file_watch();
    }

    pub fn register_file_watch_change(&mut self) -> Option<AppToast> {
        if self.file_watch.pending_external_change {
            return None;
        }

        let current_modified = self.current_file_modified();
        if current_modified == self.file_watch.last_known_modified {
            return None;
        }

        self.file_watch.pending_external_change = true;
        Some(match current_modified {
            Some(_) => AppToast::Info("File changed on disk - press Ctrl-R to reload".to_string()),
            None => AppToast::Warning(
                "File changed or is unavailable on disk - press Ctrl-R to retry reload".to_string(),
            ),
        })
    }

    pub fn selected_tree_path(&self) -> Option<String> {
        self.treeview
            .get(self.tree_view_cursor)
            .map(|item| item.node.borrow().node.path())
    }

    pub fn select_tree_node_by_path(&mut self, path: &str) -> Result<()> {
        let normalized = Self::normalized_node_path(path);
        if normalized.is_empty() {
            self.tree_view_cursor = 0;
            return Ok(());
        }

        let previous_cursor = self.tree_view_cursor;
        let mut current = self.root.clone();
        for segment in normalized.split('/') {
            let next_and_index = {
                let mut node = current.borrow_mut();
                node.ensure_expanded()?;
                node.children.iter().enumerate().find_map(|(index, child)| {
                    let name = child.borrow().name();
                    (name == segment).then(|| (index, child.clone()))
                })
            };
            let Some((index, next)) = next_and_index else {
                self.compute_tree_view();
                self.tree_view_cursor = self.treeview.len().saturating_sub(1).min(previous_cursor);
                return Err(AppError::ChildNotFound(path.to_string()));
            };
            current.borrow_mut().view_loaded = (index + 50) as u32;
            current = next;
        }
        self.compute_tree_view();
        let Some((index, _)) = self
            .treeview
            .iter()
            .enumerate()
            .find(|(_, item)| Rc::ptr_eq(&item.node, &current))
        else {
            self.tree_view_cursor = self.treeview.len().saturating_sub(1).min(previous_cursor);
            return Err(AppError::ChildNotFound(path.to_string()));
        };
        self.tree_view_cursor = index;
        Ok(())
    }

    pub fn select_attribute_by_name(&mut self, attr_name: &str) -> Result<()> {
        let tree_item = self
            .treeview
            .get(self.tree_view_cursor)
            .ok_or_else(|| AppError::EditError("No selected tree item".to_string()))?;
        let mut node = tree_item.node.borrow_mut();
        let attributes = node.read_attributes()?;
        let Some(index) = attributes
            .rendered_rows
            .iter()
            .position(|row| row.key.as_deref() == Some(attr_name))
        else {
            return Err(AppError::ChildNotFound(attr_name.to_string()));
        };
        node.attributes_view_cursor.attribute_index = index;
        node.attributes_view_cursor.attribute_view_selection = AttributeViewSelection::Value;
        Ok(())
    }

    pub fn navigate_to_attribute_target(
        &mut self,
        path: &str,
        attr_name: Option<&str>,
    ) -> Result<()> {
        self.select_tree_node_by_path(path)?;
        if let Some(attr_name) = attr_name {
            self.focus = Focus::Attributes;
            self.select_attribute_by_name(attr_name)?;
        } else {
            self.focus = Focus::Tree(LastFocused::Attributes);
        }
        Ok(())
    }

    pub fn begin_preview_debounce(&mut self, path: String) -> u64 {
        self.preview_debounce_generation = self.preview_debounce_generation.wrapping_add(1);
        self.preview_debounce_until = Some(Instant::now() + Self::PREVIEW_DEBOUNCE_DELAY);
        self.preview_debounce_path = Some(path);
        self.preview_debounce_generation
    }

    pub fn clear_preview_debounce(&mut self) {
        self.preview_debounce_until = None;
        self.preview_debounce_path = None;
    }

    pub fn resolve_preview_debounce(&mut self, generation: u64) -> bool {
        if self.preview_debounce_generation != generation {
            return false;
        }
        let Some(until) = self.preview_debounce_until else {
            return false;
        };
        if Instant::now() < until {
            return false;
        }
        self.clear_preview_debounce();
        true
    }

    pub fn should_debounce_preview(&self, node: &Node) -> bool {
        if !matches!(self.mode, Mode::Normal) || !matches!(self.focus, Focus::Tree(_)) {
            return false;
        }
        let Some(until) = self.preview_debounce_until else {
            return false;
        };
        if Instant::now() >= until {
            return false;
        }
        self.preview_debounce_path.as_deref() == Some(node.path().as_str())
    }

    pub fn active_image_window_mut(&mut self) -> Option<&mut ImageWindowState> {
        let selected_path = self.selected_tree_path()?;
        let window = self.img_state.window.as_mut()?;
        (window.ds_path == selected_path).then_some(window)
    }

    fn remember_main_focus(&mut self, last_focused: LastFocused) {
        self.focus = Focus::Tree(last_focused);
    }

    pub fn focus_tree_from_current(&mut self) {
        let last_focused = match &self.focus {
            Focus::Tree(last_focused) => last_focused.clone(),
            Focus::Attributes => LastFocused::Attributes,
            Focus::Content => LastFocused::Content,
        };
        self.focus = Focus::Tree(last_focused);
    }

    pub fn help_next_tab(&mut self) -> bool {
        let next = self.help.selected_tab.step(1);
        if next == self.help.selected_tab {
            return false;
        }
        self.help.selected_tab = next;
        true
    }

    pub fn help_prev_tab(&mut self) -> bool {
        let next = self.help.selected_tab.step(-1);
        if next == self.help.selected_tab {
            return false;
        }
        self.help.selected_tab = next;
        true
    }

    pub fn help_next_section(&mut self) -> bool {
        match self.help.selected_tab {
            HelpTab::Keymap => {
                let next = self.help.keymap_section.step(1);
                if next == self.help.keymap_section {
                    return false;
                }
                self.help.keymap_section = next;
                true
            }
            HelpTab::Commands => {
                let next = self.help.command_section.step(1);
                if next == self.help.command_section {
                    return false;
                }
                self.help.command_section = next;
                true
            }
            HelpTab::Configuration => {
                let next = self.help.customization_section.step(1);
                if next == self.help.customization_section {
                    return false;
                }
                self.help.customization_section = next;
                true
            }
            HelpTab::MultiChart => {
                let next = self.help.multichart_section.step(1);
                if next == self.help.multichart_section {
                    return false;
                }
                self.help.multichart_section = next;
                true
            }
            _ => false,
        }
    }

    pub fn help_prev_section(&mut self) -> bool {
        match self.help.selected_tab {
            HelpTab::Keymap => {
                let next = self.help.keymap_section.step(-1);
                if next == self.help.keymap_section {
                    return false;
                }
                self.help.keymap_section = next;
                true
            }
            HelpTab::Commands => {
                let next = self.help.command_section.step(-1);
                if next == self.help.command_section {
                    return false;
                }
                self.help.command_section = next;
                true
            }
            HelpTab::Configuration => {
                let next = self.help.customization_section.step(-1);
                if next == self.help.customization_section {
                    return false;
                }
                self.help.customization_section = next;
                true
            }
            HelpTab::MultiChart => {
                let next = self.help.multichart_section.step(-1);
                if next == self.help.multichart_section {
                    return false;
                }
                self.help.multichart_section = next;
                true
            }
            _ => false,
        }
    }

    pub fn help_first_section(&mut self) -> bool {
        match self.help.selected_tab {
            HelpTab::Keymap => {
                if self.help.keymap_section == HelpKeymapSection::Global {
                    false
                } else {
                    self.help.keymap_section = HelpKeymapSection::Global;
                    true
                }
            }
            HelpTab::Commands => {
                if self.help.command_section == HelpCommandSection::Navigation {
                    false
                } else {
                    self.help.command_section = HelpCommandSection::Navigation;
                    true
                }
            }
            HelpTab::Configuration => {
                if self.help.customization_section == HelpCustomizationSection::Configuration {
                    false
                } else {
                    self.help.customization_section = HelpCustomizationSection::Configuration;
                    true
                }
            }
            HelpTab::MultiChart => {
                if self.help.multichart_section == HelpMultiChartSection::Overview {
                    false
                } else {
                    self.help.multichart_section = HelpMultiChartSection::Overview;
                    true
                }
            }
            _ => false,
        }
    }

    pub fn help_last_section(&mut self) -> bool {
        match self.help.selected_tab {
            HelpTab::Keymap => {
                if self.help.keymap_section == HelpKeymapSection::MultiChart {
                    false
                } else {
                    self.help.keymap_section = HelpKeymapSection::MultiChart;
                    true
                }
            }
            HelpTab::Commands => {
                if self.help.command_section == HelpCommandSection::Input {
                    false
                } else {
                    self.help.command_section = HelpCommandSection::Input;
                    true
                }
            }
            HelpTab::Configuration => {
                if self.help.customization_section == HelpCustomizationSection::Scripting {
                    false
                } else {
                    self.help.customization_section = HelpCustomizationSection::Scripting;
                    true
                }
            }
            HelpTab::MultiChart => {
                if self.help.multichart_section == HelpMultiChartSection::Views {
                    false
                } else {
                    self.help.multichart_section = HelpMultiChartSection::Views;
                    true
                }
            }
            _ => false,
        }
    }

    pub fn focus_left(&mut self) {
        if !self.show_tree_view {
            return;
        }
        if self.stacked_tree_layout {
            match self.focus {
                Focus::Attributes => self.remember_main_focus(LastFocused::Attributes),
                Focus::Content => self.remember_main_focus(LastFocused::Content),
                Focus::Tree(_) => {}
            }
            return;
        }
        match self.focus {
            Focus::Attributes => self.remember_main_focus(LastFocused::Attributes),
            Focus::Content => self.remember_main_focus(LastFocused::Content),
            Focus::Tree(_) => {}
        }
    }

    pub fn focus_right(&mut self) {
        if !self.show_tree_view {
            return;
        }
        if self.stacked_tree_layout {
            match self.focus {
                Focus::Tree(LastFocused::Attributes) => self.focus = Focus::Attributes,
                Focus::Tree(LastFocused::Content) => self.focus = Focus::Content,
                Focus::Attributes | Focus::Content => {}
            }
            return;
        }
        match self.focus {
            Focus::Tree(LastFocused::Attributes) => self.focus = Focus::Attributes,
            Focus::Tree(LastFocused::Content) => self.focus = Focus::Content,
            Focus::Attributes | Focus::Content => {}
        }
    }

    pub fn focus_up(&mut self) {
        if !self.show_tree_view {
            return;
        }
        if self.stacked_tree_layout {
            match self.focus {
                Focus::Content => self.focus = Focus::Attributes,
                Focus::Attributes => self.remember_main_focus(LastFocused::Attributes),
                Focus::Tree(_) => {}
            }
            return;
        }
        match self.focus {
            Focus::Content => self.focus = Focus::Attributes,
            Focus::Tree(_) => self.focus = Focus::Attributes,
            Focus::Attributes => {}
        }
    }

    pub fn focus_down(&mut self) {
        if !self.show_tree_view {
            return;
        }
        if self.stacked_tree_layout {
            match self.focus {
                Focus::Tree(_) => self.focus = Focus::Attributes,
                Focus::Attributes => self.focus = Focus::Content,
                Focus::Content => {}
            }
            return;
        }
        match self.focus {
            Focus::Attributes => self.focus = Focus::Content,
            Focus::Tree(_) => self.focus = Focus::Content,
            Focus::Content => {}
        }
    }

    pub fn toggle_tree_view(&mut self) {
        self.show_tree_view = !self.show_tree_view;
        self.pending_chord = None;
        if self.show_tree_view {
            self.focus = Focus::Tree(LastFocused::Content);
        } else {
            self.focus = Focus::Content;
        }
    }

    pub fn swap_content_show_mode(&mut self, available: Vec<ContentShowMode>) {
        let available = self.filter_runtime_content_modes(available);
        let ordered = crate::configure::ordered_content_modes(&available);
        if ordered.is_empty() {
            return;
        }
        let current_index = ordered
            .iter()
            .position(|mode| *mode == self.content_mode)
            .unwrap_or(0);
        self.set_content_mode(ordered[(current_index + 1) % ordered.len()]);
    }

    pub fn set_content_mode(&mut self, mode: ContentShowMode) {
        if self.content_mode == ContentShowMode::Heatmap && mode != ContentShowMode::Heatmap {
            self.end_heatmap_drag();
        }
        self.content_mode = mode;
    }

    pub fn content_show_mode_eval(&self, available: Vec<ContentShowMode>) -> ContentShowMode {
        let available = self.filter_runtime_content_modes(available);
        if available.contains(&self.content_mode) {
            self.content_mode
        } else {
            crate::configure::ordered_content_modes(&available)
                .first()
                .copied()
                .unwrap_or(ContentShowMode::Preview)
        }
    }

    pub fn active_content_mode(&self) -> ContentShowMode {
        let available = self
            .treeview
            .get(self.tree_view_cursor)
            .and_then(|item| {
                item.node
                    .try_borrow()
                    .ok()
                    .map(|node| node.content_show_modes())
            })
            .unwrap_or_else(|| vec![self.content_mode]);
        self.content_show_mode_eval(available)
    }

    pub fn filter_runtime_content_modes(
        &self,
        available: Vec<ContentShowMode>,
    ) -> Vec<ContentShowMode> {
        if !self.compatibility_mode && self.image_protocol_enabled {
            available
        } else {
            available
                .into_iter()
                .filter(|mode| *mode != ContentShowMode::Heatmap)
                .collect()
        }
    }

    pub fn change_row(&mut self, delta: isize) -> Result<EventResult> {
        let active_mode = self.active_content_mode();
        match active_mode {
            ContentShowMode::Matrix | ContentShowMode::Heatmap => {
                let current_node = &self.treeview[self.tree_view_cursor];
                let mut current_node = current_node.node.borrow_mut();
                if let Node::Dataset(_, dsattr) = &current_node.node {
                    if matches!(active_mode, ContentShowMode::Matrix)
                        && dsattr.is_compound_container()
                        && dsattr.supports_compound_root_matrix()
                    {
                        let selectable_dims = dsattr
                            .shape
                            .iter()
                            .enumerate()
                            .filter(|(_, len)| **len > 1)
                            .map(|(dim, _)| dim)
                            .collect::<Vec<_>>();
                        if selectable_dims.is_empty() {
                            return Ok(EventResult::Redraw);
                        }
                        let current_index = selectable_dims
                            .iter()
                            .position(|dim| *dim == current_node.selected_row)
                            .unwrap_or(0);
                        let next_index = (current_index as isize + delta.signum())
                            .rem_euclid(selectable_dims.len() as isize)
                            as usize;
                        current_node.selected_row = selectable_dims[next_index];
                        if current_node.selected_dim == current_node.selected_row {
                            current_node.selected_dim = selectable_dims
                                .iter()
                                .copied()
                                .find(|dim| *dim != current_node.selected_row)
                                .unwrap_or(0);
                        }
                        return Ok(EventResult::Redraw);
                    }
                    let shape = dsattr.shape.clone();
                    if shape.len() == 2 {
                        let temp = current_node.selected_row;
                        current_node.selected_row = current_node.selected_col;
                        current_node.selected_col = temp;
                        return Ok(EventResult::Redraw);
                    }
                    let new_selected_row = ((current_node.selected_row as isize + delta)
                        % shape.len() as isize) as usize
                        % shape.len();
                    if new_selected_row != current_node.selected_col {
                        current_node.selected_row = new_selected_row;
                        return Ok(EventResult::Redraw);
                    }
                    current_node.selected_row = ((current_node.selected_row as isize + delta + 1)
                        % shape.len() as isize)
                        as usize
                        % shape.len();

                    Ok(EventResult::Redraw)
                } else {
                    Ok(EventResult::Continue)
                }
            }
            _ => Ok(EventResult::Continue),
        }
    }

    pub fn capture_multichart_item(&self) -> Result<Option<CapturedMultiChartItem>> {
        let current_node = &self.treeview[self.tree_view_cursor];
        let mut node = current_node.node.borrow_mut();
        match &node.node {
            Node::Group(_, meta) => {
                let Some(expression) = meta.preview_expr.as_deref() else {
                    return Ok(None);
                };
                let item = self
                    .multi_chart
                    .capture_expression_chart_item(expression, self.file.as_ref())
                    .map_err(AppError::InvalidCommand)?;
                let (source, points) = item;
                Ok(Some(CapturedMultiChartItem {
                    source,
                    source_len: points.len(),
                    initial_points: Some(points),
                    load_state: MultiChartLoadState::Ready,
                    request: None,
                }))
            }
            Node::Dataset(_, dsattr) if dsattr.is_compound_container() => Ok(None),
            Node::Dataset(ds, dsattr) => {
                let ds = ds.clone();
                let meta = dsattr.clone();
                let shape = dsattr.shape.clone();
                let Some(selection) =
                    preview_selection_for_node(&mut node, &shape, self.segment_state.idx)
                else {
                    return Ok(None);
                };
                let source = ChartSource::DatasetSelection(DatasetChartSource {
                    dataset_path: ds.name(),
                    display_path: meta.virtual_path().unwrap_or(&ds.name()).to_string(),
                    selection: selection.clone(),
                    shape,
                    kind: if meta.is_compound_leaf() {
                        DatasetChartKind::CompoundLeaf
                    } else {
                        DatasetChartKind::Dataset
                    },
                });
                Ok(Some(CapturedMultiChartItem {
                    source,
                    source_len: 0,
                    initial_points: None,
                    load_state: MultiChartLoadState::Queued,
                    request: Some(MultiChartLoadRequest {
                        item_id: crate::ui::mchart::ChartItemId(0),
                        kind: crate::ui::mchart::MultiChartLoadKind::Overview { generation: 0 },
                        source: if meta.is_compound_leaf() {
                            MultiChartLoadSource::CompoundLeaf {
                                dataset: ds,
                                meta: Box::new(meta),
                                selection,
                            }
                        } else {
                            MultiChartLoadSource::Dataset {
                                dataset: ds,
                                selection,
                            }
                        },
                    }),
                }))
            }
            _ => Ok(None),
        }
    }

    pub fn change_selected_dimension(&mut self, delta: isize) -> Result<EventResult> {
        let active_mode = self.active_content_mode();
        let current_node = &self.treeview[self.tree_view_cursor];
        let mut node = current_node.node.borrow_mut();
        let shape_len = match &node.node {
            Node::Dataset(_, dsattr) => dsattr.shape.len(),
            _ => return Ok(EventResult::Continue),
        };
        node.sync_selection_rank(shape_len);
        let current_shape_len = shape_len as isize;
        let next = node.selected_dim as isize + delta;
        let new_selected_dim = if next < 0 {
            (current_shape_len - 1) as usize
        } else if next >= current_shape_len {
            0_usize
        } else {
            next as usize
        };
        match active_mode {
            ContentShowMode::Preview => {
                if new_selected_dim != node.selected_x {
                    node.selected_dim = new_selected_dim;
                } else {
                    let next_next = new_selected_dim as isize + delta;
                    let next_next = if next_next < 0 {
                        (current_shape_len - 1) as usize
                    } else if next_next >= current_shape_len {
                        0_usize
                    } else {
                        next_next as usize
                    };
                    node.selected_dim = next_next.clamp(0, current_shape_len as usize);
                }
                Ok(EventResult::Redraw)
            }
            ContentShowMode::Matrix | ContentShowMode::Heatmap => {
                let is_compound_root_matrix = matches!(
                    &node.node,
                    Node::Dataset(_, dsattr)
                        if matches!(active_mode, ContentShowMode::Matrix)
                            && dsattr.is_compound_container()
                            && dsattr.supports_compound_root_matrix()
                );
                if new_selected_dim != node.selected_col && new_selected_dim != node.selected_row {
                    if !is_compound_root_matrix || new_selected_dim != node.selected_row {
                        node.selected_dim = new_selected_dim;
                    }
                } else {
                    let next_next = new_selected_dim as isize + delta;
                    let next_next = if next_next < 0 {
                        (current_shape_len - 1) as usize
                    } else if next_next >= current_shape_len {
                        0_usize
                    } else {
                        next_next as usize
                    };
                    if (is_compound_root_matrix && next_next != node.selected_row)
                        || (!is_compound_root_matrix
                            && next_next != node.selected_col
                            && next_next != node.selected_row)
                    {
                        node.selected_dim = next_next.clamp(0, current_shape_len as usize);
                    } else {
                        let next_next_next = next_next as isize + delta;
                        let next_next_next = if next_next_next < 0 {
                            (current_shape_len - 1) as usize
                        } else if next_next_next >= current_shape_len {
                            0_usize
                        } else {
                            next_next_next as usize
                        };
                        node.selected_dim =
                            if is_compound_root_matrix && next_next_next == node.selected_row {
                                node.selected_dim
                            } else {
                                next_next_next.clamp(0, current_shape_len as usize)
                            };
                    }
                }
                Ok(EventResult::Redraw)
            }
        }
    }

    pub fn change_selected_index(&mut self, delta: isize) -> Result<EventResult> {
        let current_node = &self.treeview[self.tree_view_cursor];
        let mut node = current_node.node.borrow_mut();
        let shape = match &node.node {
            Node::Dataset(_, dsattr) => dsattr.shape.clone(),
            _ => return Ok(EventResult::Continue),
        };
        node.sync_selection_rank(shape.len());
        let x_shape = shape[node.selected_dim];
        let current_selected_dim = node.selected_indexes[node.selected_dim] as isize;
        let new_current_x_index =
            (current_selected_dim + delta).clamp(0, x_shape as isize - 1) as usize;
        let selected_x = node.selected_dim;
        node.selected_indexes[selected_x] = new_current_x_index;

        Ok(EventResult::Redraw)
    }

    pub fn change_col(&mut self, delta: isize) -> Result<EventResult> {
        let active_mode = self.active_content_mode();
        match active_mode {
            ContentShowMode::Matrix | ContentShowMode::Heatmap => {
                let current_node = &self.treeview[self.tree_view_cursor];
                let mut current_node = current_node.node.borrow_mut();
                if let Node::Dataset(_, dsattr) = &current_node.node {
                    if matches!(active_mode, ContentShowMode::Matrix)
                        && dsattr.is_compound_container()
                        && dsattr.supports_compound_root_matrix()
                    {
                        return Ok(EventResult::Redraw);
                    }
                    let shape = dsattr.shape.clone();
                    if shape.len() == 2 {
                        // Just swap row and col.
                        let temp = current_node.selected_row;
                        current_node.selected_row = current_node.selected_col;
                        current_node.selected_col = temp;
                        return Ok(EventResult::Redraw);
                    }
                    let new_selected_col = ((current_node.selected_col as isize + delta)
                        % shape.len() as isize) as usize
                        % shape.len();
                    if new_selected_col != current_node.selected_row {
                        current_node.selected_col = new_selected_col;
                        return Ok(EventResult::Redraw);
                    }
                    current_node.selected_col = ((current_node.selected_col as isize + delta + 1)
                        % shape.len() as isize)
                        as usize
                        % shape.len();

                    Ok(EventResult::Redraw)
                } else {
                    Ok(EventResult::Continue)
                }
            }
            _ => Ok(EventResult::Continue),
        }
    }

    pub fn change_x(&mut self, delta: isize) -> Result<EventResult> {
        match self.active_content_mode() {
            ContentShowMode::Preview => {
                let current_node = &self.treeview[self.tree_view_cursor];
                let mut current_node = current_node.node.borrow_mut();
                if let Node::Dataset(_, dsattr) = &current_node.node {
                    let shape = dsattr.shape.clone();
                    current_node.selected_x = ((current_node.selected_x as isize + delta)
                        % shape.len() as isize)
                        as usize
                        % shape.len();
                    Ok(EventResult::Redraw)
                } else {
                    Ok(EventResult::Continue)
                }
            }
            _ => Ok(EventResult::Continue),
        }
    }

    pub fn up(&mut self, dec: usize) -> Result<EventResult> {
        match self.active_content_mode() {
            ContentShowMode::Preview => match self.segment_state.segumented {
                SegmentType::Image => {
                    if self.img_state.idx_to_load >= (dec as i32)
                        && self.img_state.idx_to_load - dec as i32 >= 0
                    {
                        self.img_state.idx_to_load -= dec as i32;
                        Ok(EventResult::Redraw)
                    } else {
                        Ok(EventResult::Continue)
                    }
                }
                SegmentType::Chart => {
                    let Some(max_index) = self.segment_state.max_index() else {
                        self.segment_state.idx = 0;
                        return Ok(EventResult::Continue);
                    };
                    self.segment_state.idx = self
                        .segment_state
                        .idx
                        .saturating_sub(dec as i32)
                        .clamp(0, max_index);
                    Ok(EventResult::Redraw)
                }
                SegmentType::NoSegment => {
                    if let Some(window) = self.active_image_window_mut() {
                        let step = ((window.len / 4).max(1) * dec.max(1)) as isize;
                        window.shift_by(-step);
                        return Ok(EventResult::Redraw);
                    }
                    self.img_state.idx_to_load = self.segment_state.idx;
                    let current_node = &self.treeview[self.tree_view_cursor];
                    let mut node = current_node.node.borrow_mut();
                    let new_offset = node.line_offset as isize - dec as isize;
                    let new_offset = if new_offset < 0 {
                        0
                    } else {
                        new_offset as usize
                    };
                    node.line_offset = new_offset;

                    Ok(EventResult::Redraw)
                }
            },
            ContentShowMode::Matrix => {
                let current_node = &self.treeview[self.tree_view_cursor];
                let node = &current_node.node.borrow_mut();
                let current_node = &node.node;
                if self.matrix_view_state.row_offset == 0 {
                    return Ok(EventResult::Redraw);
                }
                if let Node::Dataset(_, dsattr) = current_node {
                    let row_selected_shape = dsattr.shape[node.selected_row];
                    self.matrix_view_state.row_offset =
                        (self.matrix_view_state.row_offset.saturating_sub(dec)).min(
                            row_selected_shape
                                .saturating_sub(self.matrix_view_state.rows_currently_available),
                        );
                    Ok(EventResult::Redraw)
                } else {
                    Ok(EventResult::Redraw)
                }
            }
            ContentShowMode::Heatmap => {
                if dec > 1 {
                    if let Some(window) = self.heatmap_render.segment.as_mut() {
                        let next_page = window
                            .page
                            .saturating_sub(1)
                            .clamp(0, window.page_count.saturating_sub(1));
                        if next_page != window.page {
                            window.page = next_page;
                            self.heatmap_render.current_key = None;
                            return Ok(EventResult::Redraw);
                        }
                    }
                }
                self.heatmap_render.selected_setting = self
                    .heatmap_render
                    .selected_setting
                    .saturating_sub(1)
                    .min(HEATMAP_SETTING_FIELDS.len().saturating_sub(1));
                Ok(EventResult::Redraw)
            }
        }
    }

    pub fn down(&mut self, inc: usize) -> Result<EventResult> {
        match self.active_content_mode() {
            ContentShowMode::Preview => match self.segment_state.segumented {
                SegmentType::Image => {
                    let Some(max_index) = self.segment_state.max_index() else {
                        self.img_state.idx_to_load = 0;
                        return Ok(EventResult::Continue);
                    };
                    let proposed = self.img_state.idx_to_load.saturating_add(inc as i32);
                    if proposed <= max_index {
                        self.img_state.idx_to_load = proposed;
                        Ok(EventResult::Redraw)
                    } else {
                        Ok(EventResult::Continue)
                    }
                }
                SegmentType::Chart => {
                    let Some(max_index) = self.segment_state.max_index() else {
                        self.segment_state.idx = 0;
                        return Ok(EventResult::Continue);
                    };
                    self.segment_state.idx = self
                        .segment_state
                        .idx
                        .saturating_add(inc as i32)
                        .clamp(0, max_index);
                    Ok(EventResult::Redraw)
                }
                SegmentType::NoSegment => {
                    if let Some(window) = self.active_image_window_mut() {
                        let step = ((window.len / 4).max(1) * inc.max(1)) as isize;
                        window.shift_by(step);
                        return Ok(EventResult::Redraw);
                    }
                    self.img_state.idx_to_load = self.segment_state.idx;

                    self.img_state.idx_to_load = self.segment_state.idx;
                    let current_node = &self.treeview[self.tree_view_cursor];
                    let mut node = current_node.node.borrow_mut();
                    let new_offset = node.line_offset + inc;
                    node.line_offset = new_offset;
                    Ok(EventResult::Redraw)
                }
            },
            ContentShowMode::Matrix => {
                let node = &self.treeview[self.tree_view_cursor].node.borrow_mut();
                let current_node = &node.node;
                if let Node::Dataset(_, dsattr) = current_node {
                    let row_selected_shape = dsattr.shape[node.selected_row];
                    self.matrix_view_state.row_offset = (self.matrix_view_state.row_offset + inc)
                        .min(
                            row_selected_shape
                                .saturating_sub(self.matrix_view_state.rows_currently_available),
                        );
                    Ok(EventResult::Redraw)
                } else {
                    Ok(EventResult::Redraw)
                }
            }
            ContentShowMode::Heatmap => {
                if inc > 1 {
                    if let Some(window) = self.heatmap_render.segment.as_mut() {
                        let next_page = window
                            .page
                            .saturating_add(1)
                            .clamp(0, window.page_count.saturating_sub(1));
                        if next_page != window.page {
                            window.page = next_page;
                            self.heatmap_render.current_key = None;
                            return Ok(EventResult::Redraw);
                        }
                    }
                }
                self.heatmap_render.selected_setting = self
                    .heatmap_render
                    .selected_setting
                    .saturating_add(1)
                    .min(HEATMAP_SETTING_FIELDS.len().saturating_sub(1));
                Ok(EventResult::Redraw)
            }
        }
    }

    pub fn set(&mut self, idx: usize) -> Result<EventResult> {
        match self.active_content_mode() {
            ContentShowMode::Preview => match self.segment_state.segumented {
                SegmentType::Image => {
                    if let Some(window) = self.active_image_window_mut() {
                        window.center_on(idx);
                        return Ok(EventResult::Redraw);
                    }
                    if idx < self.segment_state.segment_count.max(0) as usize {
                        self.img_state.idx_to_load = idx as i32;
                        Ok(EventResult::Redraw)
                    } else {
                        Ok(EventResult::Continue)
                    }
                }
                SegmentType::Chart => {
                    let Some(max_index) = self.segment_state.max_index() else {
                        self.segment_state.idx = 0;
                        return Ok(EventResult::Continue);
                    };
                    if idx > 0 {
                        self.segment_state.idx = ((idx - 1) as i32).clamp(0, max_index);
                        Ok(EventResult::Redraw)
                    } else {
                        self.segment_state.idx = 0;
                        Ok(EventResult::Redraw)
                    }
                }
                SegmentType::NoSegment => {
                    if let Some(window) = self.active_image_window_mut() {
                        window.center_on(idx);
                        return Ok(EventResult::Redraw);
                    }
                    self.img_state.idx_to_load = idx as i32;
                    Ok(EventResult::Redraw)
                }
            },
            ContentShowMode::Matrix => {
                let node = &self.treeview[self.tree_view_cursor].node.borrow_mut();
                let current_node = &node.node;
                if let Node::Dataset(_, dsattr) = current_node {
                    let row_selected_shape = dsattr.shape[node.selected_row];
                    self.matrix_view_state.row_offset = idx.min(
                        row_selected_shape
                            .saturating_sub(self.matrix_view_state.rows_currently_available),
                    );
                    Ok(EventResult::Redraw)
                } else {
                    Ok(EventResult::Redraw)
                }
            }
            ContentShowMode::Heatmap => {
                self.heatmap_render.selected_setting =
                    idx.min(HEATMAP_SETTING_FIELDS.len().saturating_sub(1));
                Ok(EventResult::Redraw)
            }
        }
    }

    pub fn reexecute_command(&mut self) -> Result<EventResult> {
        let Some(last_command) = self.command_state.last_command.clone() else {
            return Ok(EventResult::Toast(
                AppToast::Info("No previous command to repeat".to_string()),
                false,
            ));
        };
        execute_command(self, &last_command)
    }

    pub fn heatmap_range_modes(&self) -> Vec<HeatmapRangeMode> {
        let mut modes = HeatmapRangeMode::default_modes();
        for mode in configure::current_heatmap_range_modes()
            .into_iter()
            .chain(self.heatmap_render.session_range_modes.iter().cloned())
        {
            if !modes.contains(&mode) {
                modes.push(mode);
            }
        }
        modes
    }

    pub fn sync_heatmap_configuration(&mut self) {
        let available = self.heatmap_range_modes();
        let mut configured = configure::current_heatmap_default_settings();
        if !available.contains(&configured.range) {
            configured.range = available.first().cloned().unwrap_or(HeatmapRangeMode::Auto);
        }
        self.heatmap_render.settings = configured;
        self.heatmap_render.current_key = None;
    }

    pub fn add_session_heatmap_range_mode(&mut self, mode: HeatmapRangeMode) -> Result<()> {
        let label = mode.label();
        if self
            .heatmap_range_modes()
            .iter()
            .any(|existing| existing.label().eq_ignore_ascii_case(&label))
        {
            return Err(AppError::InvalidCommand(format!(
                "Heatmap range mode '{label}' already exists"
            )));
        }
        self.heatmap_render.session_range_modes.push(mode.clone());
        self.heatmap_render.settings.range = mode;
        self.heatmap_render.current_key = None;
        Ok(())
    }

    fn adjust_heatmap_range_mode(&mut self, delta: isize) {
        let available = self.heatmap_range_modes();
        if available.is_empty() {
            return;
        }
        let current_index = available
            .iter()
            .position(|mode| *mode == self.heatmap_render.settings.range)
            .unwrap_or_else(|| {
                available
                    .iter()
                    .position(|mode| *mode == configure::current_heatmap_default_range())
                    .unwrap_or(0)
            });
        let next_index =
            (current_index as isize + delta.signum()).rem_euclid(available.len() as isize) as usize;
        self.heatmap_render.settings.range = available[next_index].clone();
        self.heatmap_render.current_key = None;
    }

    pub fn right(&mut self, inc: isize) -> Result<EventResult> {
        match self.active_content_mode() {
            ContentShowMode::Preview => match self.segment_state.segumented {
                SegmentType::Image => {
                    if let Some(window) = self.active_image_window_mut() {
                        let step = ((window.len / 4).max(1) as isize) * inc.max(1);
                        window.shift_by(step);
                        Ok(EventResult::Redraw)
                    } else {
                        self.down(1)
                    }
                }
                SegmentType::Chart => Ok(EventResult::Continue),
                SegmentType::NoSegment => {
                    if let Some(window) = self.active_image_window_mut() {
                        let step = ((window.len / 4).max(1) as isize) * inc.max(1);
                        window.shift_by(step);
                        return Ok(EventResult::Redraw);
                    }
                    let current_node = &self.treeview[self.tree_view_cursor];
                    let mut node = current_node.node.borrow_mut();
                    let new_col_offset = node.col_offset.saturating_add(inc).max(0);
                    node.col_offset = new_col_offset;
                    Ok(EventResult::Redraw)
                }
            },
            ContentShowMode::Matrix => {
                let node = &self.treeview[self.tree_view_cursor].node.borrow_mut();
                let current_node = &node.node;
                if let Node::Dataset(_, dsattr) = current_node {
                    let col_selected_shape = if dsattr.is_compound_container()
                        && dsattr.supports_compound_root_matrix()
                    {
                        dsattr
                            .compound_root_matrix_column_count()
                            .unwrap_or_default()
                    } else {
                        dsattr.shape[node.selected_col]
                    };
                    self.matrix_view_state.col_offset =
                        (self.matrix_view_state.col_offset + inc as usize).min(
                            col_selected_shape
                                .saturating_sub(self.matrix_view_state.cols_currently_available),
                        );
                    Ok(EventResult::Redraw)
                } else {
                    Ok(EventResult::Redraw)
                }
            }
            ContentShowMode::Heatmap => {
                let field = HEATMAP_SETTING_FIELDS
                    .get(self.heatmap_render.selected_setting)
                    .copied()
                    .unwrap_or(HeatmapSettingField::Colormap);
                if matches!(field, HeatmapSettingField::Range) {
                    self.adjust_heatmap_range_mode(inc);
                } else {
                    self.heatmap_render.settings.adjust(field, inc);
                    self.heatmap_render.current_key = None;
                }
                Ok(EventResult::Redraw)
            }
        }
    }

    pub fn left(&mut self, inc: isize) -> Result<EventResult> {
        match self.active_content_mode() {
            ContentShowMode::Preview => match self.segment_state.segumented {
                SegmentType::Image => {
                    if let Some(window) = self.active_image_window_mut() {
                        let step = ((window.len / 4).max(1) as isize) * inc.max(1);
                        window.shift_by(-step);
                        Ok(EventResult::Redraw)
                    } else {
                        self.up(1)
                    }
                }
                SegmentType::Chart => Ok(EventResult::Continue),
                SegmentType::NoSegment => {
                    if let Some(window) = self.active_image_window_mut() {
                        let step = ((window.len / 4).max(1) as isize) * inc.max(1);
                        window.shift_by(-step);
                        return Ok(EventResult::Redraw);
                    }
                    let current_node = &self.treeview[self.tree_view_cursor];
                    let mut node = current_node.node.borrow_mut();
                    let new_col_offset = node.col_offset.saturating_sub(inc).max(0);
                    node.col_offset = new_col_offset;
                    Ok(EventResult::Redraw)
                }
            },
            ContentShowMode::Matrix => {
                let current_node = &self.treeview[self.tree_view_cursor];
                let node = &current_node.node.borrow_mut();
                let current_node = &node.node;
                if self.matrix_view_state.col_offset == 0 {
                    return Ok(EventResult::Redraw);
                }
                if let Node::Dataset(_, dsattr) = current_node {
                    let col_selected_shape = if dsattr.is_compound_container()
                        && dsattr.supports_compound_root_matrix()
                    {
                        dsattr
                            .compound_root_matrix_column_count()
                            .unwrap_or_default()
                    } else {
                        dsattr.shape[node.selected_col]
                    };
                    self.matrix_view_state.col_offset = (self
                        .matrix_view_state
                        .col_offset
                        .saturating_sub(inc as usize))
                    .min(
                        col_selected_shape
                            .saturating_sub(self.matrix_view_state.cols_currently_available),
                    );
                    Ok(EventResult::Redraw)
                } else {
                    Ok(EventResult::Redraw)
                }
            }
            ContentShowMode::Heatmap => {
                let field = HEATMAP_SETTING_FIELDS
                    .get(self.heatmap_render.selected_setting)
                    .copied()
                    .unwrap_or(HeatmapSettingField::Colormap);
                if matches!(field, HeatmapSettingField::Range) {
                    self.adjust_heatmap_range_mode(-inc);
                } else {
                    self.heatmap_render.settings.adjust(field, -inc);
                    self.heatmap_render.current_key = None;
                }
                Ok(EventResult::Redraw)
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::expect_used)]
mod tests {
    use super::{
        heatmap_anchor_fraction, HelpCommandSection, HelpCustomizationSection, HelpKeymapSection,
        HelpTab, HelpViewState,
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
}
