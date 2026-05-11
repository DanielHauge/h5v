# Command reference

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
| `mode` | `view-mode` | `<mode>` | `Tab` | Switch between preview and matrix |
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
