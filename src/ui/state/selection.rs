use super::*;

impl AppState<'_> {
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
            .and_then(|item| item.node.try_borrow().ok().map(|node| node.node.path()))
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
                    preview_selection_for_node(&mut node, &shape, self.page_state.idx)
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
                    if next_next != node.selected_row
                        && (is_compound_root_matrix || next_next != node.selected_col)
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
}
