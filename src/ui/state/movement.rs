use super::*;

impl AppState<'_> {
    pub fn up(&mut self, dec: usize) -> Result<EventResult> {
        match self.active_content_mode() {
            ContentShowMode::Preview => match self.page_state.paged {
                PageType::Image => {
                    if self.img_state.idx_to_load >= (dec as i32)
                        && self.img_state.idx_to_load - dec as i32 >= 0
                    {
                        self.img_state.idx_to_load -= dec as i32;
                        Ok(EventResult::Redraw)
                    } else {
                        Ok(EventResult::Continue)
                    }
                }
                PageType::Chart => {
                    let Some(max_index) = self.page_state.max_index() else {
                        self.page_state.idx = 0;
                        return Ok(EventResult::Continue);
                    };
                    self.page_state.idx = self
                        .page_state
                        .idx
                        .saturating_sub(dec as i32)
                        .clamp(0, max_index);
                    Ok(EventResult::Redraw)
                }
                PageType::Unpaged => {
                    if let Some(window) = self.active_image_window_mut() {
                        let step = ((window.len / 4).max(1) * dec.max(1)) as isize;
                        window.shift_by(-step);
                        return Ok(EventResult::Redraw);
                    }
                    self.img_state.idx_to_load = self.page_state.idx;
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
                    if let Some(window) = self.heatmap_render.page_window.as_mut() {
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
            ContentShowMode::Preview => match self.page_state.paged {
                PageType::Image => {
                    let Some(max_index) = self.page_state.max_index() else {
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
                PageType::Chart => {
                    let Some(max_index) = self.page_state.max_index() else {
                        self.page_state.idx = 0;
                        return Ok(EventResult::Continue);
                    };
                    self.page_state.idx = self
                        .page_state
                        .idx
                        .saturating_add(inc as i32)
                        .clamp(0, max_index);
                    Ok(EventResult::Redraw)
                }
                PageType::Unpaged => {
                    if let Some(window) = self.active_image_window_mut() {
                        let step = ((window.len / 4).max(1) * inc.max(1)) as isize;
                        window.shift_by(step);
                        return Ok(EventResult::Redraw);
                    }
                    self.img_state.idx_to_load = self.page_state.idx;

                    self.img_state.idx_to_load = self.page_state.idx;
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
                    if let Some(window) = self.heatmap_render.page_window.as_mut() {
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
            ContentShowMode::Preview => match self.page_state.paged {
                PageType::Image => {
                    if let Some(window) = self.active_image_window_mut() {
                        window.center_on(idx);
                        return Ok(EventResult::Redraw);
                    }
                    if idx < self.page_state.page_count.max(0) as usize {
                        self.img_state.idx_to_load = idx as i32;
                        Ok(EventResult::Redraw)
                    } else {
                        Ok(EventResult::Continue)
                    }
                }
                PageType::Chart => {
                    let Some(max_index) = self.page_state.max_index() else {
                        self.page_state.idx = 0;
                        return Ok(EventResult::Continue);
                    };
                    if idx > 0 {
                        self.page_state.idx = ((idx - 1) as i32).clamp(0, max_index);
                        Ok(EventResult::Redraw)
                    } else {
                        self.page_state.idx = 0;
                        Ok(EventResult::Redraw)
                    }
                }
                PageType::Unpaged => {
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

    pub fn seek_absolute(
        &mut self,
        primary: usize,
        secondary: Option<usize>,
    ) -> Result<EventResult> {
        match self.active_content_mode() {
            ContentShowMode::Preview => {
                if secondary.is_some() {
                    return Err(AppError::InvalidCommand(
                        "seek <x> <y> is only available in matrix or heatmap mode".to_string(),
                    ));
                }
                self.set(primary)
            }
            ContentShowMode::Matrix => {
                if let Some(row) = secondary {
                    self.seek_matrix_col(primary)?;
                    self.seek_matrix_row(row)
                } else if matches!(self.smart_2d_seek_axis()?, Some(SeekAxis::Col)) {
                    self.seek_matrix_col(primary)
                } else {
                    self.seek_matrix_row(primary)
                }
            }
            ContentShowMode::Heatmap => {
                if let Some(row) = secondary {
                    self.seek_heatmap_col(primary)?;
                    self.seek_heatmap_row(row)?;
                    self.heatmap_render.selected_cells =
                        Some(HeatmapSelectedCells::single(row, primary));
                    Ok(EventResult::Redraw)
                } else if matches!(self.smart_2d_seek_axis()?, Some(SeekAxis::Col)) {
                    self.seek_heatmap_col(primary)
                } else {
                    self.seek_heatmap_row(primary)
                }
            }
        }
    }

    pub fn seek_row_absolute(&mut self, row: usize) -> Result<EventResult> {
        match self.active_content_mode() {
            ContentShowMode::Matrix => self.seek_matrix_row(row),
            ContentShowMode::Heatmap => self.seek_heatmap_row(row),
            _ => Err(AppError::InvalidCommand(
                "seek-row is only available in matrix or heatmap mode".to_string(),
            )),
        }
    }

    pub fn seek_col_absolute(&mut self, col: usize) -> Result<EventResult> {
        match self.active_content_mode() {
            ContentShowMode::Matrix => self.seek_matrix_col(col),
            ContentShowMode::Heatmap => self.seek_heatmap_col(col),
            _ => Err(AppError::InvalidCommand(
                "seek-col is only available in matrix or heatmap mode".to_string(),
            )),
        }
    }

    pub fn seek_page_absolute(&mut self, page: usize) -> Result<EventResult> {
        let page = page.saturating_sub(1);
        match self.active_content_mode() {
            ContentShowMode::Preview => match self.page_state.paged {
                PageType::Image => {
                    let Some(max_index) = self.page_state.max_index() else {
                        self.img_state.idx_to_load = 0;
                        self.page_state.idx = 0;
                        return Ok(EventResult::Continue);
                    };
                    let target = (page as i32).clamp(0, max_index);
                    self.img_state.idx_to_load = target;
                    self.page_state.idx = target;
                    Ok(EventResult::Redraw)
                }
                PageType::Chart => {
                    let Some(max_index) = self.page_state.max_index() else {
                        self.page_state.idx = 0;
                        return Ok(EventResult::Continue);
                    };
                    self.page_state.idx = (page as i32).clamp(0, max_index);
                    Ok(EventResult::Redraw)
                }
                PageType::Unpaged => Err(AppError::InvalidCommand(
                    "seek-page is only available when the current preview is paged".to_string(),
                )),
            },
            ContentShowMode::Heatmap => {
                let Some(window) = self.heatmap_render.page_window.as_mut() else {
                    return Err(AppError::InvalidCommand(
                        "seek-page is only available when the current heatmap is paged".to_string(),
                    ));
                };
                window.page = (page as i32).clamp(0, window.page_count.saturating_sub(1));
                self.heatmap_render.current_key = None;
                Ok(EventResult::Redraw)
            }
            _ => Err(AppError::InvalidCommand(
                "seek-page is only available in preview or heatmap mode".to_string(),
            )),
        }
    }

    fn smart_2d_seek_axis(&self) -> Result<Option<SeekAxis>> {
        match self.active_content_mode() {
            ContentShowMode::Matrix => {
                let current_node = &self.treeview[self.tree_view_cursor];
                let node = current_node.node.borrow();
                let Node::Dataset(_, dsattr) = &node.node else {
                    return Ok(None);
                };
                let row_total = dsattr.shape[node.selected_row];
                let col_total =
                    if dsattr.is_compound_container() && dsattr.supports_compound_root_matrix() {
                        dsattr
                            .compound_root_matrix_column_count()
                            .unwrap_or_default()
                    } else {
                        dsattr.shape[node.selected_col]
                    };
                let row_seekable =
                    row_total > self.matrix_view_state.rows_currently_available.max(1);
                let col_seekable =
                    col_total > self.matrix_view_state.cols_currently_available.max(1);
                Ok(Some(preferred_seek_axis(row_seekable, col_seekable)))
            }
            ContentShowMode::Heatmap => {
                let current_node = &self.treeview[self.tree_view_cursor];
                let node = current_node.node.borrow();
                let Node::Dataset(_, dsattr) = &node.node else {
                    return Ok(None);
                };
                let source_rows = dsattr.shape[node.selected_row];
                let source_cols = dsattr.shape[node.selected_col];
                let base_viewport = self.heatmap_render.viewport.unwrap_or(HeatmapViewport {
                    row_start: 0,
                    row_len: source_rows.max(1),
                    col_start: 0,
                    col_len: source_cols.max(1),
                });
                let row_seekable = base_viewport.row_len < source_rows
                    || self
                        .heatmap_render
                        .page_window
                        .as_ref()
                        .is_some_and(|window| matches!(window.axis, HeatmapPageAxis::Rows));
                let col_seekable = base_viewport.col_len < source_cols
                    || self
                        .heatmap_render
                        .page_window
                        .as_ref()
                        .is_some_and(|window| matches!(window.axis, HeatmapPageAxis::Cols));
                Ok(Some(preferred_seek_axis(row_seekable, col_seekable)))
            }
            _ => Ok(None),
        }
    }

    fn seek_matrix_row(&mut self, row: usize) -> Result<EventResult> {
        let Some(row_total) = ({
            let current_node = &self.treeview[self.tree_view_cursor];
            let node = current_node.node.borrow();
            match &node.node {
                Node::Dataset(_, dsattr) => Some(dsattr.shape[node.selected_row]),
                _ => None,
            }
        }) else {
            return Ok(EventResult::Continue);
        };
        self.matrix_view_state.row_offset = clamp_absolute_seek_start(
            row,
            row_total,
            self.matrix_view_state.rows_currently_available.max(1),
        );
        Ok(EventResult::Redraw)
    }

    fn seek_matrix_col(&mut self, col: usize) -> Result<EventResult> {
        let Some(col_total) = ({
            let current_node = &self.treeview[self.tree_view_cursor];
            let node = current_node.node.borrow();
            match &node.node {
                Node::Dataset(_, dsattr) => Some(
                    if dsattr.is_compound_container() && dsattr.supports_compound_root_matrix() {
                        dsattr
                            .compound_root_matrix_column_count()
                            .unwrap_or_default()
                    } else {
                        dsattr.shape[node.selected_col]
                    },
                ),
                _ => None,
            }
        }) else {
            return Ok(EventResult::Continue);
        };
        self.matrix_view_state.col_offset = clamp_absolute_seek_start(
            col,
            col_total,
            self.matrix_view_state.cols_currently_available.max(1),
        );
        Ok(EventResult::Redraw)
    }

    fn seek_heatmap_row(&mut self, row: usize) -> Result<EventResult> {
        let Some((source_rows, source_cols)) = ({
            let current_node = &self.treeview[self.tree_view_cursor];
            let node = current_node.node.borrow();
            match &node.node {
                Node::Dataset(_, dsattr) => Some((
                    dsattr.shape[node.selected_row],
                    dsattr.shape[node.selected_col],
                )),
                _ => None,
            }
        }) else {
            return Ok(EventResult::Continue);
        };
        let mut viewport = self.heatmap_render.viewport.unwrap_or(HeatmapViewport {
            row_start: 0,
            row_len: source_rows.max(1),
            col_start: 0,
            col_len: source_cols.max(1),
        });
        if viewport.row_len < source_rows {
            viewport.row_start = clamp_absolute_seek_start(row, source_rows, viewport.row_len);
            self.heatmap_render.viewport = Some(viewport);
        }
        if let Some(window) = self.heatmap_render.page_window.as_mut() {
            if matches!(window.axis, HeatmapPageAxis::Rows) {
                let relative = row
                    .saturating_sub(viewport.row_start)
                    .min(viewport.row_len.saturating_sub(1));
                window.page = window.page_for_target(relative);
            }
        }
        self.heatmap_render.current_key = None;
        Ok(EventResult::Redraw)
    }

    fn seek_heatmap_col(&mut self, col: usize) -> Result<EventResult> {
        let Some((source_rows, source_cols)) = ({
            let current_node = &self.treeview[self.tree_view_cursor];
            let node = current_node.node.borrow();
            match &node.node {
                Node::Dataset(_, dsattr) => Some((
                    dsattr.shape[node.selected_row],
                    dsattr.shape[node.selected_col],
                )),
                _ => None,
            }
        }) else {
            return Ok(EventResult::Continue);
        };
        let mut viewport = self.heatmap_render.viewport.unwrap_or(HeatmapViewport {
            row_start: 0,
            row_len: source_rows.max(1),
            col_start: 0,
            col_len: source_cols.max(1),
        });
        if viewport.col_len < source_cols {
            viewport.col_start = clamp_absolute_seek_start(col, source_cols, viewport.col_len);
            self.heatmap_render.viewport = Some(viewport);
        }
        if let Some(window) = self.heatmap_render.page_window.as_mut() {
            if matches!(window.axis, HeatmapPageAxis::Cols) {
                let relative = col
                    .saturating_sub(viewport.col_start)
                    .min(viewport.col_len.saturating_sub(1));
                window.page = window.page_for_target(relative);
            }
        }
        self.heatmap_render.current_key = None;
        Ok(EventResult::Redraw)
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
            ContentShowMode::Preview => match self.page_state.paged {
                PageType::Image => {
                    if let Some(window) = self.active_image_window_mut() {
                        let step = ((window.len / 4).max(1) as isize) * inc.max(1);
                        window.shift_by(step);
                        Ok(EventResult::Redraw)
                    } else {
                        self.down(1)
                    }
                }
                PageType::Chart => Ok(EventResult::Continue),
                PageType::Unpaged => {
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
            ContentShowMode::Preview => match self.page_state.paged {
                PageType::Image => {
                    if let Some(window) = self.active_image_window_mut() {
                        let step = ((window.len / 4).max(1) as isize) * inc.max(1);
                        window.shift_by(-step);
                        Ok(EventResult::Redraw)
                    } else {
                        self.up(1)
                    }
                }
                PageType::Chart => Ok(EventResult::Continue),
                PageType::Unpaged => {
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
