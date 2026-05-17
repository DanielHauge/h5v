<div class="oranda-hide">
<p align="center" class="hide-">
  <img src="https://raw.githubusercontent.com/DanielHauge/h5v/main/docs/src/assets/banner.png" alt="h5v banner with logo and title" />
</p>
</div>

> **A terminal-first HDF5 explorer for charts, matrices, images, compound schemas, and scripted workflows.**

```bash
curl -fsSL https://raw.githubusercontent.com/DanielHauge/h5v/main/install.sh | sh
```

The shell installer works on Linux, macOS, and POSIX-style Windows shells such as Git Bash, MSYS2, and Cygwin.

`h5v` is a Rust TUI for inspecting HDF5 files in the terminal: browse the tree, switch between preview, matrix, and heatmap views, inspect image datasets inline, drill into compound fields, edit attributes in write mode, and script startup workflows.

<div class="oranda-hide">

## What it looks like

| Charts                                                                                                     | Heatmap                                                                                                              | Images                                                                                                              |
| ---------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------- |
| ![Chart preview](https://raw.githubusercontent.com/DanielHauge/h5v/main/docs/src/assets/chart.jpg)         | ![Heatmap view](https://raw.githubusercontent.com/DanielHauge/h5v/main/docs/src/assets/heatmap.png)                 | ![Image preview](https://raw.githubusercontent.com/DanielHauge/h5v/main/docs/src/assets/images.jpg)                |
| Multichart                                                                                                 | Commands                                                                                                             | Help                                                                                                                |
| ![Multichart view](https://raw.githubusercontent.com/DanielHauge/h5v/main/docs/src/assets/multi-chart.jpg) | ![Command mode](https://raw.githubusercontent.com/DanielHauge/h5v/main/docs/src/assets/cmd.jpg)                    | ![Help overlay](https://raw.githubusercontent.com/DanielHauge/h5v/main/docs/src/assets/help.jpg)                  |

## Highlights

- Tree browsing for datasets, groups, links, and projected compound fields.
- Preview, matrix, heatmap, image, and schema views from the same selection.
- In-place edits when the file is opened with `-w`.
- Startup automation with commands, scripts, and `press ...`.
- Derived series and comparisons in multichart.

</div>

## Quick start

If you do not have an h5 file, you can download an example:

```bash
curl -fsSL https://raw.githubusercontent.com/DanielHauge/h5v/main/examples/h5v-example.h5 -o h5v-example.h5
```

Open a file in read-only mode:

```bash
h5v h5v-example.h5
```

If your terminal renders icons, line drawing, or graphics previews badly, start in compatibility mode:

```bash
h5v --compatibility h5v-example.h5
```

That switches the UI to simpler symbols and disables terminal graphics previews. To make it the default, set `H5V_COMPATIBILITY_MODE=true` in your shell rc file.

Open the same file in write mode so edits are allowed:

```bash
h5v -w h5v-example.h5
```

Themes, symbols, and heatmap defaults are configured in Lua. Inside `h5v`, run `:configure` to open `init.lua`, or `:configure reset` to regenerate the default scaffold.

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

| Method               | Command                                                                                   |
| -------------------- | ----------------------------------------------------------------------------------------- |
| Shell installer      | `curl -fsSL https://raw.githubusercontent.com/DanielHauge/h5v/main/install.sh \| sh`      |
| PowerShell installer | `irm https://raw.githubusercontent.com/DanielHauge/h5v/main/install.ps1 \| iex`           |
| Homebrew             | `brew tap DanielHauge/h5v https://github.com/DanielHauge/h5v.git && brew install h5v`     |
| Scoop                | `scoop bucket add h5v https://github.com/DanielHauge/h5v && scoop install h5v/h5v`        |
| cargo-binstall       | `cargo binstall h5v`                                                                      |
| Cargo source build   | `cargo install h5v`                                                                       |

On Windows, `install.ps1` installs into `%LOCALAPPDATA%\Programs\h5v\bin` and adds that directory to the user `PATH`.

On Linux, source builds may require native packages such as `cmake`, `pkg-config`, `libfontconfig`, `freetype`, and `expat` development headers.

## Configuration and plugins

Configuration lives in `init.lua`.

```text
:configure
:configure reset
```

- `:configure` opens the config file and reloads it on exit.
- `:configure reset` writes a fresh scaffold.
- `--config <PATH>` uses a different config file.
- `--init-plugin <PATH>` creates a plugin scaffold. Rerun it to refresh `.luarc.json` and `.h5v-luals/h5v.lua` without overwriting `h5v-plugin.toml` or `lua/init.lua`.

Plugins are loaded from `init.lua` with `h5v.plugins.use(...)` and can come from:

- a local path
- `owner/repo`
- a git URL

Use the in-app help as the source of truth for commands, keymaps, actions, health, and plugin status.

![Light theme configuration example](https://raw.githubusercontent.com/DanielHauge/h5v/main/docs/src/assets/themes.png)

## Documentation

The manual is published at [danielhauge.github.io/h5v/book](https://danielhauge.github.io/h5v/book/), and the source lives in [`docs/src`](https://github.com/DanielHauge/h5v/tree/main/docs/src).

- Inside `h5v`, press `?` for keybindings, commands, multichart, heatmap, health, and customization help.
- The in-app help is the primary reference for controls and command behavior.

- [Overview](https://danielhauge.github.io/h5v/book/)
- [Quick start](https://danielhauge.github.io/h5v/book/quick-start.html)
- [Configuration](https://danielhauge.github.io/h5v/book/configuration.html)
- [Plugins](https://danielhauge.github.io/h5v/book/plugins.html)
- [Heatmap](https://danielhauge.github.io/h5v/book/heatmap.html)
- [Multichart](https://danielhauge.github.io/h5v/book/multichart.html)

## Core interaction model

- `Shift` + arrow keys or `Ctrl+W` then `h/j/k/l` move focus between panes.
- `Tab` cycles content modes when more than one is available.
- `:` opens the command minibuffer, `.` repeats the last command, and `?` opens the in-app help overlay.
- `m` adds the current previewable selection to multichart and `M` opens multichart mode.
- `s` toggles the sidebar, `/` enters search, and `Ctrl+R` reloads the file from disk.
