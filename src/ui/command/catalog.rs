use super::{
    handlers::{
        handle_attr, handle_col, handle_configure, handle_dim, handle_down, handle_focus,
        handle_goto, handle_heatmap, handle_help, handle_index, handle_left, handle_mchart,
        handle_mode, handle_page_down, handle_page_up, handle_press, handle_reload, handle_repeat,
        handle_right, handle_row, handle_seek, handle_seek_col, handle_seek_row,
        handle_toggle_tree, handle_up, handle_x,
    },
    CommandArgKind, CommandArgSpec, CommandCategory, CommandDescriptor, CommandId,
};

const INDEX_ARG: CommandArgSpec = CommandArgSpec {
    name: "index",
    kind: CommandArgKind::UnsignedInt,
    required: true,
    help: "Zero-based absolute index to jump to.",
    values: &[],
};

const SEEK_PRIMARY_ARG: CommandArgSpec = CommandArgSpec {
    name: "index",
    kind: CommandArgKind::UnsignedInt,
    required: true,
    help: "Absolute index, or x/column when a second y argument is provided in matrix or heatmap mode.",
    values: &[],
};

const SEEK_SECONDARY_ARG: CommandArgSpec = CommandArgSpec {
    name: "index2",
    kind: CommandArgKind::UnsignedInt,
    required: false,
    help: "Optional y/row coordinate for matrix or heatmap mode.",
    values: &[],
};

const ROW_INDEX_ARG: CommandArgSpec = CommandArgSpec {
    name: "row",
    kind: CommandArgKind::UnsignedInt,
    required: true,
    help: "Absolute row index to bring into view.",
    values: &[],
};

const COL_INDEX_ARG: CommandArgSpec = CommandArgSpec {
    name: "col",
    kind: CommandArgKind::UnsignedInt,
    required: true,
    help: "Absolute column index to bring into view.",
    values: &[],
};

const OPTIONAL_AMOUNT_ARG: CommandArgSpec = CommandArgSpec {
    name: "amount",
    kind: CommandArgKind::UnsignedInt,
    required: false,
    help: "Optional positive step count.",
    values: &["1", "5", "10"],
};

const TARGET_ARG: CommandArgSpec = CommandArgSpec {
    name: "target",
    kind: CommandArgKind::Word,
    required: true,
    help: "Pane to focus.",
    values: &["tree", "attributes", "content"],
};

const DIRECTION_ARG: CommandArgSpec = CommandArgSpec {
    name: "direction",
    kind: CommandArgKind::Word,
    required: true,
    help: "Relative direction to move.",
    values: &[
        "next", "prev", "forward", "back", "left", "right", "up", "down",
    ],
};

const MODE_ARG: CommandArgSpec = CommandArgSpec {
    name: "mode",
    kind: CommandArgKind::Word,
    required: true,
    help: "Content mode to activate.",
    values: &["preview", "matrix", "heatmap"],
};

const PATH_ARG: CommandArgSpec = CommandArgSpec {
    name: "path",
    kind: CommandArgKind::Word,
    required: true,
    help: "Absolute HDF5 path to select in the tree.",
    values: &["/group/dataset"],
};

const OPTIONAL_COMMAND_ARG: CommandArgSpec = CommandArgSpec {
    name: "command",
    kind: CommandArgKind::Word,
    required: false,
    help: "Optional command name to inspect.",
    values: &["help", "mchart", "configure"],
};

const ATTR_ACTION_ARG: CommandArgSpec = CommandArgSpec {
    name: "action",
    kind: CommandArgKind::Word,
    required: true,
    help: "Attribute action to run.",
    values: &["create", "delete"],
};

const CONFIGURE_ACTION_ARG: CommandArgSpec = CommandArgSpec {
    name: "action",
    kind: CommandArgKind::Word,
    required: false,
    help: "Optional configure action.",
    values: &["reset"],
};

const ATTR_NAME_ARG: CommandArgSpec = CommandArgSpec {
    name: "name",
    kind: CommandArgKind::Word,
    required: false,
    help: "Attribute name on the selected node.",
    values: &["title", "scale"],
};

const ATTR_TYPE_ARG: CommandArgSpec = CommandArgSpec {
    name: "type",
    kind: CommandArgKind::Word,
    required: false,
    help: "Attribute type when creating.",
    values: &["bool", "i64", "u64", "f64", "string", "ascii"],
};

const ATTR_VALUE_ARG: CommandArgSpec = CommandArgSpec {
    name: "value",
    kind: CommandArgKind::Word,
    required: false,
    help: "Optional initial value when creating.",
    values: &[],
};

const MCHART_ACTION_ARG: CommandArgSpec = CommandArgSpec {
    name: "action",
    kind: CommandArgKind::Word,
    required: true,
    help: "Multichart action to run.",
    values: &[
        "open", "close", "toggle", "add", "expr", "prompt", "select", "visible", "remove", "clear",
        "fit", "zoom", "pan",
    ],
};

const MCHART_ARG_1: CommandArgSpec = CommandArgSpec {
    name: "arg",
    kind: CommandArgKind::Word,
    required: false,
    help: "Subcommand-specific argument such as a dataset spec, expression, target, or direction.",
    values: &[],
};

const MCHART_ARG_2: CommandArgSpec = CommandArgSpec {
    name: "arg2",
    kind: CommandArgKind::Word,
    required: false,
    help: "Optional extra argument such as amount, zoom action, or selector.",
    values: &[],
};

const MCHART_ARG_3: CommandArgSpec = CommandArgSpec {
    name: "arg3",
    kind: CommandArgKind::Word,
    required: false,
    help: "Optional extra argument such as amount or label.",
    values: &[],
};

const HEATMAP_ACTION_ARG: CommandArgSpec = CommandArgSpec {
    name: "action",
    kind: CommandArgKind::Word,
    required: true,
    help: "Heatmap command family.",
    values: &["range"],
};

const HEATMAP_ARG_1: CommandArgSpec = CommandArgSpec {
    name: "arg",
    kind: CommandArgKind::Word,
    required: false,
    help: "Range action.",
    values: &["list", "use", "select", "add"],
};

const HEATMAP_ARG_2: CommandArgSpec = CommandArgSpec {
    name: "arg2",
    kind: CommandArgKind::Word,
    required: false,
    help: "Range selector or lower bound, depending on the action.",
    values: &[],
};

const HEATMAP_ARG_3: CommandArgSpec = CommandArgSpec {
    name: "arg3",
    kind: CommandArgKind::Word,
    required: false,
    help: "Upper bound for `heatmap range add`.",
    values: &[],
};

const HEATMAP_ARG_4: CommandArgSpec = CommandArgSpec {
    name: "arg4",
    kind: CommandArgKind::Word,
    required: false,
    help: "Optional label for `heatmap range add`.",
    values: &[],
};

const KEY_ARG_1: CommandArgSpec = CommandArgSpec {
    name: "key1",
    kind: CommandArgKind::Word,
    required: true,
    help: "Key spec to simulate through the input dispatcher.",
    values: &["ctrl+w", "o", "shift+tab"],
};

const KEY_ARG_2: CommandArgSpec = CommandArgSpec {
    name: "key2",
    kind: CommandArgKind::Word,
    required: false,
    help: "Optional extra key spec to simulate.",
    values: &[],
};

const KEY_ARG_3: CommandArgSpec = CommandArgSpec {
    name: "key3",
    kind: CommandArgKind::Word,
    required: false,
    help: "Optional extra key spec to simulate.",
    values: &[],
};

const KEY_ARG_4: CommandArgSpec = CommandArgSpec {
    name: "key4",
    kind: CommandArgKind::Word,
    required: false,
    help: "Optional extra key spec to simulate.",
    values: &[],
};

const COMMAND_CATALOG: &[CommandDescriptor] = &[
    CommandDescriptor {
        id: CommandId::Seek,
        name: "seek",
        aliases: &[],
        description: "Jump to an absolute index, or to x/y coordinates in matrix and heatmap views",
        category: CommandCategory::Navigation,
        keybindings: &[],
        args: &[SEEK_PRIMARY_ARG, SEEK_SECONDARY_ARG],
        example: "seek 25 35",
        handler: handle_seek,
    },
    CommandDescriptor {
        id: CommandId::SeekRow,
        name: "seek-row",
        aliases: &["row-seek"],
        description: "Jump to an absolute row while keeping the current column or x position",
        category: CommandCategory::Navigation,
        keybindings: &[],
        args: &[ROW_INDEX_ARG],
        example: "seek-row 35",
        handler: handle_seek_row,
    },
    CommandDescriptor {
        id: CommandId::SeekCol,
        name: "seek-col",
        aliases: &["col-seek", "seek-column"],
        description: "Jump to an absolute column while keeping the current row or y position",
        category: CommandCategory::Navigation,
        keybindings: &[],
        args: &[COL_INDEX_ARG],
        example: "seek-col 25",
        handler: handle_seek_col,
    },
    CommandDescriptor {
        id: CommandId::Goto,
        name: "goto",
        aliases: &["jump", "open"],
        description: "Select a dataset or group by HDF5 path",
        category: CommandCategory::Navigation,
        keybindings: &[],
        args: &[PATH_ARG],
        example: "goto /runs/run_04/signal",
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
        example: "up 5",
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
        example: "down 10",
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
        example: "left 1",
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
        example: "right 1",
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
        example: "page-up",
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
        example: "page-down",
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
        example: "focus content",
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
        example: "mode heatmap",
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
        example: "toggle-tree",
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
        example: "reload",
        handler: handle_reload,
    },
    CommandDescriptor {
        id: CommandId::Configure,
        name: "configure",
        aliases: &["config"],
        description: "Open the Lua config, or `configure reset` to recreate the default scaffold",
        category: CommandCategory::App,
        keybindings: &[],
        args: &[CONFIGURE_ACTION_ARG],
        example: "configure reset",
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
        example: "x next",
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
        example: "row prev",
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
        example: "col next",
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
        example: "dim next",
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
        example: "index next 4",
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
        example: "help mchart",
        handler: handle_help,
    },
    CommandDescriptor {
        id: CommandId::Attr,
        name: "attr",
        aliases: &["attribute"],
        description: "Create or delete scalar attributes on the selected node",
        category: CommandCategory::Attributes,
        keybindings: &["a", "d", "Delete"],
        args: &[ATTR_ACTION_ARG, ATTR_NAME_ARG, ATTR_TYPE_ARG, ATTR_VALUE_ARG],
        example: "attr create title string \"Run 42\"",
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
        example: "repeat",
        handler: handle_repeat,
    },
    CommandDescriptor {
        id: CommandId::MultiChart,
        name: "mchart",
        aliases: &["multichart"],
        description: "Control multichart from command mode: open, add, expr, derive, select, pan, zoom, clear, and more",
        category: CommandCategory::MultiChart,
        keybindings: &["M"],
        args: &[MCHART_ACTION_ARG, MCHART_ARG_1, MCHART_ARG_2, MCHART_ARG_3],
        example: "mchart add /group/signal[..,0]",
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
        example: "press ctrl+w o",
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
            HEATMAP_ACTION_ARG,
            HEATMAP_ARG_1,
            HEATMAP_ARG_2,
            HEATMAP_ARG_3,
            HEATMAP_ARG_4,
        ],
        example: "heatmap range use \"Clip 1-99%\"",
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
