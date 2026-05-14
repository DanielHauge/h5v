# Command reference

Key columns below show the shipped defaults. `h5v.keymaps` can override them for non-text-entry contexts.

## Top-level commands

| Command | Aliases | Args | Keys | Purpose |
| --- | --- | --- | --- | --- |
| `seek` | — | `<index>` | — | Jump to an absolute index in the current content view |
| `goto` | `jump`, `open` | `<path>` | — | Select a node by HDF5 path |
| `up` | `dec`, `decrement` | `[amount]` | `Up`, `k` | Move up by a relative amount |
| `down` | `inc`, `increment` | `[amount]` | `Down`, `j` | Move down by a relative amount |
| `left` | — | `[amount]` | `Left`, `h` | Move left by a relative amount |
| `right` | — | `[amount]` | `Right`, `l` | Move right by a relative amount |
| `page-up` | `pgup` | — | `PageUp`, `Ctrl+U` | Move up by a page |
| `page-down` | `pgdown` | — | `PageDown`, `Ctrl+D` | Move down by a page |
| `focus` | — | `<target>` | `Shift+Arrows` | Focus a pane |
| `mode` | `view-mode` | `<mode>` | `Tab` | Switch between preview, matrix, and heatmap |
| `toggle-tree` | `tree` | — | `s` | Show or hide the tree pane |
| `reload` | `refresh` | — | `Ctrl+R` | Reload the current file |
| `configure` | `config` | `[reset]` | — | Open or reset the Lua config |
| `x` | — | `<prev|next>` | `x`, `X` | Move the preview x-axis selection |
| `row` | — | `<prev|next>` | `r`, `R` | Move the matrix row-axis selection |
| `col` | `column` | `<prev|next>` | `c`, `C` | Move the matrix column-axis selection |
| `dim` | `dimension` | `<prev|next>` | `[`, `]` | Move the selected dimension cursor |
| `index` | `selected-index` | `<prev|next> [amount]` | `Ctrl+A`, `Ctrl+X`, `Alt+Up/Down` | Move the selected index |
| `help` | `?` | `[command]` | `?` | Open help or show help for one command |
| `attr` | `attribute` | `<create|delete> ...` | `a`, `d`, `Delete` | Create or delete scalar attributes |
| `repeat` | `again` | — | `.` | Repeat the last successful command |
| `mchart` | `multichart` | `<action> ...` | `M` | Control multichart |
| `press` | `key`, `keys` | `<key1> [key2] [key3] [key4]` | — | Dispatch key presses through the normal keymap |
| `heatmap` | — | `<action> ...` | — | Manage heatmap-specific range presets |

## Numeric shorthand

| Input | Expands to |
| --- | --- |
| `:5` | `seek 5` |
| `:+3` | `down 3` |
| `:-2` | `up 2` |

## Focus targets

| Target |
| --- |
| `tree` |
| `content` |
| `attributes` |

## View modes

| Mode |
| --- |
| `preview` |
| `matrix` |
| `heatmap` |

## Heatmap view command mapping

Heatmap uses the existing movement commands:

| Command | Effect in heatmap |
| --- | --- |
| `up [amount]` | Move the selected heatmap setting row |
| `down [amount]` | Move the selected heatmap setting row |
| `left [amount]` | Change the selected heatmap setting value |
| `right [amount]` | Change the selected heatmap setting value |
| `page-up` | Move to the previous segmented heatmap page |
| `page-down` | Move to the next segmented heatmap page |

## Heatmap commands

| Command | Effect |
| --- | --- |
| `heatmap range list` | Show all built-in, configured, and session-added range presets |
| `heatmap range use <preset>` | Select an existing range preset by label or built-in alias |
| `heatmap range add <min> <max> [label]` | Add a session range preset and select it immediately |

## Heatmap key-only actions

These actions are available from the keymap and can be scripted with `press ...`:

| Key | Script form | Effect |
| --- | --- | --- |
| `z` | `press z` | Zoom in |
| `Z` | `press Z` | Zoom out |
| `0` | `press 0` | Reset the viewport |
| `v` | `press v` | Clear the explicit selection |
| `H` / `J` / `K` / `L` | `press H` / `press J` / `press K` / `press L` | Pan the zoomed viewport |

## Selection directions

| Direction |
| --- |
| `prev` |
| `next` |

## Attribute actions

| Action | Form |
| --- | --- |
| `create` | `attr create <name> <type> <value>` |
| `delete` | `attr delete <name>` |

## Multichart actions

| Action | Example |
| --- | --- |
| open/show/close/hide/toggle | `mchart open` |
| add | `mchart add !/signals/sine_wave` |
| expr/expression/prompt | `mchart expr "($1, !/signals/cosine_wave)"` |
| base toggle / clear | `mchart base toggle` |
| derive | `mchart derive difference` |
| select / move | `mchart select next` |
| visible toggle / show / hide | `mchart visible hide` |
| remove / delete | `mchart delete` |
| clear / clear all / clear zoom | `mchart clear zoom` |
| zoom in / out / reset | `mchart zoom in 25` |
| pan left / right | `mchart pan right 10` |

## Press examples

```text
press ctrl+w o
press M j enter
```

`press` uses the effective keymaps after config load, including custom keymap overrides.
