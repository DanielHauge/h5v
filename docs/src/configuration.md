# Configuration

`h5v` loads one Lua file: `init.lua`.

## File location

- Linux: `~/.config/h5v/init.lua`
- macOS: `~/Library/Application Support/h5v/init.lua`
- Windows: `%AppData%\\h5v\\init.lua`

Commands:

```text
:configure
:configure reset
h5v --config /path/to/init.lua file.h5
```

LuaLS support files live next to the config under `.h5v-luals/`.

## Start with this shape

```lua
h5v.theme = "dark"
h5v.symbol_theme = "rich"
h5v.compatibility = false
h5v.content_mode_order = { "preview", "matrix" }

h5v.heatmap.default_colormap = "inferno"
h5v.layout.tree.focused = "28%"

h5v.colors.accent.selection_bg = "#005f87"
h5v.symbols.title.help = " Help "

h5v.keys.bind({
  mode = h5v.ids.keymap_modes.global,
  key = "ctrl+h",
  target = h5v.actions.ShowHelp,
  description = "Show help",
})
```

## Main pieces

| Lua entry | Use it for |
| --- | --- |
| `h5v.theme`, `h5v.symbol_theme` | shipped themes |
| `h5v.colors.*`, `h5v.symbols.*` | targeted overrides |
| `h5v.content_mode_order` | preferred preview/matrix/heatmap order |
| `h5v.compatibility` | compatibility mode default |
| `h5v.layout.*` | tree / attributes / content sizing |
| `h5v.heatmap.*` | heatmap defaults and custom ranges |
| `h5v.multichart.*` | multichart sampling defaults |
| `h5v.keys.*` | keybindings |
| `h5v.commands.register(...)` | custom commands |
| `h5v.events.on(...)` | autocommands |
| `h5v.mchart.functions.register(...)` | custom multichart functions |
| `h5v.plugins.use(...)` | plugins |
| `h5v.logs.*` | log from Lua |

## Use constants, not magic strings

Prefer the generated constants and action ids:

```lua
h5v.keys.bind({
  mode = h5v.ids.keymap_modes.global,
  key = "ctrl+l",
  target = h5v.actions.ReloadFile,
})
```

Use the in-app help for the current command list, action names, keymaps, multichart functions, and health details.

## Examples

### Keybinding

```lua
h5v.keys.bind({
  mode = h5v.ids.keymap_modes.heatmap,
  key = "ctrl+alt+r",
  command = "heatmap range use \"Clip 1-99%\"",
  description = "Use clipped range",
})
```

### Custom command

```lua
h5v.commands.register({
  id = "analysis.refresh",
  title = "Refresh analysis",
  summary = "Refresh plugin output",
  run = function(ctx)
    ctx.toast.info("refreshing")
    ctx.command("logs")
  end,
})
```

### Event hook

```lua
h5v.events.on({
  event = h5v.ids.events.file_opened,
  run = function(ctx)
    ctx.log.info("file opened: " .. ctx.event.path)
  end,
})
```

### Plugin from config

```lua
h5v.plugins.use("~/dev/h5v-demo-plugin")
h5v.plugins.use("owner/example-plugin")
h5v.plugins.use("owner/example-plugin@main")
h5v.plugins.use("owner/example-plugin", { auto_pull = false })
```

## Health and logs

- Bad config loads show up in the header and in `Help -> Health`.
- Plugin problems show up on the plugin health page when the plugin has a valid manifest.
- `:logs` opens the log panel.

For plugin authoring, see [Plugins](./plugins.md).
