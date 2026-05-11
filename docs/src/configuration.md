# Configuration and theming

`h5v` can load a Lua configuration file at startup. Use it to pick a default theme, override individual colors, and script small startup helpers.

## Config file location

The config file is stored as `init.lua` inside your platform config directory under `h5v`:

- Linux: `~/.config/h5v/init.lua`
- macOS: `~/Library/Application Support/h5v/init.lua`
- Windows: `%AppData%\h5v\init.lua`

If the file does not exist yet, `h5v` can generate a default scaffold for you.

## Configuration commands

Open the command minibuffer with `:` and use:

| Command | What it does |
| --- | --- |
| `configure` | Opens `init.lua` in `$VISUAL` or `$EDITOR`, then reloads the configuration when you exit the editor. |
| `configure reset` | Replaces `init.lua` with a fresh default scaffold, then reloads it immediately. |

These commands are useful when tuning themes interactively because you can edit, save, and jump straight back into the current session.

## Lua API surface

The config file gets an `h5v` table with a few built-ins:

| Field | Purpose |
| --- | --- |
| `h5v.theme` | Selects the base built-in color theme: `"dark"`, `"light"`, or `"light_blue"`. |
| `h5v.compatibility` | Sets compatibility mode from config when the CLI flag is not present. |
| `h5v.symbol_theme` | Selects the base built-in symbol theme: `"rich"` or `"compatibility"`. |
| `h5v.colors` | Overrides individual theme colors. |
| `h5v.symbols` | Overrides individual UI symbols, labels, and decorated titles. |
| `h5v.themes.dark` | Exposes the built-in dark theme colors as a nested table. |
| `h5v.themes.light` | Exposes the built-in light theme colors as a nested table. |
| `h5v.themes.light_blue` | Exposes the built-in light-blue theme colors as a nested table. |
| `h5v.symbol_themes.rich` | Exposes the built-in rich symbol set as a nested table. |
| `h5v.symbol_themes.compatibility` | Exposes the built-in ASCII-safe symbol set as a nested table. |
| `h5v.log("message")` | Sends a small informational toast while the config runs. |

## Theme model

The theme model is layered:

1. Resolve compatibility with this precedence: CLI `--compatibility` > `h5v.compatibility` > `H5V_COMPATIBILITY_MODE` > default.
2. Pick a built-in theme with `h5v.theme`.
3. Pick a built-in symbol theme with `h5v.symbol_theme`.
4. Override only the colors and symbols you want in `h5v.colors` and `h5v.symbols`.

That means you do not need to redefine the entire palette or symbol set just to change a few values.

## Color categories

Colors are grouped by purpose:

| Category | Covers |
| --- | --- |
| `accent` | Selection, highlight, symbol, and search accent colors |
| `text` | Primary value rendering, search text, line numbers, and type-description text |
| `content` | App header text, empty states, content tabs, and tree membership overflow text |
| `command` | Command prompt usage, hints, descriptions, and suggestion labels |
| `help` | Help overlay headings, descriptions, and muted separators |
| `metadata` | Metadata section headers plus property and attribute labels/values |
| `file` | File preview labels, values, and subsection titles |
| `mchart` | Multi-chart workspace empty states, list rows, detail labels, and prompt prefix |
| `surface` | Backgrounds, borders, title bars, highlight backgrounds, and image borders |
| `tree` | Tree lines and HDF5 node colors such as files, groups, datasets, and compounds |
| `chart` | Axes, grids, plot background, line series, and enum series colors |
| `status` | Read-only/write/link/update status badges |
| `toast` | Toast border colors |

## Symbol categories

Symbols are grouped the same way:

| Category | Covers |
| --- | --- |
| `tree` | Connectors, guides, expand/collapse arrows, node icons, and load-more label |
| `section` | Decorated metadata section titles such as Properties and Attributes |
| `title` | Panel titles, dialog titles, and tab labels |
| `badge` | Header badges, linked markers, linked root suffix, and compatibility badge |
| `chart` | Membership markers, visibility markers, and enum markers |

## Example

```lua
h5v.theme = "light"
h5v.compatibility = true
h5v.symbol_theme = "compatibility"

h5v.colors = {
  text = {
    primary = "#050508",
    type_desc = "#5a6670",
  },
  content = {
    app_brand = "#402400",
    tab_inactive = "#5a6670",
  },
  metadata = {
    section = "#314f7a",
    property_name = "#1e3755",
  },
  surface = {
    title_bg = "#d2ccc2",
    panel_border = "#1e3755",
  },
  tree = {
    group = "#d26c00",
    dataset_file = "#008e58",
  },
  accent = {
    symbol = "#5a3200",
    selection_bg = "#e1c878",
  },
}

h5v.symbols = {
  tree = {
    dataset_link_icon = "D@",
    load_more_label = "v Load more",
  },
  title = {
    tree = " Tree ",
    matrix_tab = "Matrix",
  },
  badge = {
    linked_root_suffix = " ({count}) linked ",
  },
  chart = {
    visibility_visible = "*",
    visibility_hidden = "o",
  },
}
```
