# Plugins

Plugins are normal Lua modules with a small manifest.

## Scaffold

```bash
h5v --init-plugin path/to/my-plugin
```

This creates:

- `h5v-plugin.toml`
- `lua/init.lua`
- `.luarc.json`
- `.h5v-luals/h5v.lua`

Overwrite rules:

- `.luarc.json` is **always rewritten**
- `.h5v-luals/h5v.lua` is **always rewritten**
- `h5v-plugin.toml` is created **only if missing**
- `lua/init.lua` is created **only if missing**

That means you can rerun `--init-plugin` to refresh LuaLS support without overwriting the plugin manifest or your Lua code.

## Load a plugin

From `init.lua`:

```lua
h5v.plugins.use("~/dev/my-plugin")
h5v.plugins.use("owner/example-plugin")
h5v.plugins.use("owner/example-plugin@main")
h5v.plugins.use("https://github.com/owner/example-plugin.git@v0.1.0")
```

Git sources support:

```lua
h5v.plugins.use("owner/example-plugin", { auto_pull = false })
```

Local paths do not support `@ref`.

The repository includes a full demo plugin at `h5v-demo/`. Use it as the end-to-end example for health checks, commands, keymaps, autocommands, custom content modes, themes, plugin store usage, and custom multichart functions.

## Manifest

```toml
id = "demo.analysis"
name = "Demo analysis"
version = "0.1.0"
api_version = "2"
entry = "lua/init.lua"
```

If the manifest cannot be resolved, that is an `h5v` health issue. Once the manifest is valid, the plugin is modeled as a plugin and any later load/health/init failure belongs to that plugin.

## Module contract

```lua
---@type H5vPluginModule
return {
  health = function(ctx)
    return {
      status = ctx.health.healthy,
      summary = "ready",
      message = ctx.ui.build(function(ui)
        ui.text("🟢 successfully loaded plugin")
      end),
    }
  end,

  init = function(h5v, ctx)
    ctx.toast.info("success :D")
  end,
}
```

## What plugins can register

Examples:

```lua
h5v.commands.register({
  id = "analysis.refresh",
  title = "Refresh analysis",
  summary = "Refresh plugin output",
  run = function(ctx)
    ctx.toast.info("refreshing")
  end,
})

h5v.events.on(h5v.ids.events.file_opened, function(ctx, event)
  ctx.log.info("opened " .. event.path)
end)

h5v.ui.content_modes.register({
  id = "analysis.results",
  title = "Analysis",
  render = function(ctx, ui)
    ui.block({ title = "Analysis" }, function(ui)
      ui.kv("selected", ctx.selection.path or "/")
    end)
  end,
})

h5v.themes.register({
  id = "analysis.theme",
  title = "Analysis theme",
  variant = "dark",
  colors = {
    [h5v.ids.colors.content.app_brand] = "#7dd3fc",
  },
})

h5v.events.on(h5v.ids.events.file_opened, function(ctx, event)
  local exe = ctx.process.command_path("h5v") or "h5v"
  local help = ctx.process.run({ command = { exe, "--help" } })
  if help.success then
    ctx.plugin.store.set("help_text", help.stdout or "")
  else
    ctx.log.warning("failed to capture help for " .. event.path)
  end
end)

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

Built-in events cover file lifecycle plus runtime transitions such as selection changes, mode/focus changes, tree visibility changes, and help/logs/search/command/mchart open-close hooks. Use `h5v.ids.events.*` and the in-app help for the current event list.

## Plugin contexts

Plugins can use:

- `ctx.process.run(...)`
- `ctx.process.spawn(...)`
- `ctx.process.parse_json(...)`
- `ctx.process.command_path(...)`
- `ctx.toast.*(...)`
- `ctx.log.*(...)`
- `ctx.ui.build(...)`
- `ctx.plugin.store.get/set/delete(...)`

Selection-aware command, keymap, event, and content-mode callbacks also receive:

```lua
ctx.selection.path
ctx.selection.kind
ctx.selection.attribute_names
ctx.selection.has_attribute("ATTR_NAME")
```

## Health and logs

- Plugin health is shown in `Help -> Health`.
- Plugin logs are tagged with the plugin handle and show up in `:logs`.
- If `health` or `init` fails, the plugin is marked unhealthy instead of becoming an `h5v` health issue.
