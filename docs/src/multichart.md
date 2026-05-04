# Multichart

## What multichart is for

![Multichart workspace](./assets/multi-chart.png)

Multichart is h5v's comparison workspace. Instead of looking at one previewable slice at a time, you can collect several chart items and compare them in a single view.

Chart items can come from:

- the currently selected previewable tree selection
- an explicit dataset reference
- a built-in derived operation
- an expression-defined series

## Basic workflow

1. Add a series with `m` or `mchart add ...`.
2. Press `M` or run `mchart open` to enter the workspace.
3. Mark one series as the base with `Space` if you plan to derive comparisons.
4. Create derived items with `D`, `S`, `R`, `P`, or `X`.
5. Use zoom and pan to inspect the area of interest.

The bundled `examples/h5v-example.h5v` script follows exactly this workflow with `/signals/sine_wave` and `/signals/cosine_wave`.

Groups with `H5V_PREVIEW_EXPR` also participate here. Selecting `/group_preview` in the bundled example and pressing `m` adds the group's derived preview expression as a multichart item.

## Derived operations

Built-in derived operations are:

| Key / command | Operation | Notes |
| --- | --- | --- |
| `D` / `mchart derive difference` | Difference | Subtract selected from base |
| `S` / `mchart derive sum` | Sum | Adds base and selected |
| `R` / `mchart derive ratio` | Ratio | Errors if a divisor is zero |
| `P` / `mchart derive product` | Product | Multiplies pointwise |
| `X` / `mchart derive xy` | X/Y pair | Requires exact length match |

Difference, sum, ratio, and product align their inputs by the shorter series length. X/Y output is stricter and requires equal-length input.

## Visibility and organization

Within multichart mode you can:

- move through chart items with `j` / `k`
- hide or show an item with `v` or `Enter`
- remove the selected item with `d`, `Backspace`, or `Delete`
- clear the whole workspace with `C`

## Zoom and pan

The multichart viewport tracks an area of interest. Use:

- `+`, `=`, or `Shift+Up` to zoom in
- `-` or `Shift+Down` to zoom out
- `h` / `Shift+Left` to pan left
- `l` / `Shift+Right` to pan right
- `c` to reset zoom

The same capabilities are available from commands such as `mchart zoom in 10` and `mchart pan right 10`.

## Expression workflows

Expression-derived chart items support dataset references, scalar attributes, existing chart items, and tuple expressions for explicit x/y series. See [Multichart expressions](./multichart-expressions.md) for the syntax.
