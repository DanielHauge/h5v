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

- `j`/`k`/`up`/`down`: Navigate through the items
- `enter`/`space`/`l`/`h`: Open/close items
- `shift` + navigate: shift focus
- `q` / `ctrl`+`c`: Quit
- `y`: Copy highlighted to clipboard
- `ctrl` + navigate: Scroll through contents (image list or matrix)
- `PgUp`/`PgDown`: Scroll through contents by half a page (image list or matrix)
- `ctrl` + `d`/`u`: Navigate by half a page
- `g`/`Home`: Go to the top
- `G`/`End`: Go to the bottom
- `:` Enter command mode
- `.` repeat last command
- `?`: Show help

## Commands

- `:n` Go the nth item
- `:+n` Go down n items
- `:-n`  Go up n items

For example, `:5` will go to the 5th item, `:+3` will go down 3 items, and `:-2` will go up 2 items.
Use `:` to enter command mode, type the command, and press `enter` to execute it.
Use `.` to repeat the last command.

## Installation

```bash
cargo install h5v
```
