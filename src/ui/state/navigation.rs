use std::{rc::Rc, sync::mpsc::channel};

use crate::{
    configure,
    h5f::{H5FNode, Node},
};

use super::{
    AppState, Focus, HelpCommandSection, HelpCustomizationSection, HelpKeymapSection,
    HelpMultiChartSection, HelpTab, LastFocused, LogsFilterFocus,
};

impl AppState<'_> {
    pub fn drain_content_previews(&mut self) {
        let (done_tx, done_rx) = std::sync::mpsc::channel();
        let _ = self
            .content_preview_tx
            .send(super::ContentPreviewWork::Drain(done_tx));
        let _ = done_rx.recv();
    }

    pub fn drain_matrix_viewports(&mut self) {
        let (tx, rx) = std::sync::mpsc::channel();
        let _ = self
            .matrix_viewport_state
            .tx_load
            .send(super::MatrixViewportWork::Drain(tx));
        let _ = rx.recv();
    }
    pub fn invalidate_selected_navigation_data(&mut self) {
        self.navigation_generation = self.navigation_generation.wrapping_add(1);
        self.pending_navigation_request = None;
        self.content_preview_state.pending_key = None;
        self.content_preview_state.error = None;
        self.content_preview_state.cached.clear();
    }

    /// Queues the selected node's lazy metadata and attributes.  One worker keeps HDF5 access
    /// ordered with reload; later selections make earlier replies irrelevant.
    pub fn request_selected_navigation_data(&mut self) {
        let Some(item) = self.treeview.get(self.tree_view_cursor) else {
            return;
        };
        let mut node = item.node.borrow_mut();
        if node.computed_attributes.is_some()
            && self.pending_tree_selection_state.is_none()
            && self.pending_tree_attribute_selection.is_none()
            && !matches!(
                node.node,
                Node::Dataset(_, crate::h5f::DatasetMetaState::Pending(_))
            )
        {
            return;
        }
        if (node.metadata_loading || node.attributes_loading)
            && self.pending_navigation_request.is_some()
        {
            return;
        }
        let request_id = self.next_navigation_request_id;
        self.next_navigation_request_id = self.next_navigation_request_id.wrapping_add(1);
        node.metadata_loading = matches!(
            node.node,
            Node::Dataset(_, crate::h5f::DatasetMetaState::Pending(_))
        );
        node.attributes_loading = true;
        node.metadata_error = None;
        node.attributes_error = None;
        let request = crate::ui::app::NavigationLoadRequest {
            generation: self.navigation_generation,
            request_id,
            node: node.node.clone(),
        };
        drop(node);
        self.pending_navigation_request = Some(request_id);
        if self
            .navigation_load_tx
            .send(crate::ui::app::NavigationLoadWork::Load(request))
            .is_err()
        {
            self.pending_navigation_request = None;
            let mut node = item.node.borrow_mut();
            node.metadata_loading = false;
            node.attributes_loading = false;
            node.attributes_error = Some("Navigation loading worker stopped".to_string());
        }
    }

    /// Wait for the metadata worker before closing its HDF5 handles during reload/shutdown.
    pub fn drain_navigation_loads(&mut self) {
        let (done_tx, done_rx) = channel();
        if self
            .navigation_load_tx
            .send(crate::ui::app::NavigationLoadWork::Drain(done_tx))
            .is_ok()
        {
            let _ = done_rx.recv();
        }
        self.pending_navigation_request = None;
    }

    pub fn request_tree_children(&mut self, node: Rc<std::cell::RefCell<H5FNode>>) {
        let mut node_ref = node.borrow_mut();
        if node_ref.read || node_ref.loading {
            node_ref.expanded = true;
            return;
        }
        let Some(enumeration_node) = node_ref.enumeration_node() else {
            return;
        };
        node_ref.expanded = true;
        node_ref.loading = true;
        node_ref.load_error = None;
        let request_id = self.next_tree_load_request_id;
        self.next_tree_load_request_id += 1;
        drop(node_ref);
        self.pending_tree_loads.push((request_id, node.clone()));
        if self
            .tree_load_tx
            .send(crate::ui::app::TreeLoadWork::Load(
                crate::ui::app::TreeLoadRequest {
                    generation: self.tree_load_generation,
                    request_id,
                    node: enumeration_node,
                },
            ))
            .is_err()
        {
            self.pending_tree_loads.retain(|(id, _)| *id != request_id);
            node.borrow_mut()
                .apply_enumeration_error("Tree loading worker stopped".to_string());
        }
    }

    /// Wait until the single tree worker has finished every old HDF5 traversal.
    pub fn drain_tree_loads(&mut self) {
        let (done_tx, done_rx) = channel();
        if self
            .tree_load_tx
            .send(crate::ui::app::TreeLoadWork::Drain(done_tx))
            .is_ok()
        {
            let _ = done_rx.recv();
        }
        self.pending_tree_loads.clear();
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
        self.help.scroll_offset = 0;
        true
    }

    pub fn help_prev_tab(&mut self) -> bool {
        let next = self.help.selected_tab.step(-1);
        if next == self.help.selected_tab {
            return false;
        }
        self.help.selected_tab = next;
        self.help.scroll_offset = 0;
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
                self.help.scroll_offset = 0;
                true
            }
            HelpTab::Commands => {
                let next = self.help.command_section.step(1);
                if next == self.help.command_section {
                    return false;
                }
                self.help.command_section = next;
                self.help.scroll_offset = 0;
                true
            }
            HelpTab::Configuration => {
                let next = self.help.customization_section.step(1);
                if next == self.help.customization_section {
                    return false;
                }
                self.help.customization_section = next;
                self.help.scroll_offset = 0;
                true
            }
            HelpTab::MultiChart => {
                let next = self.help.multichart_section.step(1);
                if next == self.help.multichart_section {
                    return false;
                }
                self.help.multichart_section = next;
                self.help.scroll_offset = 0;
                true
            }
            HelpTab::Health => {
                let last = self.health_section_count().saturating_sub(1);
                if self.help.health_section >= last {
                    false
                } else {
                    self.help.health_section += 1;
                    self.help.scroll_offset = 0;
                    true
                }
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
                self.help.scroll_offset = 0;
                true
            }
            HelpTab::Commands => {
                let next = self.help.command_section.step(-1);
                if next == self.help.command_section {
                    return false;
                }
                self.help.command_section = next;
                self.help.scroll_offset = 0;
                true
            }
            HelpTab::Configuration => {
                let next = self.help.customization_section.step(-1);
                if next == self.help.customization_section {
                    return false;
                }
                self.help.customization_section = next;
                self.help.scroll_offset = 0;
                true
            }
            HelpTab::MultiChart => {
                let next = self.help.multichart_section.step(-1);
                if next == self.help.multichart_section {
                    return false;
                }
                self.help.multichart_section = next;
                self.help.scroll_offset = 0;
                true
            }
            HelpTab::Health => {
                if self.help.health_section == 0 {
                    false
                } else {
                    self.help.health_section -= 1;
                    self.help.scroll_offset = 0;
                    true
                }
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
                    self.help.scroll_offset = 0;
                    true
                }
            }
            HelpTab::Commands => {
                if self.help.command_section == HelpCommandSection::Navigation {
                    false
                } else {
                    self.help.command_section = HelpCommandSection::Navigation;
                    self.help.scroll_offset = 0;
                    true
                }
            }
            HelpTab::Configuration => {
                if self.help.customization_section == HelpCustomizationSection::Configuration {
                    false
                } else {
                    self.help.customization_section = HelpCustomizationSection::Configuration;
                    self.help.scroll_offset = 0;
                    true
                }
            }
            HelpTab::MultiChart => {
                if self.help.multichart_section == HelpMultiChartSection::Overview {
                    false
                } else {
                    self.help.multichart_section = HelpMultiChartSection::Overview;
                    self.help.scroll_offset = 0;
                    true
                }
            }
            HelpTab::Health => {
                if self.help.health_section == 0 {
                    false
                } else {
                    self.help.health_section = 0;
                    self.help.scroll_offset = 0;
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
                    self.help.scroll_offset = 0;
                    true
                }
            }
            HelpTab::Commands => {
                if self.help.command_section == HelpCommandSection::Input {
                    false
                } else {
                    self.help.command_section = HelpCommandSection::Input;
                    self.help.scroll_offset = 0;
                    true
                }
            }
            HelpTab::Configuration => {
                if self.help.customization_section == HelpCustomizationSection::Scripting {
                    false
                } else {
                    self.help.customization_section = HelpCustomizationSection::Scripting;
                    self.help.scroll_offset = 0;
                    true
                }
            }
            HelpTab::MultiChart => {
                if self.help.multichart_section == HelpMultiChartSection::FunctionTransforms {
                    false
                } else {
                    self.help.multichart_section = HelpMultiChartSection::FunctionTransforms;
                    self.help.scroll_offset = 0;
                    true
                }
            }
            HelpTab::Health => {
                let last = self.health_section_count().saturating_sub(1);
                if self.help.health_section == last {
                    false
                } else {
                    self.help.health_section = last;
                    self.help.scroll_offset = 0;
                    true
                }
            }
            _ => false,
        }
    }

    pub fn help_scroll_by(&mut self, delta: isize, max_scroll: usize) -> bool {
        let next = self
            .help
            .scroll_offset
            .saturating_add_signed(delta)
            .min(max_scroll);
        if next == self.help.scroll_offset {
            return false;
        }
        self.help.scroll_offset = next;
        true
    }

    pub fn help_set_scroll(&mut self, offset: usize, max_scroll: usize) -> bool {
        let next = offset.min(max_scroll);
        if next == self.help.scroll_offset {
            return false;
        }
        self.help.scroll_offset = next;
        true
    }

    fn health_section_count(&self) -> usize {
        1 + configure::current_registry_snapshot().plugins().count()
    }

    pub fn logs_max_scroll(&self) -> usize {
        self.logs
            .content_lines
            .saturating_sub(self.logs.viewport_lines.max(1))
    }

    pub fn logs_scroll_by(&mut self, delta: isize) -> bool {
        let next = self
            .logs
            .scroll_offset
            .saturating_add_signed(delta)
            .min(self.logs_max_scroll());
        if next == self.logs.scroll_offset {
            return false;
        }
        self.logs.scroll_offset = next;
        true
    }

    pub fn logs_set_scroll(&mut self, offset: usize) -> bool {
        let next = offset.min(self.logs_max_scroll());
        if next == self.logs.scroll_offset {
            return false;
        }
        self.logs.scroll_offset = next;
        true
    }

    pub fn logs_next_filter_focus(&mut self) {
        self.logs.filter_focus = match self.logs.filter_focus {
            LogsFilterFocus::Scope => LogsFilterFocus::Level,
            LogsFilterFocus::Level => LogsFilterFocus::Handle,
            LogsFilterFocus::Handle => LogsFilterFocus::Scope,
        };
    }

    pub fn logs_prev_filter_focus(&mut self) {
        self.logs.filter_focus = match self.logs.filter_focus {
            LogsFilterFocus::Scope => LogsFilterFocus::Handle,
            LogsFilterFocus::Level => LogsFilterFocus::Scope,
            LogsFilterFocus::Handle => LogsFilterFocus::Level,
        };
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
}
