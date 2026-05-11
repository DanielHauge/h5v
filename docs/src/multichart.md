# Multichart

![Multichart workspace](./assets/multi-chart.png)

Multichart is the comparison workspace for previewable series.

Sources:

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

Groups with `H5V_PREVIEW_EXPR` also work here. Pressing `m` on `/group_preview` adds the group preview expression as a chart item.

## Derived operations

| Key / command | Operation | Notes |
| --- | --- | --- |
| `D` / `mchart derive difference` | Difference | Subtract selected from base |
| `S` / `mchart derive sum` | Sum | Adds base and selected |
| `R` / `mchart derive ratio` | Ratio | Errors if a divisor is zero |
| `P` / `mchart derive product` | Product | Multiplies pointwise |
| `X` / `mchart derive xy` | X/Y pair | Requires exact length match |

Difference, sum, ratio, and product align by the shorter series length. X/Y requires an exact match.

## Visibility and organization

- move through chart items with `j` / `k`
- hide or show an item with `v` or `Enter`
- remove the selected item with `d`, `Backspace`, or `Delete`
- clear the whole workspace with `C`

## Zoom and pan

- `+`, `=`, or `Shift+Up` to zoom in
- `-` or `Shift+Down` to zoom out
- `h` / `Shift+Left` to pan left
- `l` / `Shift+Right` to pan right
- `c` to reset zoom

The same actions are available from the command line.

## Expression workflows

See [Multichart expressions](./multichart-expressions.md) for syntax and [Command reference](./command-reference.md) for the full `mchart` command surface.
