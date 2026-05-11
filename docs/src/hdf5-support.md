# HDF5 feature support

h5v can browse groups, datasets, links, broken links, and synthetic nodes created for projected compound fields.

## Matrixable and previewable data types

| Type family | Preview | Matrix | Notes |
| --- | --- | --- | --- |
| Signed integers | Yes | Yes | Rendered as numeric data |
| Unsigned integers | Yes | Yes | Rendered as numeric data |
| Floating point | Yes | Yes | Rendered as numeric data |
| Boolean | Yes | Yes | Routed through unsigned rendering |
| Enum | Yes | Yes | Uses colored symbols and labels, with optional dataset-defined overrides |
| Fixed strings | Text | Yes | Fixed strings are read with a 32768-byte limit |
| Variable strings | Text | Yes | Good for inline string inspection |
| Compound | Schema or projected preview | Projected fields only | Root compound matrix rendering is not implemented |
| Fixed arrays | Limited | No | Standalone fixed arrays are not matrixable through the main renderer |
| Variable arrays | Limited | No | Not matrixable through the main matrix renderer |
| References | Limited | No | No dedicated matrix renderer |

Matrix mode is only available when a dataset is matrixable and its shape has at least one dimension larger than `1`. Single-value datasets stay in scalar preview mode.

## Strings and highlighting

String datasets can carry a `HIGHLIGHT` attribute with an extension hint such as `json`, `py`, or `yml`. If it is absent, h5v falls back to the dataset name extension.

## Enum styling overrides

Enum datasets can override the default symbol and color cycle with:

- `SYMBOLS`: a 1D string attribute, aligned with ascending numeric enum value order
- `COLORS`: a 1D string attribute, aligned with ascending numeric enum value order

Color values accept named colors and `#RRGGBB`. See [Configuration reference](./configuration-reference.md).

## Group preview expressions

Groups can opt into preview rendering with a variable-length string attribute named `H5V_PREVIEW_EXPR`. The value uses the same expression syntax as multichart.

## Image metadata handling

Datasets that follow the HDF5 image convention are rendered inline as images. See [Images](./images.md) and [Image conventions](./image-conventions.md).

## Properties vs attributes

`type`, `size`, `shape`, `chunk`, `link`, and `path` are properties shown in the Properties section. They are not HDF5 attributes.

Those built-in properties include:

- `type`
- `size`
- `shape`
- `chunk`
- `link`
- `path`
