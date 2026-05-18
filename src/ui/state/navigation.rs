use crate::configure;

use super::{
    AppState, Focus, HelpCommandSection, HelpCustomizationSection, HelpKeymapSection,
    HelpMultiChartSection, HelpTab, LastFocused, LogsFilterFocus,
};

impl AppState<'_> {
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
