
<div class="oranda-hide">
<p align="center" class="hide-">
  <img src="./docs/src/assets/banner.png" alt="h5v banner showing HDF5 terminal viewing" />
</p>
</div>

> **A terminal-first HDF5 explorer for charts, matrices, images, compound schemas, and scripted workflows.**

```bash
curl -fsSL https://raw.githubusercontent.com/DanielHauge/h5v/main/install.sh | sh
```

The shell installer works on Linux, macOS, and POSIX-style Windows shells such as Git Bash, MSYS2, and Cygwin. It defaults to the first writable directory already on `PATH`, falling back to `~/.local/bin` on Linux and `~/bin` elsewhere.

`h5v` is a Rust TUI for inspecting HDF5 files without leaving the terminal. It is built for fast data exploration: browse the tree, switch between chart and matrix views, inspect image datasets inline, drill into compound fields, edit attributes in write mode, and script startup workflows for repeatable sessions.

<div class="oranda-hide">

## What it looks like

| Charts | Images |
| --- | --- |
| ![Chart preview](./docs/src/assets/chart.jpg) | ![Image preview](./docs/src/assets/images.jpg) |
| Multichart | Commands |
| ![Multichart view](./docs/src/assets/multi-chart.jpg) | ![Command mode](./docs/src/assets/cmd.jpg) |
| Help | Scripting |
| ![Help overlay](./docs/src/assets/help.jpg) | ![Startup scripting and code view](./docs/src/assets/code.jpg) |

## Highlights

- Explore datasets, groups, links, and synthetic compound-field nodes from a tree view.
- Switch between preview, matrix, schema, and image-oriented views from the same selection.
- Edit values and scalar attributes in place when the file is opened with `-w`.
- Automate repeatable sessions with `--command`, `--script`, `--script-test`, and simulated `press ...` input.
- Build derived comparisons and expression-based overlays in multichart mode.

</div>

## Quick start

Open a file in read-only mode:

```bash
h5v path/to/file.h5
```

Open the same file in write mode so edits are allowed:

```bash
h5v -w path/to/file.h5
```

Start with scripted commands:

```bash
h5v examples/h5v-example.h5 \
  -c "goto /matrices/cube" \
  -c "focus content" \
  -c "mode matrix"
```

Validate a startup script without launching the UI:

```bash
h5v path/to/file.h5 --script-test --script setup.h5v
```

Try the bundled example file and walkthrough script from this repository:

```bash
h5v examples/h5v-example.h5 --script examples/h5v-example.h5v
```

Regenerate the example file after editing the generator:

```bash
python scripts/generate_example_h5.py
```

## Installation

| Method | Command |
| --- | --- |
| Shell installer | `curl -fsSL https://raw.githubusercontent.com/DanielHauge/h5v/main/install.sh \| sh` |
| PowerShell installer | `irm https://raw.githubusercontent.com/DanielHauge/h5v/main/install.ps1 \| iex` |
| Homebrew | `brew install DanielHauge/h5v/h5v` |
| Scoop | `scoop bucket add h5v https://github.com/DanielHauge/h5v && scoop install h5v/h5v` |
| cargo-binstall | `cargo binstall h5v` |
| Cargo source build | `cargo install h5v` |

On Linux, source builds may require native packages such as `cmake`, `pkg-config`, `libfontconfig`, `freetype`, and `expat` development headers.

## Documentation

The full manual lives in [`docs/src`](./docs/src) and is organized as an mdBook.

| Guide | Link |
| --- | --- |
| Book contents | [`docs/src/SUMMARY.md`](./docs/src/SUMMARY.md) |
| Installation | [`docs/src/installation.md`](./docs/src/installation.md) |
| Navigation and layout | [`docs/src/navigation.md`](./docs/src/navigation.md) |
| Controls reference | [`docs/src/controls.md`](./docs/src/controls.md) |
| HDF5 support and previews | [`docs/src/hdf5-support.md`](./docs/src/hdf5-support.md) |
| Images and conventions | [`docs/src/images.md`](./docs/src/images.md) |
| Commands and scripting | [`docs/src/commands.md`](./docs/src/commands.md) |
| Multichart guide | [`docs/src/multichart.md`](./docs/src/multichart.md) |
| Bundled example workflow | [`examples/h5v-example.h5`](./examples/h5v-example.h5) and [`examples/h5v-example.h5v`](./examples/h5v-example.h5v) |

## Core interaction model

- `Shift` + arrow keys or `Ctrl+W` then `h/j/k/l` move focus between panes.
- `Tab` cycles content modes when the current dataset can be shown in more than one way.
- `:` opens the command minibuffer, `.` repeats the last command, and `?` opens the in-app help overlay.
- `m` adds the current previewable selection to multichart and `M` opens multichart mode.
- `s` toggles the sidebar, `/` enters search, and `Ctrl+R` reloads the file from disk.
