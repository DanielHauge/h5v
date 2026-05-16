# Multichart

![Multichart workspace](./assets/multi-chart.png)

Multichart is the comparison workspace for previewable series.

Sources:

- the currently selected previewable tree selection
- an explicit dataset reference
- an expression-defined series

## Basic workflow

1. Add a series with `m` or `mchart add ...`.
2. Press `M` or run `mchart open` to enter the workspace.
3. Press `Enter` to open a new expression, or `e` to edit the selected series.
4. Build derived series with expressions such as `$1 - $2`, `($1, !/time[..])`, or `$1[0..256]`.
5. Use zoom and pan to inspect the area of interest.

Groups with `H5V_PREVIEW_EXPR` also work here. Pressing `m` on `/group_preview` adds the group preview expression as a chart item.

## Expression editor

- `Enter` opens a new expression without changing the chart viewport
- `e` edits the selected series in place
- `Enter` submits the current expression
- `Tab` completes the selected suggestion
- `Up` and `Down` move through suggestions while editing
- `Esc` closes the editor

The editor validates expressions live and suggests chart item ids, dataset paths, and attribute references.

Raw dataset references such as `!/big_dataset[..]` are queued and loaded in the background when submitted.
Zoomed dataset-backed views can request a denser viewport sample in the background while the coarse overview stays visible.
Derived series can also refine to viewport detail when their referenced chart-item inputs share the same loaded detail window.

## Config

Use `h5v.multichart = { ... }` in Lua to tune large-series behavior.

- `overview_max_samples` limits the initial background overview sample
- `detail_enabled` turns viewport-driven detail refinement on or off
- `detail_samples_per_column` scales viewport detail against chart width
- `detail_min_samples` and `detail_max_samples` clamp viewport detail size
- `detail_padding_ratio` loads extra x-range around the visible viewport
- `derived_detail_enabled` lets derived series refine when inputs share the same detail window

## Visibility and organization

- move through chart items with `j` / `k`
- reorder the selected item with `Alt+Up` / `Alt+Down`
- hide or show an item with `Space` or `v`
- remove the selected item with `d`, `Backspace`, or `Delete` when nothing depends on it
- clear the whole workspace with `C`
- open multichart help with `?`

## Views

- `Tab`, `Shift+Tab`, or `t` cycles line, histogram, box plot, and comparison scatter
- line focuses sampled curves
- histogram overlays visible-value distributions
- box plot summarizes visible-value quartiles, whiskers, and outliers
- comparison scatter aligns the selected series with the next visible series

## Zoom and pan

- `+`, `=`, or `Shift+Up` to zoom in
- `-` or `Shift+Down` to zoom out
- `h` / `Shift+Left` to pan left
- `l` / `Shift+Right` to pan right
- `f` to fit all visible series
- `F` to fit the selected series
- `c` to reset zoom

Mouse interaction follows the same anchored viewport model as heatmap:

- wheel zoom over the plot anchors to the hovered point
- plain wheel zoom changes both x and y
- `Ctrl` + wheel zoom changes x only
- `Shift` + wheel zoom changes y only
- right-click drag snapshots on press and pans on release

The same actions are available from the command line, including `mchart fit ...` and axis-specific zoom like `mchart zoom x in 20`.

## Expression workflows

See [Multichart expressions](./multichart-expressions.md). For commands and keys, use the in-app help.
