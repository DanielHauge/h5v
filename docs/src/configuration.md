# Configuration and theming

h5v loads `init.lua` at startup.

## Config file location

`init.lua` lives under your platform config directory:

- Linux: `~/.config/h5v/init.lua`
- macOS: `~/Library/Application Support/h5v/init.lua`
- Windows: `%AppData%\h5v\init.lua`

If it does not exist, `h5v` creates it.

LuaLS sidecar files are generated next to it under `.h5v-luals/`. `h5v.lua` is refreshed automatically. `.luarc.json` is refreshed only while it is still h5v-managed.

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
5. Override only the values you want in `h5v.colors` and `h5v.symbols`.

For all keys, categories, themes, and accepted color names, see [Configuration reference](./configuration-reference.md).

## Example

```lua
h5v.theme = "light"
h5v.content_mode_order = { "matrix", "preview" }
h5v.symbol_theme = "compatibility"
h5v.compatibility = true

h5v.colors.accent.selection_bg = "#e1c878"
h5v.colors.tree.group = "#d26c00"
h5v.colors.surface.panel_border = "#1e3755"

h5v.symbols.tree.dataset_link_icon = "D@"
h5v.symbols.tree.load_more_label = "v Load more"
h5v.symbols.title.matrix_tab = "Matrix"
h5v.symbols.chart.visibility_visible = "*"
```

The built-in light theme is a good starting point when you want a bright palette and then layer a few targeted overrides on top:

![Light theme example](./assets/themes.png)

Use `h5v.themes.<name>` and `h5v.symbol_themes.<name>` as built-in catalogs when you want to copy values from a shipped theme.
