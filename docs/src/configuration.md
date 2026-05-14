# Configuration

h5v loads `init.lua` at startup.

If the file is invalid, h5v keeps running, shows a warning toast, and marks the header with a config warning badge until the configuration loads cleanly again.

## Config file location

`init.lua` lives under your platform config directory:

- Linux: `~/.config/h5v/init.lua`
- macOS: `~/Library/Application Support/h5v/init.lua`
- Windows: `%AppData%\h5v\init.lua`

If it does not exist, `h5v` creates it.

LuaLS sidecar files are generated next to it under `.h5v-luals/`. `h5v.lua` is refreshed automatically. `.luarc.json` is refreshed only while h5v manages it.

## Commands

| Command | What it does |
| --- | --- |
| `configure` | Opens `init.lua` in `$VISUAL` or `$EDITOR`, then reloads the configuration when you exit the editor. |
| `configure reset` | Replaces `init.lua` with a fresh default scaffold, then reloads it immediately. |

## Start here

Most configs only need these fields:

- `h5v.theme`
- `h5v.symbol_theme`
- `h5v.content_mode_order`
- `h5v.compatibility`
- `h5v.heatmap.*`
- `h5v.keymaps`
- `h5v.colors`
- `h5v.symbols`

For all keys, scopes, themes, and accepted color names, see [Configuration reference](./configuration-reference.md).

## Keymaps

Keymaps live in `h5v.keymaps`.

Scopes:

- `global`
- `normal`
- `window`
- `tree`
- `content`
- `heatmap`
- `attributes`
- `mchart`

Precedence:

1. `heatmap`
2. `content` / `tree` / `attributes` / `mchart` / `window`
3. `normal`
4. `global`

Each scope supports:

- `clear_defaults = true` to remove shipped bindings for that scope
- `unbind = { "key", ... }` to remove selected shipped bindings
- `bind = { { key = "...", action = "..." }, { key = "...", command = "..." } }`
- `bind(mode, key, action[, description])`
- `bind_command(mode, key, command[, description])`
- `bind_commands(mode, key, commands[, description])`
- `bind_script(mode, key, script[, description])`
- `bind_lua(mode, key, callback[, description])`
- `unbind(mode, key)`

Use `h5v.modes.*` and `h5v.actions.*` constants in Lua. Examples: `h5v.modes.Global`, `h5v.modes.Heatmap`, `h5v.actions.ShowHelp`, `h5v.actions.HeatmapZoomIn`.

Example:

```lua
bind(h5v.modes.Global, "ctrl+h", h5v.actions.ShowHelp, "Show help")
unbind(h5v.modes.Heatmap, "v")
bind_command(h5v.modes.Heatmap, "ctrl+alt+r", "heatmap range use \"Clip 1-99%\"")
bind_commands(h5v.modes.Global, "ctrl+k", { "down 2", "up 1" })
bind_script(h5v.modes.Global, "ctrl+s", "down 2\nup 1")
bind_lua(h5v.modes.Global, "ctrl+l", function(ctx)
  ctx.command("help reload")
end)
bind(h5v.modes.Heatmap, "ctrl+z", h5v.actions.HeatmapZoomIn)
```

## Heatmap defaults

Heatmap settings live under `h5v.heatmap`:

- `default_range`
- `default_colormap`
- `default_normalization`
- `default_invert_x`
- `default_invert_y`
- `default_invert_c`
- `range_modes`

## Example

```lua
h5v.theme = "light"
h5v.content_mode_order = { "matrix", "preview" }
h5v.symbol_theme = "compatibility"
h5v.compatibility = true

h5v.heatmap.default_colormap = "inferno"
h5v.heatmap.default_normalization = "log"
h5v.heatmap.default_invert_c = true

bind(h5v.modes.Global, "ctrl+h", h5v.actions.ShowHelp)

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
