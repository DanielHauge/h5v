use super::{
    handlers::{
        handle_attr, handle_col, handle_configure, handle_dim, handle_down, handle_focus,
        handle_goto, handle_heatmap, handle_help, handle_index, handle_left, handle_mchart,
        handle_mode, handle_page_down, handle_page_up, handle_press, handle_reload, handle_repeat,
        handle_right, handle_row, handle_seek, handle_toggle_tree, handle_up, handle_x,
    },
    CommandArgKind, CommandArgSpec, CommandCategory, CommandDescriptor, CommandId,
};

const INDEX_ARG: CommandArgSpec = CommandArgSpec {
    name: "index",
    kind: CommandArgKind::UnsignedInt,
    required: true,
};

const OPTIONAL_AMOUNT_ARG: CommandArgSpec = CommandArgSpec {
    name: "amount",
    kind: CommandArgKind::UnsignedInt,
    required: false,
};

const TARGET_ARG: CommandArgSpec = CommandArgSpec {
    name: "target",
    kind: CommandArgKind::Word,
    required: true,
};

const DIRECTION_ARG: CommandArgSpec = CommandArgSpec {
    name: "direction",
    kind: CommandArgKind::Word,
    required: true,
};

const MODE_ARG: CommandArgSpec = CommandArgSpec {
    name: "mode",
    kind: CommandArgKind::Word,
    required: true,
};

const PATH_ARG: CommandArgSpec = CommandArgSpec {
    name: "path",
    kind: CommandArgKind::Word,
    required: true,
};

const OPTIONAL_COMMAND_ARG: CommandArgSpec = CommandArgSpec {
    name: "command",
    kind: CommandArgKind::Word,
    required: false,
};

const ACTION_ARG: CommandArgSpec = CommandArgSpec {
    name: "action",
    kind: CommandArgKind::Word,
    required: true,
};

const OPTIONAL_WORD_ARG: CommandArgSpec = CommandArgSpec {
    name: "arg",
    kind: CommandArgKind::Word,
    required: false,
};

const OPTIONAL_WORD_ARG_2: CommandArgSpec = CommandArgSpec {
    name: "arg2",
    kind: CommandArgKind::Word,
    required: false,
};

const OPTIONAL_WORD_ARG_3: CommandArgSpec = CommandArgSpec {
    name: "arg3",
    kind: CommandArgKind::Word,
    required: false,
};

const OPTIONAL_WORD_ARG_4: CommandArgSpec = CommandArgSpec {
    name: "arg4",
    kind: CommandArgKind::Word,
    required: false,
};

const KEY_ARG_1: CommandArgSpec = CommandArgSpec {
    name: "key1",
    kind: CommandArgKind::Word,
    required: true,
};

const KEY_ARG_2: CommandArgSpec = CommandArgSpec {
    name: "key2",
    kind: CommandArgKind::Word,
    required: false,
};

const KEY_ARG_3: CommandArgSpec = CommandArgSpec {
    name: "key3",
    kind: CommandArgKind::Word,
    required: false,
};

const KEY_ARG_4: CommandArgSpec = CommandArgSpec {
    name: "key4",
    kind: CommandArgKind::Word,
    required: false,
};

const COMMAND_CATALOG: &[CommandDescriptor] = &[
    CommandDescriptor {
        id: CommandId::Seek,
        name: "seek",
        aliases: &[],
        description: "Jump to an absolute index in the current content view",
        category: CommandCategory::Navigation,
        keybindings: &[],
        args: &[INDEX_ARG],
        handler: handle_seek,
    },
    CommandDescriptor {
        id: CommandId::Goto,
        name: "goto",
        aliases: &["jump", "open"],
        description: "Select a dataset or group by HDF5 path",
        category: CommandCategory::Navigation,
        keybindings: &[],
        args: &[PATH_ARG],
        handler: handle_goto,
    },
    CommandDescriptor {
        id: CommandId::Up,
        name: "up",
        aliases: &["dec", "decrement"],
        description: "Move up by a relative amount",
        category: CommandCategory::Navigation,
        keybindings: &["Up", "k"],
        args: &[OPTIONAL_AMOUNT_ARG],
        handler: handle_up,
    },
    CommandDescriptor {
        id: CommandId::Down,
        name: "down",
        aliases: &["inc", "increment"],
        description: "Move down by a relative amount",
        category: CommandCategory::Navigation,
        keybindings: &["Down", "j"],
        args: &[OPTIONAL_AMOUNT_ARG],
        handler: handle_down,
    },
    CommandDescriptor {
        id: CommandId::Left,
        name: "left",
        aliases: &[],
        description: "Move left by a relative amount",
        category: CommandCategory::Navigation,
        keybindings: &["Left", "h"],
        args: &[OPTIONAL_AMOUNT_ARG],
        handler: handle_left,
    },
    CommandDescriptor {
        id: CommandId::Right,
        name: "right",
        aliases: &[],
        description: "Move right by a relative amount",
        category: CommandCategory::Navigation,
        keybindings: &["Right", "l"],
        args: &[OPTIONAL_AMOUNT_ARG],
        handler: handle_right,
    },
    CommandDescriptor {
        id: CommandId::PageUp,
        name: "page-up",
        aliases: &["pgup"],
        description: "Move up by a page",
        category: CommandCategory::Navigation,
        keybindings: &["PageUp", "Ctrl+u"],
        args: &[],
        handler: handle_page_up,
    },
    CommandDescriptor {
        id: CommandId::PageDown,
        name: "page-down",
        aliases: &["pgdown"],
        description: "Move down by a page",
        category: CommandCategory::Navigation,
        keybindings: &["PageDown", "Ctrl+d"],
        args: &[],
        handler: handle_page_down,
    },
    CommandDescriptor {
        id: CommandId::Focus,
        name: "focus",
        aliases: &[],
        description: "Focus a target pane",
        category: CommandCategory::View,
        keybindings: &["Shift+Arrows"],
        args: &[TARGET_ARG],
        handler: handle_focus,
    },
    CommandDescriptor {
        id: CommandId::Mode,
        name: "mode",
        aliases: &["view-mode"],
        description: "Switch between preview and matrix modes",
        category: CommandCategory::View,
        keybindings: &["Tab"],
        args: &[MODE_ARG],
        handler: handle_mode,
    },
    CommandDescriptor {
        id: CommandId::ToggleTree,
        name: "toggle-tree",
        aliases: &["tree"],
        description: "Show or hide the tree pane",
        category: CommandCategory::View,
        keybindings: &["s"],
        args: &[],
        handler: handle_toggle_tree,
    },
    CommandDescriptor {
        id: CommandId::Reload,
        name: "reload",
        aliases: &["refresh"],
        description: "Reload the current file",
        category: CommandCategory::App,
        keybindings: &["Ctrl+r"],
        args: &[],
        handler: handle_reload,
    },
    CommandDescriptor {
        id: CommandId::Configure,
        name: "configure",
        aliases: &["config"],
        description: "Open the Lua config, or `configure reset` to recreate the default scaffold",
        category: CommandCategory::App,
        keybindings: &[],
        args: &[OPTIONAL_WORD_ARG],
        handler: handle_configure,
    },
    CommandDescriptor {
        id: CommandId::X,
        name: "x",
        aliases: &[],
        description: "Move the preview x-dimension selection",
        category: CommandCategory::Selection,
        keybindings: &["x", "X"],
        args: &[DIRECTION_ARG],
        handler: handle_x,
    },
    CommandDescriptor {
        id: CommandId::Row,
        name: "row",
        aliases: &[],
        description: "Move the matrix row-dimension selection",
        category: CommandCategory::Selection,
        keybindings: &["r", "R"],
        args: &[DIRECTION_ARG],
        handler: handle_row,
    },
    CommandDescriptor {
        id: CommandId::Col,
        name: "col",
        aliases: &["column"],
        description: "Move the matrix column-dimension selection",
        category: CommandCategory::Selection,
        keybindings: &["c", "C"],
        args: &[DIRECTION_ARG],
        handler: handle_col,
    },
    CommandDescriptor {
        id: CommandId::Dim,
        name: "dim",
        aliases: &["dimension"],
        description: "Move the selected dimension cursor",
        category: CommandCategory::Selection,
        keybindings: &["[", "]"],
        args: &[DIRECTION_ARG],
        handler: handle_dim,
    },
    CommandDescriptor {
        id: CommandId::Index,
        name: "index",
        aliases: &["selected-index"],
        description: "Move the selected index within the active dimension",
        category: CommandCategory::Selection,
        keybindings: &["Ctrl+a", "Ctrl+x", "Alt+Up/Down"],
        args: &[DIRECTION_ARG, OPTIONAL_AMOUNT_ARG],
        handler: handle_index,
    },
    CommandDescriptor {
        id: CommandId::Help,
        name: "help",
        aliases: &["?"],
        description: "Open help or show details for a command",
        category: CommandCategory::App,
        keybindings: &["?"],
        args: &[OPTIONAL_COMMAND_ARG],
        handler: handle_help,
    },
    CommandDescriptor {
        id: CommandId::Attr,
        name: "attr",
        aliases: &["attribute"],
        description: "Create or delete scalar attributes on the selected node",
        category: CommandCategory::Attributes,
        keybindings: &["a", "d", "Delete"],
        args: &[ACTION_ARG, OPTIONAL_WORD_ARG, OPTIONAL_WORD_ARG, OPTIONAL_WORD_ARG],
        handler: handle_attr,
    },
    CommandDescriptor {
        id: CommandId::Repeat,
        name: "repeat",
        aliases: &["again"],
        description: "Repeat the last successful command",
        category: CommandCategory::App,
        keybindings: &["."],
        args: &[],
        handler: handle_repeat,
    },
    CommandDescriptor {
        id: CommandId::MultiChart,
        name: "mchart",
        aliases: &["multichart"],
        description: "Control multichart from command mode: open, add, expr, derive, select, pan, zoom, clear, and more",
        category: CommandCategory::MultiChart,
        keybindings: &["M"],
        args: &[ACTION_ARG, OPTIONAL_WORD_ARG, OPTIONAL_WORD_ARG, OPTIONAL_WORD_ARG],
        handler: handle_mchart,
    },
    CommandDescriptor {
        id: CommandId::Press,
        name: "press",
        aliases: &["key", "keys"],
        description: "Simulate one or more key presses through the normal keymap dispatcher",
        category: CommandCategory::Input,
        keybindings: &[],
        args: &[KEY_ARG_1, KEY_ARG_2, KEY_ARG_3, KEY_ARG_4],
        handler: handle_press,
    },
    CommandDescriptor {
        id: CommandId::Heatmap,
        name: "heatmap",
        aliases: &[],
        description: "Manage heatmap-specific settings such as range presets",
        category: CommandCategory::View,
        keybindings: &[],
        args: &[
            ACTION_ARG,
            OPTIONAL_WORD_ARG,
            OPTIONAL_WORD_ARG_2,
            OPTIONAL_WORD_ARG_3,
            OPTIONAL_WORD_ARG_4,
        ],
        handler: handle_heatmap,
    },
];

pub fn command_catalog() -> &'static [CommandDescriptor] {
    COMMAND_CATALOG
}

pub fn find_command_descriptor(name: &str) -> Option<&'static CommandDescriptor> {
    let normalized = name.trim().to_ascii_lowercase();
    COMMAND_CATALOG.iter().find(|descriptor| {
        descriptor.name == normalized
            || descriptor
                .aliases
                .iter()
                .any(|alias| alias.eq_ignore_ascii_case(&normalized))
    })
}

pub fn find_command_descriptor_by_id(id: CommandId) -> Option<&'static CommandDescriptor> {
    COMMAND_CATALOG
        .iter()
        .find(|descriptor| descriptor.id == id)
}
