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

h5v.events.on({
  event = h5v.ids.events.file_opened,
  run = function(ctx)
    ctx.log.info("opened " .. ctx.event.path)
  end,
})

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

Plugins can also use:

- `ctx.process.run(...)`
- `ctx.process.parse_json(...)`
- `ctx.toast.*(...)`
- `ctx.log.*(...)`
- `ctx.ui.build(...)`

## Health and logs

- Plugin health is shown in `Help -> Health`.
- Plugin logs are tagged with the plugin handle and show up in `:logs`.
- If `health` or `init` fails, the plugin is marked unhealthy instead of becoming an `h5v` health issue.
