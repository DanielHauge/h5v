# Configuration and theming

h5v loads `init.lua` at startup.

If the file is invalid, h5v keeps running, shows a warning toast, and marks the header with a config warning badge until the configuration loads cleanly again.

## Config file location

`init.lua` lives under your platform config directory:

- Linux: `~/.config/h5v/init.lua`
- macOS: `~/Library/Application Support/h5v/init.lua`
- Windows: `%AppData%\h5v\init.lua`

If it does not exist, `h5v` creates it.

LuaLS sidecar files are generated next to it under `.h5v-luals/`. `h5v.lua` is refreshed automatically. `.luarc.json` is refreshed only while h5v manages it.

## Configuration commands

| Command | What it does |
| --- | --- |
| `configure` | Opens `init.lua` in `$VISUAL` or `$EDITOR`, then reloads the configuration when you exit the editor. |
| `configure reset` | Replaces `init.lua` with a fresh default scaffold, then reloads it immediately. |

## Config flow

1. Resolve compatibility with this precedence: CLI `--compatibility` > `h5v.compatibility` > `H5V_COMPATIBILITY_MODE` > default.
2. Pick content mode order with `h5v.content_mode_order`.
3. Pick a built-in theme with `h5v.theme`.
4. Pick a built-in symbol theme with `h5v.symbol_theme`.
5. Pick preferred heatmap defaults in `h5v.heatmap`.
6. Override only the values you want in `h5v.colors` and `h5v.symbols`.

For all keys, categories, themes, and accepted color names, see [Configuration reference](./configuration-reference.md).

Heatmap also supports preferred defaults in `h5v.heatmap`:

- `default_range`
- `default_colormap`
- `default_normalization`
- `default_invert_x`
- `default_invert_y`
- `default_invert_c`
- `range_modes` for custom presets

## Example

```lua
h5v.theme = "light"
h5v.content_mode_order = { "matrix", "preview" }
h5v.symbol_theme = "compatibility"
h5v.compatibility = true

h5v.heatmap.default_colormap = "inferno"
h5v.heatmap.default_normalization = "log"
h5v.heatmap.default_invert_c = true

h5v.colors.accent.selection_bg = "#e1c878"
h5v.colors.tree.group = "#d26c00"
h5v.colors.surface.panel_border = "#1e3755"

h5v.symbols.tree.dataset_link_icon = "D@"
h5v.symbols.tree.load_more_label = "v Load more"
h5v.symbols.title.matrix_tab = "Matrix"
h5v.symbols.chart.visibility_visible = "*"
```

The built-in light theme is a good starting point for a bright palette plus a few targeted overrides:

![Light theme example](./assets/themes.png)

Use `h5v.themes.<name>` and `h5v.symbol_themes.<name>` as built-in catalogs when you want to copy values from a shipped theme.
