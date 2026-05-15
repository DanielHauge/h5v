# Configuration reference

## Core config fields

| Field | Type | Notes |
| --- | --- | --- |
| `h5v.theme` | string | Built-in color theme name |
| `h5v.symbol_theme` | string | Built-in symbol theme name |
| `h5v.compatibility` | boolean | Compatibility mode override from config |
| `h5v.content_mode_order` | string array | Ordered content-mode preference |
| `h5v.layout` | table | Focus-aware auto sizing for tree, attributes, and content panels |
| `h5v.heatmap` | table | Preferred heatmap defaults and custom range presets |
| `h5v.multichart` | table | Multichart overview sampling and viewport-refinement tuning |
| `h5v.keymaps` | table | Scoped keymap overrides and command bindings |
| `h5v.colors` | table | Per-key color overrides |
| `h5v.symbols` | table | Per-key symbol overrides |
| `h5v.themes.<name>` | table | Built-in color catalogs |
| `h5v.symbol_themes.<name>` | table | Built-in symbol catalogs |
| `h5v.log("message")` | function | Show a small info toast while config runs |

## Built-in themes

| Theme |
| --- |
| `dark` |
| `light` |

## Built-in symbol themes

| Theme |
| --- |
| `rich` |
| `compatibility` |

## Content modes

| Mode |
| --- |
| `preview` |
| `matrix` |
| `heatmap` |

`heatmap` in `h5v.content_mode_order` only affects preferred tab order. Heatmap appears only when compatibility mode is off and terminal image rendering is available.

## Color categories

| Category | Purpose |
| --- | --- |
| `accent` | Selection, highlight, and search accents |
| `text` | Text rendering |
| `content` | Header, tabs, empty states |
| `command` | Minibuffer text |
| `help` | Help overlay text |
| `metadata` | Metadata labels and values |
| `file` | File preview labels and values |
| `mchart` | Multichart list and prompt colors |
| `surface` | Backgrounds, borders, highlights |
| `tree` | Tree lines and node colors |
| `chart` | Axes, grids, plot area, chart series |
| `status` | Read-only, writable, linked, update status |
| `toast` | Toast border colors |

## Symbol categories

| Category | Purpose |
| --- | --- |
| `tree` | Tree connectors, arrows, icons |
| `section` | Metadata section titles |
| `title` | Panel and dialog titles |
| `badge` | Header badges and linked markers |
| `chart` | Multichart markers and enum symbols |

## Accepted color values

Hex values: `#RRGGBB`

| Canonical name | Accepted aliases |
| --- | --- |
| `black` | `black` |
| `red` | `red` |
| `green` | `green` |
| `yellow` | `yellow` |
| `blue` | `blue` |
| `magenta` | `magenta`, `purple` |
| `cyan` | `cyan` |
| `gray` | `gray`, `grey` |
| `darkgray` | `darkgray`, `darkgrey`, `dark_gray`, `dark_grey` |
| `lightred` | `lightred`, `light_red`, `pink` |
| `lightgreen` | `lightgreen`, `light_green` |
| `lightyellow` | `lightyellow`, `light_yellow` |
| `lightblue` | `lightblue`, `light_blue` |
| `lightmagenta` | `lightmagenta`, `light_magenta` |
| `lightcyan` | `lightcyan`, `light_cyan` |
| `white` | `white` |
| `amber` | `amber` |
| `orange` | `orange` |

## Color override keys

| Group | Keys |
| --- | --- |
| `text` | `primary`, `number`, `string`, `opaque`, `bool_value`, `error`, `search_text`, `search_count`, `type_desc`, `line_num` |
| `content` | `app_brand`, `app_version`, `help_hint`, `empty_state`, `tab_active`, `tab_inactive`, `tree_membership_more` |
| `command` | `prompt_prefix`, `usage`, `description`, `suggestion_label`, `no_match`, `key_hint` |
| `help` | `title`, `section`, `description`, `muted` |
| `metadata` | `section`, `property_name`, `property_value`, `attribute_name` |
| `file` | `section_title`, `label`, `value` |
| `mchart` | `empty_state`, `item_selected`, `item_selected_hidden`, `item_visible`, `item_hidden`, `prefix_selected`, `prefix`, `detail_label`, `prompt_prefix` |
| `surface` | `title_bg`, `focus_bg`, `bg`, `bg_val1`, `bg_val2`, `bg_val3`, `bg_val4`, `break_line`, `highlight_bg`, `highlight_bg_copy`, `panel_border`, `panel_title`, `help_key_bg`, `image_border` |
| `tree` | `lines`, `root_file`, `variable`, `variable_builtin`, `file`, `group`, `compound_name`, `dataset`, `dataset_file`, `compound`, `load_more` |
| `accent` | `selected_index`, `selected_dim`, `equal_sign`, `symbol`, `selection_fg`, `selection_bg`, `search_highlight`, `search_icon` |
| `chart` | `axis`, `grid`, `label`, `preview_line`, `plot_bg`, `series_1` to `series_8`, `enum_1` to `enum_8` |
| `status` | `readonly`, `writable`, `linked`, `compability`, `update_available` |
| `toast` | `info`, `warning`, `neutral` |

## Symbol override keys

| Group | Keys |
| --- | --- |
| `tree` | `horizontal_rule`, `connector_last`, `connector_middle`, `vertical_guide`, `collapse_expanded`, `collapse_collapsed`, `folder_open_branch`, `folder_open_leaf`, `folder_closed_branch`, `folder_closed_leaf`, `root_file_icon`, `dataset_icon`, `dataset_link_icon`, `compound_container_icon`, `compound_leaf_icon`, `link_marker`, `broken_node_icon`, `load_more_label` |
| `section` | `properties_title`, `attributes_title` |
| `title` | `preview`, `tree`, `meta`, `file_metadata`, `empty_group`, `empty_dataset`, `error`, `create_attribute`, `delete_attribute`, `fixed_string_overflow`, `fixed_string_resize`, `help`, `matrix_tab` |
| `badge` | `readonly`, `writable`, `linked`, `linked_root_suffix`, `compatibility_mode` |
| `chart` | `membership_marker`, `visibility_visible`, `visibility_hidden`, `enum_1` to `enum_8` |

## Example

```lua
h5v.theme = "dark"
h5v.symbol_theme = "rich"

h5v.colors.accent.selection_bg = "lightblue"
h5v.colors.chart.series_1 = "#ff8800"
h5v.symbols.title.preview = "Plot"
h5v.symbols.tree.dataset_icon = "D"
```

## Heatmap config

| Field | Type | Notes |
| --- | --- | --- |
| `h5v.heatmap.default_range` | string | Preferred starting range preset |
| `h5v.heatmap.default_colormap` | string | `turbo`, `grayscale`, or `inferno` |
| `h5v.heatmap.default_normalization` | string | `linear`, `log`, or `sqrt` |
| `h5v.heatmap.default_invert_x` | boolean | Preferred X-axis inversion |
| `h5v.heatmap.default_invert_y` | boolean | Preferred Y-axis inversion |
| `h5v.heatmap.default_invert_c` | boolean | Preferred colormap inversion |
| `h5v.heatmap.range_modes` | table array | Custom selectable range presets |

Example:

```lua
h5v.content_mode_order = { "heatmap", "preview", "matrix" }
h5v.heatmap.default_range = "MIN/MAX"
h5v.heatmap.default_colormap = "inferno"
h5v.heatmap.default_normalization = "log"
h5v.heatmap.default_invert_y = true
h5v.heatmap.default_invert_c = true
h5v.heatmap.range_modes = {
  { label = "5-80%", min = "5%", max = "80%" },
}
```

## Layout config

| Field | Type | Notes |
| --- | --- | --- |
| `h5v.layout.tree.focused` | integer or string | Tree width/height while tree has focus |
| `h5v.layout.tree.unfocused` | integer or string | Tree width/height while another panel has focus |
| `h5v.layout.attributes.focused` | integer or string | Attributes height while attributes are the active main panel |
| `h5v.layout.attributes.unfocused` | integer or string | Attributes height while content is the active main panel |
| `h5v.layout.content.focused` | integer or string | Content height while content is the active main panel |
| `h5v.layout.content.unfocused` | integer or string | Content height while attributes are the active main panel |

Accepted values:

- integer like `12` for exact terminal rows/columns
- percentage string like `"28%"`
- `"*"` to fill remaining space

If both sides of an attributes/content focus pair use percentages, they must add up to `100%`.

Use the same focused and unfocused value for a panel if you want it to stay fixed across focus changes.

Example:

```lua
h5v.layout.tree.focused = "28%"
h5v.layout.tree.unfocused = "20%"
h5v.layout.attributes.focused = 12
h5v.layout.attributes.unfocused = 5
h5v.layout.content.focused = "*"
h5v.layout.content.unfocused = "*"
```

## Multichart config

| Field | Type | Notes |
| --- | --- | --- |
| `h5v.multichart.overview_max_samples` | integer | Cap for the initial background overview sample |
| `h5v.multichart.detail_enabled` | boolean | Enable viewport-driven detail refinement |
| `h5v.multichart.detail_samples_per_column` | integer | Width multiplier used to pick detail sample count |
| `h5v.multichart.detail_min_samples` | integer | Lower clamp for viewport detail samples |
| `h5v.multichart.detail_max_samples` | integer | Upper clamp for viewport detail samples |
| `h5v.multichart.detail_padding_ratio` | number | Extra x-range loaded around the visible viewport |
| `h5v.multichart.derived_detail_enabled` | boolean | Allow derived series to refine when inputs share detail windows |

Example:

```lua
h5v.multichart = {
  overview_max_samples = 2048,
  detail_enabled = true,
  detail_samples_per_column = 6,
  detail_min_samples = 1024,
  detail_max_samples = 32768,
  detail_padding_ratio = 0.1,
  derived_detail_enabled = true,
}
```

## Keymap config

Scopes:

| Scope | Notes |
| --- | --- |
| `h5v.keymaps.global` | Available in non-text-entry modes regardless of focus |
| `h5v.keymaps.normal` | Normal mode bindings before focus-specific scopes |
| `h5v.keymaps.window` | Follow-up keys after the window chord action |
| `h5v.keymaps.tree` | Tree focus in normal mode |
| `h5v.keymaps.content` | Preview, matrix, and shared content bindings |
| `h5v.keymaps.heatmap` | Heatmap-only bindings |
| `h5v.keymaps.attributes` | Attributes focus in normal mode |
| `h5v.keymaps.mchart` | Multichart navigation/view bindings |

Each scope table accepts:

| Field | Type | Notes |
| --- | --- | --- |
| `clear_defaults` | boolean | Remove the shipped bindings for that scope before applying overrides |
| `unbind` | string array | Remove specific shipped bindings by key |
| `bind` | table array | Add built-in actions or command bindings |

Each `bind` entry accepts:

| Field | Type | Notes |
| --- | --- | --- |
| `key` | string | Key spec such as `ctrl+h`, `PageDown`, `?`, or `ctrl+alt+r` |
| `action` | string | Built-in action id for that scope |
| `command` | string | Command text executed through the normal command parser |
| `commands` | string array | Command list executed like a startup script |
| `script` | string | Startup-script text executed on keypress |
| `lua` | function | Lua callback receiving `ctx.command`, `ctx.commands`, and `ctx.script` helpers |
| `description` | string | Optional help text stored with the binding definition |

An entry must set exactly one of `action`, `command`, `commands`, `script`, or `lua`.

Helper functions:

| Function | Notes |
| --- | --- |
| `bind(mode, key, action[, description])` | Append a built-in action binding using `h5v.modes.*` and `h5v.actions.*` constants |
| `bind_command(mode, key, command[, description])` | Append a command-backed binding |
| `bind_commands(mode, key, commands[, description])` | Append a startup-script-style command list |
| `bind_script(mode, key, script[, description])` | Append one script string using startup-script parsing |
| `bind_lua(mode, key, callback[, description])` | Append a Lua callback |
| `unbind(mode, key)` | Append one key to the scope `unbind` list |

Common mode constants: `h5v.modes.Global`, `h5v.modes.Normal`, `h5v.modes.Window`, `h5v.modes.Tree`, `h5v.modes.Content`, `h5v.modes.Heatmap`, `h5v.modes.Attributes`, `h5v.modes.Multichart`.

Example:

```lua
bind(h5v.modes.Global, "ctrl+h", h5v.actions.ShowHelp)
unbind(h5v.modes.Heatmap, "v")
bind(h5v.modes.Heatmap, "ctrl+z", h5v.actions.HeatmapZoomIn)
bind_command(h5v.modes.Heatmap, "ctrl+alt+r", "heatmap range use \"Clip 1-99%\"")
bind_commands(h5v.modes.Global, "ctrl+k", { "down 2", "up 1" })
bind_lua(h5v.modes.Global, "ctrl+l", function(ctx) ctx.command("help reload") end)
```
