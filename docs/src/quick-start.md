# Quick start

Open a file:

```bash
h5v path/to/file.h5
h5v -w path/to/file.h5
```

`?` opens the in-app help overlay.

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

- `?` opens the in-app help overlay
- `Tab` switches between preview and matrix when both exist
- `:` opens the command minibuffer
- `m` adds the current preview to multichart, including group previews driven by `H5V_PREVIEW_EXPR`
- `M` opens or closes multichart mode

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

See [Controls reference](./controls.md), [Commands](./commands.md), and [Startup scripting](./startup-scripting.md).
