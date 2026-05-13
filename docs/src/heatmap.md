# Heatmap

Heatmap is the image-style view for numeric datasets with at least two non-singleton dimensions.

It is available only when:

- compatibility mode is off
- terminal image rendering is available

## What it shows

- a rendered 2D slice
- slice/page context
- settings
- viewport stats
- optional selection stats
- legend and histogram

## Settings

The settings panel controls:

- colormap
- range mode
- invert x
- invert y
- normalization

Use `Up` / `Down` to move between settings and `Left` / `Right` to change the selected value.

## Selection and viewport

- no explicit selection means the active region is the current viewport
- one left click selects one terminal-cell region
- a second left click expands that to a rectangle
- another left click after a rectangle clears the explicit selection

The region panel shows both:

- viewport `x/y/w/h`, `mean`, `std`
- selection `x/y/w/h`, `mean`, `std`

## Zoom and pan

- `z` zoom in
- `Z` zoom out
- `0` reset viewport
- `v` clear explicit selection
- `H` / `J` / `K` / `L` pan the zoomed viewport
- `PageUp` / `PageDown` move through segmented heatmap pages

## Mouse

- left click selects a region
- wheel zoom is anchored to the hovered cell
- right click on an explicit selection zooms into that selection
- right-click drag pans the viewport

## Copy

`y` copies the active heatmap summary:

- selection summary when a region is selected
- viewport summary otherwise
