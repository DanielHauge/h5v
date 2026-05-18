use crate::{configure, configure::registry::ContentModeHandle};

use super::{AppState, ContentShowMode};

impl AppState<'_> {
    pub fn swap_content_show_mode(&mut self, available: Vec<ContentShowMode>) {
        self.swap_content_mode_handle(self.available_content_mode_handles(available));
    }

    pub fn swap_content_mode_handle(&mut self, available: Vec<ContentModeHandle>) {
        let ordered = configure::ordered_content_mode_handles(&available);
        if ordered.is_empty() {
            return;
        }
        let current_index = ordered
            .iter()
            .position(|handle| *handle == self.content_mode)
            .unwrap_or(0);
        self.set_content_mode_handle(ordered[(current_index + 1) % ordered.len()].clone());
    }

    pub fn set_content_mode(&mut self, mode: ContentShowMode) {
        if self.content_mode == ContentShowMode::Heatmap.handle()
            && mode != ContentShowMode::Heatmap
        {
            self.end_heatmap_drag();
        }
        self.content_mode = mode.handle();
    }

    pub fn set_content_mode_handle(&mut self, handle: ContentModeHandle) {
        if self.content_mode == ContentShowMode::Heatmap.handle()
            && handle != ContentShowMode::Heatmap.handle()
        {
            self.end_heatmap_drag();
        }
        self.content_mode = handle;
    }

    pub fn content_mode_handle_eval(&self, available: Vec<ContentModeHandle>) -> ContentModeHandle {
        if let Some(handle) = available
            .iter()
            .find(|handle| **handle == self.content_mode)
            .cloned()
        {
            handle
        } else {
            configure::ordered_content_mode_handles(&available)
                .first()
                .cloned()
                .unwrap_or_else(|| ContentShowMode::Preview.handle())
        }
    }

    pub fn active_content_mode_handle(&self) -> ContentModeHandle {
        let available = self
            .treeview
            .get(self.tree_view_cursor)
            .and_then(|item| {
                item.node
                    .try_borrow()
                    .ok()
                    .map(|node| self.available_content_mode_handles(node.content_show_modes()))
            })
            .unwrap_or_else(|| vec![self.content_mode.clone()]);
        self.content_mode_handle_eval(available)
    }

    pub fn content_show_mode_eval(&self, available: Vec<ContentShowMode>) -> ContentShowMode {
        let available = self.filter_runtime_content_modes(available);
        if let Some(mode) = available
            .iter()
            .copied()
            .find(|mode| mode.handle() == self.content_mode)
        {
            mode
        } else {
            configure::ordered_content_modes(&available)
                .first()
                .copied()
                .unwrap_or(ContentShowMode::Preview)
        }
    }

    pub fn active_content_mode(&self) -> ContentShowMode {
        if let Some(mode) =
            ContentShowMode::parse_handle(self.active_content_mode_handle().as_str())
        {
            return mode;
        }
        let available = self
            .treeview
            .get(self.tree_view_cursor)
            .and_then(|item| {
                item.node
                    .try_borrow()
                    .ok()
                    .map(|node| node.content_show_modes())
            })
            .unwrap_or_else(|| {
                vec![ContentShowMode::parse_handle(self.content_mode.as_str())
                    .unwrap_or(ContentShowMode::Preview)]
            });
        self.content_show_mode_eval(available)
    }

    pub fn available_content_mode_handles(
        &self,
        available: Vec<ContentShowMode>,
    ) -> Vec<ContentModeHandle> {
        let mut handles = self
            .filter_runtime_content_modes(available)
            .into_iter()
            .map(ContentShowMode::handle)
            .collect::<Vec<_>>();
        if let Ok(custom_handles) = configure::available_lua_content_mode_handles(self) {
            for handle in custom_handles {
                if !handles.contains(&handle) {
                    handles.push(handle);
                }
            }
            return handles;
        }
        let registry = configure::current_registry_snapshot();
        for metadata in registry.content_modes() {
            if ContentShowMode::parse_handle(metadata.handle.as_str()).is_none()
                && metadata.callback_id.is_some()
                && !handles.contains(&metadata.handle)
            {
                handles.push(metadata.handle.clone());
            }
        }
        handles
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
}
