# Navigation and layout

## Pane model

h5v has three main focus targets:

- the tree pane for HDF5 structure and selection
- the content pane for preview, matrix, schema, image, and multichart views
- the attributes pane for metadata inspection and editing

On wider terminals, the tree sits on the left and the content plus attributes live on the right. On narrower terminals, the layout stacks vertically. Search mode temporarily takes over the full working area so the search results are easier to scan.

## Moving focus

Use either style:

- `Shift` + arrow keys to move between panes directly
- `Ctrl+W`, then `h/j/k/l` for vim-style pane navigation

The sidebar can be toggled with `s` or `Ctrl+W o`.

## Tree navigation

The tree is the source of truth for what the rest of the UI shows.

- `j` / `k` or arrow keys move the selection
- `h` and `l` collapse or expand nodes
- `Enter` or `Space` toggles the current node
- `g` / `G` jump to the top or bottom
- `Ctrl+U` / `Ctrl+D` move by larger chunks

When the current selection is previewable, `m` adds it to the multichart workspace immediately.

Mouse support follows the same model:

- a click selects the row under the cursor
- clicking the already selected group or compound container toggles it
- repeated clicks on `Load more` keep expanding long child lists

## Content modes

The content area changes based on the selected node:

- numeric datasets prefer chart-style preview
- matrixable datasets can switch to matrix mode
- scalar and string data render as text
- HDF5 image datasets render inline as images
- compound container nodes show a recursive schema view
- file nodes show filesystem metadata
- groups stay previewable even when they only show an empty-state card

Use `Tab` to cycle between content modes when more than one representation is available.

## Search and help

- `/` enters search mode
- `:` opens the command minibuffer
- `?` opens the built-in help overlay
- `.` repeats the last successful command

The help overlay is worth using early; it mirrors the actual input model and command syntax exposed by the application.
