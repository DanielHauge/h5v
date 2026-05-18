#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum LuaContentNode {
    Text(String),
    Code {
        body: String,
        kind: Option<String>,
    },
    Badge(String),
    KeyValue {
        key: String,
        value: String,
    },
    Separator {
        label: Option<String>,
        empty: bool,
        height: usize,
    },
    Row {
        children: Vec<LuaContentNode>,
    },
    Column {
        children: Vec<LuaContentNode>,
    },
    Split {
        direction: LuaSplitDirection,
        ratio_millis: u16,
        gap: usize,
        left: Vec<LuaContentNode>,
        right: Vec<LuaContentNode>,
    },
    Table {
        rows: Vec<Vec<String>>,
    },
    Block {
        title: Option<String>,
        children: Vec<LuaContentNode>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LuaSplitDirection {
    Horizontal,
    Vertical,
}
