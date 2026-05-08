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
| `h5v.theme` | Selects the base built-in theme: `"dark"` or `"light"`. |
| `h5v.colors` | Overrides individual theme colors. |
| `h5v.themes.dark` | Exposes the built-in dark theme colors as a nested table. |
| `h5v.themes.light` | Exposes the built-in light theme colors as a nested table. |
| `h5v.log("message")` | Sends a small informational toast while the config runs. |

## Theme model

The theme model is layered:

1. Pick a built-in theme with `h5v.theme`.
2. Override only the colors you want in `h5v.colors`.

That means you do not need to redefine the entire palette just to change a few values.

## Color categories

Colors are grouped by purpose:

| Category | Covers |
| --- | --- |
| `accent` | Selection, highlight, symbol, and search accent colors |
| `text` | Titles, primary text, values, line numbers, and minibuffer text |
| `surface` | Backgrounds, borders, title bars, highlight backgrounds, and image borders |
| `tree` | Tree lines and HDF5 node colors such as files, groups, datasets, and compounds |
| `chart` | Axes, grids, plot background, line series, and enum series colors |
| `status` | Read-only/write status colors and toast colors |

## Example

```lua
h5v.theme = "light"

h5v.colors = {
  text = {
    title = "#402400",
    primary = "#050508",
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
```
