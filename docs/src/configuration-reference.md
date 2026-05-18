# Configuration reference

Use [Configuration](./configuration.md) for the normal setup flow. Use this page when you need the exact entry points.

## Top-level config fields

| Field | Type |
| --- | --- |
| `h5v.theme` | string |
| `h5v.symbol_theme` | string |
| `h5v.compatibility` | boolean |
| `h5v.content_mode_order` | string[] |
| `h5v.layout.*` | integer, `"NN%"`, or `"*"` |
| `h5v.heatmap.*` | table |
| `h5v.multichart.*` | table |
| `h5v.colors.*` | string |
| `h5v.symbols.*` | string |

## Helper namespaces

| Namespace | What it does |
| --- | --- |
| `h5v.keys` | bind and unbind keymaps |
| `h5v.commands` | register commands |
| `h5v.events` | register event handlers |
| `h5v.mchart.functions` | register multichart functions |
| `h5v.plugins` | load plugins |
| `h5v.logs` / `h5v.log` | write logs from Lua |
| `h5v.ids.*` | generated ids and constants |
| `h5v.actions.*` | generated built-in action ids |

## Keymaps

```lua
h5v.keys.bind({
  mode = h5v.ids.keymap_modes.global,
  key = "ctrl+h",
  target = h5v.actions.ShowHelp,
  description = "Show help",
})

h5v.keys.bind({
  mode = h5v.ids.keymap_modes.global,
  key = "ctrl+l",
  command = "logs",
  description = "Open logs",
})

h5v.keys.unbind({
  mode = h5v.ids.keymap_modes.heatmap,
  key = "v",
})
```

## Commands

```lua
h5v.commands.register({
  id = "analysis.refresh",
  title = "Refresh analysis",
  summary = "Refresh the current analysis",
  run = function(ctx)
    ctx.commands({
      "logs",
      "help health",
    })
  end,
})
```

## Events

```lua
h5v.events.on(h5v.ids.events.file_opened, function(ctx, event)
  ctx.log.info("opened " .. event.path)
})
```

## Multichart functions

```lua
h5v.mchart.functions.register({
  id = "analysis.scale",
  name = "scale",
  params = {
    { name = "series", kind = h5v.ids.value_kinds.series },
    { name = "factor", kind = h5v.ids.value_kinds.scalar },
  },
  returns = h5v.ids.value_kinds.series,
  eval = function(series, factor)
    return series * factor
  end,
})
```

## Plugins

```lua
h5v.plugins.use("~/dev/h5v-demo-plugin")
h5v.plugins.use("owner/example-plugin")
h5v.plugins.use("owner/example-plugin@main")
h5v.plugins.use("https://github.com/owner/example-plugin.git@v0.1.0")
```

`auto_pull` defaults to `true` for git sources:

```lua
h5v.plugins.use("owner/example-plugin", { auto_pull = false })
```

## Runtime helpers available in Lua callbacks

Common callback helpers:

```lua
ctx.command("logs")
ctx.commands({ "goto /signals/sine_wave", "mode preview" })
ctx.toast.info("done")
ctx.log.warning("slow path")
ctx.process.run({ command = { "git", "status" } })
ctx.process.parse_json("{\"ok\":true}")
```

## Healthcheck result shape

```lua
return {
  status = ctx.health.healthy,
  summary = "ready",
  message = ctx.ui.build(function(ui)
    ui.block({ title = "Plugin health" }, function(ui)
      ui.text("🟢 successfully loaded plugin")
    end)
  end),
}
```

`message` may be a plain string or a built UI document. `summary` is the short text used when the detailed message is structured UI.

## UI builder quick reference

```lua
ctx.ui.build(function(ui)
  ui.text("plain text")
  ui.code("return 1 + 2", "lua")
  ui.badge("ok")
  ui.kv("theme", h5v.theme)
  ui.separator({ label = "details" })
  ui.split({ direction = "horizontal", ratio = 0.4 }, function(ui)
    ui.text("left")
  end, function(ui)
    ui.text("right")
  end)
  ui.table({
    { "key", "value" },
    { "theme", h5v.theme },
  })
  ui.block({ title = "status" }, function(ui)
    ui.text("ready")
  end)
end)
```

## Built-in themes

- `dark`
- `light`

## Built-in symbol themes

- `rich`
- `compatibility`

## Heatmap values

- `default_colormap`: `turbo`, `grayscale`, `inferno`
- `default_normalization`: `linear`, `log`, `sqrt`

## Accepted color values

- `#RRGGBB`
- named colors such as `red`, `green`, `yellow`, `blue`, `magenta`, `cyan`, `gray`, `white`
- extra names: `amber`, `orange`

## Color groups

- `accent`
- `text`
- `content`
- `command`
- `help`
- `metadata`
- `file`
- `mchart`
- `surface`
- `tree`
- `chart`
- `status`
- `toast`

## Symbol groups

- `tree`
- `section`
- `title`
- `badge`
- `chart`
