# HDF5 feature support

## Dataset and node support

h5v can browse groups, datasets, links, broken links, and synthetic nodes created for projected compound fields. That makes it useful both for simple numeric files and for deeper HDF5 layouts where the meaningful data is nested inside compound types.

The bundled `examples/h5v-example.h5` intentionally includes each of those categories in a compact tree so you can test the UI against a known file instead of assembling your own from scratch.

## Matrixable and previewable data types

The core matrix/preview pipeline supports these HDF5 type families:

| Type family | Preview | Matrix | Notes |
| --- | --- | --- | --- |
| Signed integers | Yes | Yes | Rendered as numeric data |
| Unsigned integers | Yes | Yes | Rendered as numeric data |
| Floating point | Yes | Yes | Rendered as numeric data |
| Boolean | Yes | Yes | Routed through unsigned rendering |
| Enum | Yes | Yes | Matrix rendering uses colored symbols and labels |
| Fixed strings | Text | Yes | Fixed strings are read with a 32768-byte limit |
| Variable strings | Text | Yes | Good for inline string inspection |
| Compound | Schema or projected preview | Projected fields only | Root compound matrix rendering is not implemented |
| Fixed arrays | Limited | No | Standalone fixed arrays are not matrixable through the main renderer |
| Variable arrays | Limited | No | Not matrixable through the main matrix renderer |
| References | Limited | No | No dedicated matrix renderer |

Matrix mode is only available when a dataset is matrixable and its shape has at least one dimension larger than `1`. Single-value datasets stay in scalar preview mode.

## Strings and highlighting

String datasets can carry a `HIGHLIGHT` attribute with an extension hint such as `json`, `py`, or `yml`. That hint takes precedence. If the attribute is absent, h5v falls back to the dataset name and uses the trailing extension instead, so datasets like `demo.py` or `pipeline.yml` can highlight automatically.

## Group preview expressions

Groups can opt into preview rendering with a variable-length string attribute named `H5V_PREVIEW_EXPR`. The value is interpreted as a multichart expression, so the same explicit reference rules apply there too: `!` for series, `#` for scalars, and `:ATTR` for object attributes.

That lets a group act like a lightweight dashboard node for related datasets instead of showing an empty content pane.

The bundled example file includes a `/group_preview` group that demonstrates this pattern with `time`, `value`, a scalar `offset` dataset, a scalar `scale` attribute, and an expression that plots `time` against `(value - offset) * scale`.

The same group can also be added directly to multichart with `m` or `mchart add` while it is selected.

## Image metadata handling

Datasets that follow the standard HDF5 image convention are treated specially. h5v recognizes `CLASS="IMAGE"` plus the expected image subclass attributes and renders those datasets inline as images instead of plain numeric arrays. The [Images](./images.md) chapter covers the exact rules.

## Attribute filtering

Some system-level metadata is intentionally hidden from normal attribute editing views, including values such as:

- `type`
- `size`
- `shape`
- `chunk`
- `link`
- `path`

Standard HDF5 image marker attributes are also filtered out of the user-editable attribute list so that structural metadata and user metadata stay separate.
