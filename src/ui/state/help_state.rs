#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HelpTab {
    Keymap,
    Commands,
    MultiChart,
    Heatmap,
    Configuration,
}

impl HelpTab {
    const ALL: [Self; 5] = [
        Self::Keymap,
        Self::Commands,
        Self::MultiChart,
        Self::Heatmap,
        Self::Configuration,
    ];

    pub(crate) fn step(self, delta: isize) -> Self {
        let current = Self::ALL.iter().position(|tab| *tab == self).unwrap_or(0) as isize;
        let next = (current + delta).rem_euclid(Self::ALL.len() as isize) as usize;
        Self::ALL[next]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HelpKeymapSection {
    Global,
    Normal,
    Window,
    Tree,
    Content,
    Heatmap,
    Attributes,
    MultiChart,
}

impl HelpKeymapSection {
    const ALL: [Self; 8] = [
        Self::Global,
        Self::Normal,
        Self::Window,
        Self::Tree,
        Self::Content,
        Self::Heatmap,
        Self::Attributes,
        Self::MultiChart,
    ];

    pub(crate) fn step(self, delta: isize) -> Self {
        let current = Self::ALL
            .iter()
            .position(|section| *section == self)
            .unwrap_or(0) as isize;
        let next = (current + delta).rem_euclid(Self::ALL.len() as isize) as usize;
        Self::ALL[next]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HelpCommandSection {
    Navigation,
    View,
    Selection,
    Attributes,
    App,
    MultiChart,
    Input,
}

impl HelpCommandSection {
    const ALL: [Self; 7] = [
        Self::Navigation,
        Self::View,
        Self::Selection,
        Self::Attributes,
        Self::App,
        Self::MultiChart,
        Self::Input,
    ];

    pub(crate) fn step(self, delta: isize) -> Self {
        let current = Self::ALL
            .iter()
            .position(|section| *section == self)
            .unwrap_or(0) as isize;
        let next = (current + delta).rem_euclid(Self::ALL.len() as isize) as usize;
        Self::ALL[next]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HelpCustomizationSection {
    Configuration,
    Settings,
    Colors,
    Symbols,
    Keymaps,
    Scripting,
}

impl HelpCustomizationSection {
    const ALL: [Self; 6] = [
        Self::Configuration,
        Self::Settings,
        Self::Colors,
        Self::Symbols,
        Self::Keymaps,
        Self::Scripting,
    ];

    pub(crate) fn step(self, delta: isize) -> Self {
        let current = Self::ALL
            .iter()
            .position(|section| *section == self)
            .unwrap_or(0) as isize;
        let next = (current + delta).rem_euclid(Self::ALL.len() as isize) as usize;
        Self::ALL[next]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HelpMultiChartSection {
    Overview,
    Expressions,
    FunctionReducers,
    FunctionMath,
    FunctionTransforms,
}

impl HelpMultiChartSection {
    const ALL: [Self; 5] = [
        Self::Overview,
        Self::Expressions,
        Self::FunctionReducers,
        Self::FunctionMath,
        Self::FunctionTransforms,
    ];

    pub(crate) fn step(self, delta: isize) -> Self {
        let current = Self::ALL
            .iter()
            .position(|section| *section == self)
            .unwrap_or(0) as isize;
        let next = (current + delta).rem_euclid(Self::ALL.len() as isize) as usize;
        Self::ALL[next]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HelpViewState {
    pub selected_tab: HelpTab,
    pub keymap_section: HelpKeymapSection,
    pub command_section: HelpCommandSection,
    pub customization_section: HelpCustomizationSection,
    pub multichart_section: HelpMultiChartSection,
    pub scroll_offset: usize,
}

impl Default for HelpViewState {
    fn default() -> Self {
        Self {
            selected_tab: HelpTab::Keymap,
            keymap_section: HelpKeymapSection::Global,
            command_section: HelpCommandSection::Navigation,
            customization_section: HelpCustomizationSection::Configuration,
            multichart_section: HelpMultiChartSection::Overview,
            scroll_offset: 0,
        }
    }
}
