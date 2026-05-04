# Compound datasets

## Synthetic child nodes

The bundled example file includes `/compound/records` and `/compound/nested_records` so you can test both simple and nested compound browsing immediately.

Compound datasets are not treated as opaque blobs. h5v creates synthetic tree nodes for their fields so you can navigate the compound structure directly from the main tree.

## Root-level schema preview

When the focused node is the root of a compound dataset, the preview pane shows a recursive schema view rather than an empty content area. The schema includes:

- field names
- field types
- byte offsets
- nested compounds and compound arrays

## Recursion handling

Schema rendering has a hard recursion limit of `32` nested levels and explicitly omits recursive loops once they are detected. That keeps pathological or self-referential layouts from hanging the preview.

## Projected field workflows

Once you drill down to a concrete leaf field, that projected field behaves like an ordinary dataset slice:

- numeric fields can preview as charts
- matrixable fields can render in matrix mode
- scalar string fields render as text
- multi-value string arrays stay matrix-only

The bundled example is a good regression harness here:

- `/compound/records/label` shows projected string handling
- `/compound/nested_records/window` shows a projected fixed array that can be edited from matrix mode

This makes compound-heavy files much easier to inspect without exporting fields into standalone datasets first.
