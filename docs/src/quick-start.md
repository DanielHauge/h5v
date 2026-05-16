# Quick start

Open a file:

```bash
h5v path/to/file.h5
h5v -w path/to/file.h5
```

Press `?` to open the in-app help.

![h5v quick layout overview](./assets/help.png)

## Try the bundled example

```bash
h5v examples/h5v-example.h5 --script examples/h5v-example.h5v
```

Regenerate the example file if needed:

```bash
python scripts/generate_example_h5.py
```

## Default workflow

1. Move around the tree on the left.
2. Inspect the selected node in the content pane.
3. Inspect or edit metadata in the attributes pane.

Useful keys:

- `?` opens the in-app help
- `Tab` switches between preview and matrix when both exist
- `:` opens the command minibuffer
- `m` adds the current preview to multichart, including group previews driven by `H5V_PREVIEW_EXPR`
- `M` opens or closes multichart mode

## Help

Press `?`. The in-app help is the reference for keys and commands.

The help view has five tabs:

- `Keymap`
- `Commands`
- `Multichart`
- `Heatmap`
- `Customization`

Use `Tab` / `Shift+Tab` or `h` / `l` to switch tabs.

In `Keymap`, `Commands`, and `Customization`, use `j` / `k`, arrow keys, `Home`, `End`, `g`, and `G` to move through the left-hand list.

Example paths:

| Path | What to look at |
| --- | --- |
| `/signals/sine_wave` | Basic numeric chart preview |
| `/matrices/cube` | Matrix mode with dimension selectors |
| `/images/truecolor_rgb` | Inline truecolor image |
| `/images/wide_grayscale` | Wide pannable image window |
| `/images/varlen_png_frames` | Variable-length encoded image frames |
| `/compound/nested_records` | Recursive compound schema view |
| `/compound/nested_records/window` | Projected fixed-array field with matrix editing |
| `/metadata/attributes_demo` | Mixed attribute types and references |
| `/group_preview` | Group-level chart preview driven by `H5V_PREVIEW_EXPR` |

## First commands

```text
:goto /signals/sine_wave
:goto /group_preview
:mode matrix
:help mchart
```

## Script a startup

Inline commands:

```bash
h5v examples/h5v-example.h5 \
  -c "goto /signals/sine_wave" \
  -c "mchart add" \
  -c "goto /signals/cosine_wave" \
  -c "mchart add" \
  -c "mchart show"
```

Script file:

```bash
h5v examples/h5v-example.h5 --script examples/h5v-example.h5v
```

Dry-run:

```bash
h5v --script-test --script examples/h5v-example.h5v
```

See [Multichart](./multichart.md), [Configuration](./configuration.md), and [Startup scripting](./startup-scripting.md).
