# Navigation and layout

## Pane model

h5v has three panes:

- the tree pane for HDF5 structure and selection
- the content pane for preview, matrix, schema, image, and multichart views
- the attributes pane for metadata inspection and editing

Wide terminals use a side-by-side layout. Narrow terminals stack the panes. Search takes over the working area.

## Moving focus

- `Shift` + arrow keys to move between panes directly
- `Ctrl+W`, then `h/j/k/l` for vim-style pane navigation

The sidebar can be toggled with `s` or `Ctrl+W o`.

## Tree navigation

- `j` / `k` or arrow keys move the selection
- `h` and `l` collapse or expand nodes
- `Enter` or `Space` toggles the current node
- `g` / `G` jump to the top or bottom
- `u` / `Ctrl+U` and `Ctrl+D` move by larger chunks

When the current selection is previewable, `m` adds it to the multichart workspace.

Mouse support:

- a click selects the row under the cursor
- clicking the already selected group or compound container toggles it
- repeated clicks on `Load more` keep expanding long child lists

## Content modes

The content pane changes with the selected node:

- numeric datasets prefer chart-style preview
- matrixable datasets can switch to matrix mode
- scalar and string data render as text
- HDF5 image datasets render inline as images
- compound container nodes show a recursive schema view
- file nodes show filesystem metadata
- groups stay previewable even when they only show an empty-state card

Use `Tab` to switch modes when more than one is available.

## Search and help

- `/` enters search mode
- `:` opens the command minibuffer
- `?` opens the built-in help overlay
- `.` repeats the last successful command

See [Controls reference](./controls.md) and [Commands](./commands.md).
