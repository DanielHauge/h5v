# Preview types

## Numeric chart preview

![Chart preview](./assets/chart.png)

For numeric datasets, h5v's default preview is a line plot over a selected one-dimensional slice.

The preview model keeps track of:

- the active x-axis dimension
- fixed indices for the other dimensions
- the currently visible slice when the data is segmented

This lets you browse higher-dimensional arrays without losing context. Use `x` / `X`, `[` / `]`, and the index controls to move through the dataset without leaving the preview.

Large previews are segmented. The current implementation limits a preview segment to `250000` elements, and `PageUp` / `PageDown` move through larger chunks efficiently.

If a numeric slice contains only invalid values such as `NaN` or infinity, h5v reports that it cannot establish valid bounds for the preview.

Chart previews are rendered through the same terminal image pipeline as dataset image previews, so they look best in terminals with strong graphics protocol support. Kitty is the best default target:

- Kitty graphics protocol: <https://sw.kovidgoyal.net/kitty/graphics-protocol/>
- ratatui-image backend support: <https://github.com/ratatui/ratatui-image>
- Terminal compatibility gallery: <https://benjajaja.github.io/ratatui-image-screenshots/>

## Scalar and string preview

Scalar datasets render as text instead of chart or matrix views. This covers:

- floating-point scalars
- signed and unsigned integer scalars
- fixed and variable strings

That makes h5v useful for files that mix heavy numeric data with metadata-style datasets.

Scalar enum datasets also render through the enum renderer, so they can show both a symbol and a color-coded label instead of only the raw numeric value. If the dataset defines `SYMBOLS` and `COLORS` string attributes, h5v uses those overrides in ascending numeric enum value order before falling back to the built-in defaults.

String datasets can also carry syntax-highlighting hints. h5v resolves them in this order:

- `HIGHLIGHT` attribute on the dataset
- dataset name extension such as `.py` or `.yml`

That means the bundled example highlights `/strings/config_json` via `HIGHLIGHT=json`, while `/strings/demo.py` and `/strings/pipeline.yml` highlight from their dataset names when no attribute is present.

## File preview

Selecting the root file node keeps the content pane alive too. h5v renders a filesystem metadata table for the open file, including path, size, timestamps, permissions, and open mode, so the content pane focus remains obvious even at the root.

## Schema preview for compounds

Selecting the root of a compound dataset shows a recursive schema preview instead of a blank pane. The schema view expands nested compounds and compound arrays and stops recursion cleanly when the nesting becomes pathological.

Projected compound leaves keep following their concrete field type. For example:

- `/compound/nested_records/gain` previews like a numeric field
- `/compound/nested_records/window` is matrixable and editable as one value per line
- projected enum leaves can inherit custom symbol/color styling from dataset metadata
- projected multi-value string arrays stay matrix-only instead of attempting chart preview

## Group preview

Groups now keep the content pane alive as well. If a group has a variable-length string attribute named `H5V_PREVIEW_EXPR`, h5v evaluates it with the same expression syntax used by multichart and renders the resulting chart in the preview pane.

If the attribute is missing, the group preview shows a friendly empty-state message instead of a blank pane, which also makes focused group content easier to see.

The bundled example includes `/group_preview`, which uses:

```text
(!/group_preview/time, (!/group_preview/value - #/group_preview/offset) * #/group_preview:scale)
```

That renders `time` on the x-axis and `(value - offset) * scale` on the y-axis from group-local datasets and attributes.

Pressing `m` on that group in the tree adds the same preview expression directly to multichart as an expression-derived chart item.

## Image preview

Datasets recognized as HDF5 images render inline in the content pane. h5v supports grayscale, bitmap, truecolor, indexed, JPEG, and PNG-backed image datasets, including multi-frame layouts. See [Images](./images.md) for the format rules and expected attributes.

![Images](./assets/images.png)
