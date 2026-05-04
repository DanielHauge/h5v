# Quick start

You can open any HDF5 file with `h5v` to start exploring. If you do not have one handy, you can download the example file from the repository in the `examples` folder, or generate it yourself with the provided script.

You can always open the in-app help overlay with `?` to get a reminder of the key bindings.

![h5v quick layout overview](./assets/help.png)

## Open a file

```bash
h5v path/to/file.h5
```

If you plan to edit dataset values or attributes, reopen with write mode:

```bash
h5v -w path/to/file.h5
```

## Try the bundled example

The repository includes a compact example file plus a startup script that opens a multichart comparison workflow:

```bash
h5v examples/h5v-example.h5 --script examples/h5v-example.h5v
```

If you are editing the example content itself, regenerate the `.h5` from:

```bash
python scripts/generate_example_h5.py
```

## Learn the layout

The default workflow is:

1. Move around the tree on the left.
2. Inspect the selected dataset or group preview in the content pane.
3. Check or edit attributes in the metadata pane.

Useful keys to memorize immediately:

- `?` opens the in-app help overlay
- `Tab` switches between preview and matrix when both exist
- `:` opens the command minibuffer
- `m` adds the current preview to multichart, including group previews driven by `H5V_PREVIEW_EXPR`
- `M` opens or closes multichart mode

Interesting paths inside `examples/h5v-example.h5`:

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

## First commands to try

```text
:goto /signals/sine_wave
:goto /group_preview
:mode matrix
:help mchart
```

## Script a repeatable startup

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

Dry-run validation:

```bash
h5v --script-test --script examples/h5v-example.h5v
```
