# h5v

HDF5 Terminal Viewer.

It is a viewer for HDF5 files, allowing you to explore the contents of HDF5 files in a terminal with chart,string, matrix and image previews of the data including attributes.

Run `h5v` with the path to an HDF5 file:

```bash
h5v path/to/file.h5
```

![](./docs/chart_example.png)
![](./docs/image_example.png)
![](./docs/matrix_example.png)

## Controls

- `j`/`k`/`up`/`down`: Navigate lists and move selections
- `h`/`l`/`left`/`right`: Collapse/expand tree items and move within content
- `enter`/`space`: Toggle tree items
- `shift` + `arrow`: Shift focus between panes
- `ctrl` + `w`, then `h`/`j`/`k`/`l`: Move focus vim-style between panes
- `s` or `ctrl` + `w`, then `o`: Toggle the tree/attribute sidebar
- `q` / `ctrl`+`c`: Quit
- `y`: Copy highlighted to clipboard
- `ctrl` + navigate: Scroll through contents (image list or matrix)
- `PgUp`/`PgDown`: Scroll through contents by half a page (image list or matrix)
- `ctrl` + `d`/`u`: Navigate the tree by half a page
- `[` / `]`: Change the selected dimension in preview and matrix modes
- `ctrl` + `x` / `a`: Decrement or increment the selected index in preview and matrix modes
- `alt` + `left`/`right`: Alias for changing selected dimension
- `alt` + `up`/`down`: Alias for incrementing or decrementing selected index
- `c`/`C`: Shift column axis in matrix mode.
- `r`/`R`: Shift row axis in matrix mode.
- `x`/`X`: Shift x-axis selector in preview mode.
- `g`/`Home`: Go to the top
- `G`/`End`: Go to the bottom
- `m`: Add currently selected preview to multichart
- `M`: Toggle multichart mode
- `ctrl` + `r`: Reload the file from disk
- `:` Enter command mode
- `.` repeat last command
- `?`: Show help

## Multichart mode

- `backspace/delete/d`: Remove currently selected source from multichart
- `M`: Toggle back to normal mode
- `c`: Clear zoom
- `j`/`k`: Move between series
- `h`/`l` or `shift` + `right`/`left`: Pan right/left
- `+`/`-` or `shift` + `up`/`down`: Zoom in/out by 10%

## Edit mode

Shift focus to an attribute name or value or preview value and press `enter` or `e` to enter edit mode. Edit mode will open \"$EDITOR\" with the current value. Edit the value then save and close the editor to update the value in the file. In read-only mode, h5v will warn instead of editing.

## Commands

- `:n` Go the nth item
- `:+n` Go down n items
- `:-n` Go up n items

For example, `:5` will go to the 5th item, `:+3` will go down 3 items, and `:-2` will go up 2 items.
Use `:` to enter command mode, type the command, and press `enter` to execute it.
Use `.` to repeat the last command.

## Installation

```bash
cargo install h5v
```

## Roadmap

- [ ] Edit file on readonly -> ask to open in write mode.
- [ ] Add edit value for dataset values (scalar and single values)
- [ ] Adding/Updating/Deletion of attributes/matrix values
- [ ] Add more command support: All actions could be cmd'able -> delete attribute, remove dataset from multi-chart, go up 50, etc. Anything that could change the state basically.
