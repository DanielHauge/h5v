# Configuration reference

## Core config fields

| Field | Type | Notes |
| --- | --- | --- |
| `h5v.theme` | string | Built-in color theme name |
| `h5v.symbol_theme` | string | Built-in symbol theme name |
| `h5v.compatibility` | boolean | Compatibility mode override from config |
| `h5v.content_mode_order` | string array | Ordered content-mode preference |
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
| `light_blue` |

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
