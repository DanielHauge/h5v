# h5v

HDF5 terminal viewer with matrix/chart/image previews, compound-schema browsing, editable
attributes, and startup scripting.

Run `h5v` with the path to an HDF5 file:

```bash
h5v path/to/file.h5
```

![](./docs/chart_example.png)
![](./docs/image_example.png)
![](./docs/matrix_example.png)

## Highlights

- Explore files as a tree with dataset, group, link, and synthetic compound-field nodes.
- Preview numeric data as charts, dense data as matrices, and image datasets inline.
- Browse compound datasets from the root schema down to individual projected fields.
- Edit values and attributes in-place, and create or delete scalar attributes from the UI or
  command mode.
- Script startup actions with `--command`, `--script`, `--script-test`, and `press ...`.

## Controls

- `j`/`k`/`up`/`down`: Navigate lists and move selections
- `h`/`l`/`left`/`right`: Collapse/expand tree items and move within content
- `enter`/`space`: Toggle tree items
- `Tab`: Switch between preview and matrix when both are available
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
- `e`: Edit the focused preview value or attribute
- `a`: Create an attribute from the attributes pane
- `d`/`Delete`: Delete the focused attribute from the attributes pane
- `Esc`: Cancel popups such as attribute create/delete dialogs
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
- `e`: Open the expression prompt
- `space`: Mark or unmark the base series
- `D`/`S`/`R`/`P`/`X`: Create derived difference, sum, ratio, product, or x/y series
- `enter`/`v`: Hide or show the selected series
- `C`: Clear all series
- `c`: Clear zoom
- `j`/`k`: Move between series
- `h`/`l` or `shift` + `right`/`left`: Pan right/left
- `+`/`-` or `shift` + `up`/`down`: Zoom in/out by 10%

Expression-derived series support:

- series references by workspace id: `$1`
- scalar numeric attributes: `#SCALE`, `#../:OFFSET`, `#/group/ds:BIAS`
- exact dataset path series: `!/my_dataset`, `!/my_dataset[..,0]`, `!/my_dataset_bigger_dim[1,..,2,3]`
- tuple expressions for computed x/y series: `($1 * #SCALE, !/ticks + #OFFSET)`

## Edit mode

Shift focus to an attribute name or value or preview value and press `enter` or `e` to enter edit mode. Edit mode will open \"$EDITOR\" with the current value. Edit the value then save and close the editor to update the value in the file. In read-only mode, h5v will warn instead of editing.

## Compound datasets

Compound datasets expose synthetic child nodes for each field. When you focus the root compound node,
the Preview tab now shows a recursive schema view for the current compound type instead of a blank
content pane. Nested compounds and compound arrays are expanded, and pathological recursion is cut
off with an explicit placeholder so schema rendering always terminates cleanly.

Use the tree to drill into individual compound fields when you want normal preview or matrix behavior
for a projected leaf field.

## Commands

Use `:` to open the bottom command minibuffer, type a command, and press `enter` to run it.

Current command features:

- `Tab` / `Shift+Tab`: cycle and apply command completion
- `Up` / `Down`: move through command suggestions
- `Ctrl+P` / `Ctrl+N`: browse command history
- `.` or `repeat`: repeat the last successful command
- `help` or `help <command>`: open help or show help for a specific command

Examples:

- `goto /group/dataset`
- `seek 5`
- `down 3`
- `focus content`
- `mode matrix`
- `toggle-tree`
- `reload`
- `x next`
- `row prev`
- `index next 10`
- `attr create title string "release candidate"`
- `attr delete title`
- `mchart open`
- `mchart add /group/dataset[..,0]`
- `mchart expr "($1, !/ticks + #OFFSET)"`
- `mchart derive difference`
- `press ctrl+w o`
- `press M j enter`

Legacy numeric aliases are still supported:

- `:5` -> `seek 5`
- `:+3` -> `down 3`
- `:-2` -> `up 2`

## Startup automation

Startup commands use the same parser and command catalog as the interactive minibuffer.

- `-c`, `--command <COMMAND>`: run a command at startup, repeatable
- `--script <PATH>`: load startup commands from a file
- `--script -`: read startup commands from stdin and warn if EOF arrives without any commands
- piped stdin is also consumed implicitly without `--script -`
- `--script-test` or `-ct`: validate startup commands and print a formatted dry-run summary
- attribute commands, multichart commands, and `press ...` are all available from startup scripts

Scripts and inline command strings can separate commands with either newlines or `;`.
Blank lines and lines starting with `#` are ignored in script input.

Examples:

```bash
h5v file.h5 -c "focus content" -c "mode matrix"
h5v file.h5 -c "mchart open" -c "mchart add /group/dataset[..,0]"
h5v file.h5 -c "attr create title string \"draft\""
h5v file.h5 -c "mchart expr \"($1, !/ticks + #OFFSET)\""
h5v file.h5 --script setup.h5v
printf 'toggle-tree; mode preview\nreload\n' | h5v file.h5
h5v file.h5 --script-test --script setup.h5v
```

## Installation

Prebuilt binaries (recommended once a release is published):

```bash
cargo binstall h5v
```

Shell installer (Linux/macOS, installs the latest release to `~/.local/bin` by default):

```bash
curl -fsSL https://raw.githubusercontent.com/DanielHauge/h5v/main/install.sh | sh
```

PowerShell installer (Windows, installs the latest release to `~/.local/bin` by default):

```powershell
irm https://raw.githubusercontent.com/DanielHauge/h5v/main/install.ps1 | iex
```

Homebrew (Linux/macOS, after a release updates the in-repo formula):

```bash
brew install DanielHauge/h5v/h5v
```

Scoop (Windows, after a release updates the in-repo bucket manifest):

```powershell
scoop bucket add h5v https://github.com/DanielHauge/h5v
scoop install h5v/h5v
```

Or download the archive for your platform from the GitHub Releases page.

Source build fallback:

```bash
cargo install h5v
```

On Linux, source builds may require native build dependencies such as `cmake`, `pkg-config`, `libfontconfig`, `freetype`, and `expat` development packages.
