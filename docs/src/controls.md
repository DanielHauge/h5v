# Controls reference

These are the shipped defaults. `h5v.keymaps` can override them for non-text-entry contexts.

## Global and pane controls

| Keys | Action |
| --- | --- |
| `q`, `Ctrl+C` | Quit |
| `?` | Show help |
| `:` | Open command mode |
| `.` | Repeat the last successful command |
| `/` | Enter search |
| `Shift` + arrows | Move focus between panes |
| `Ctrl+W`, then `h/j/k/l` | Vim-style pane focus |
| `s` or `Ctrl+W o` | Toggle the sidebar |
| `Ctrl+R` | Reload the file |

## Help

| Keys | Action |
| --- | --- |
| `?` | Open help |
| `Esc` | Close help |
| `Tab`, `l`, `Right` | Next tab |
| `Shift+Tab`, `h`, `Left` | Previous tab |
| `j`, `Down` | Next item in the left-hand list |
| `k`, `Up` | Previous item in the left-hand list |
| `g`, `Home` | Jump to the first item in the left-hand list |
| `G`, `End` | Jump to the last item in the left-hand list |

## Tree navigation

| Keys | Action |
| --- | --- |
| `j`, `Down` | Move down |
| `k`, `Up` | Move up |
| `Ctrl+D` | Move down by a larger step |
| `u`, `Ctrl+U` | Move up by a larger step |
| `g`, `Home` | Jump to the top |
| `G`, `End` | Jump to the bottom |
| `h`, `Left` | Collapse |
| `l`, `Right` | Expand |
| `Enter`, `Space` | Toggle node |
| `m` | Add the current previewable selection to multichart, including group previews |

## Preview and matrix selectors

| Keys | Action |
| --- | --- |
| `Tab` | Toggle preview and matrix when both exist |
| `x`, `X` | Move the preview x-axis |
| `r`, `R` | Move the matrix row axis |
| `c`, `C` | Move the matrix column axis |
| `[`, `]` | Change the selected dimension |
| `Alt+Left`, `Alt+Right` | Previous or next selected dimension |
| `Ctrl+X`, `Ctrl+A` | Decrement or increment the selected index |
| `Alt+Up`, `Alt+Down` | Increment or decrement the selected index |
| `PageUp`, `PageDown` | Scroll larger preview or matrix segments |

## Content and attributes

| Keys | Action |
| --- | --- |
| `h/j/k/l`, arrows | Move inside the active pane |
| `Enter`, `e` | Edit the focused value or attribute |
| `y` | Copy the focused name or value |
| `a` | Create an attribute from the attributes pane |
| `d`, `Delete` | Delete the focused attribute or multichart item depending on mode |
| `Esc` | Cancel the active popup or prompt |

Edits use your configured editor through `$EDITOR` and fall back to `vi`.

## Heatmap

| Keys | Action |
| --- | --- |
| `Up`, `Down` | Select heatmap setting row |
| `Left`, `Right` | Change the selected heatmap setting |
| `PageUp`, `PageDown` | Move through segmented heatmap pages |
| `z`, `Z` | Zoom in or out |
| `0` | Reset the heatmap viewport |
| `v` | Clear the explicit heatmap selection |
| `H`, `J`, `K`, `L` | Pan the zoomed viewport |
| `y` | Copy the active viewport or selection summary |

## Mouse

| Action | Effect |
| --- | --- |
| Click a tree row | Select it |
| Click the selected group or compound container again | Toggle expand or collapse |
| Click `Load more` again | Reveal more child rows |
| Click a matrix cell | Move the matrix cursor there |
| Click a heatmap settings row | Focus that heatmap setting |
| Left click a heatmap cell | Select a heatmap region |
| Mouse wheel over heatmap | Anchored zoom on the hovered cell |
| Right click on a heatmap selection | Zoom into the selected region |
| Right-click drag on heatmap or multichart | Pan |

## Command minibuffer

| Keys | Action |
| --- | --- |
| `Enter` | Run the command |
| `Esc` | Cancel |
| `Tab` | Apply the next completion |
| `Shift+Tab`, `Up` | Previous suggestion |
| `Down` | Next suggestion |
| `Ctrl+P`, `Ctrl+N` | Browse command history |
| `Ctrl+W` | Delete the previous word |
| `Ctrl+A`, `Home` | Move to the start |
| `Ctrl+E`, `End` | Move to the end |
| `Ctrl+U` | Clear the line |

## Multichart mode

| Keys | Action |
| --- | --- |
| `M`, `Esc` | Leave multichart mode |
| `j`, `k` | Move through chart items |
| `Enter` | Open a new expression |
| `e` | Edit the selected series |
| `Space`, `v` | Toggle selected item visibility |
| `?` | Open multichart help |
| `d`, `Backspace`, `Delete` | Remove the selected item when nothing depends on it |
| `C` | Clear all chart items |
| `c` | Reset zoom |
| `+`, `=`, `Shift+Up` | Zoom in |
| `-`, `Shift+Down` | Zoom out |
| `h`, `Shift+Left` | Pan left |
| `l`, `Shift+Right` | Pan right |
| Right-click drag | Pan horizontally |
