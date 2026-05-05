# Troubleshooting and limits

## "Cannot edit in read-only mode"

h5v only writes when the file is opened with `-w`:

```bash
h5v -w file.h5
```

If you forget that flag, the UI stays navigable but edit actions report that the file must be reopened in write mode.

## An image dataset does not render as an image

Check the HDF5 image metadata first:

- `CLASS` must be `IMAGE`
- `IMAGE_SUBCLASS` must be a supported image subclass
- truecolor and indexed images also need `INTERLACE_MODE`

If those attributes are missing, h5v falls back to normal dataset handling.

## The UI is blank or badly garbled

Some terminals partially respond to graphics capability probes but do not render the resulting protocol correctly.

Start h5v with:

```bash
h5v --no-terminal-graphics file.h5
```

That forces the safer text-only preview path and is a good first workaround on browser-backed or otherwise unusual terminal emulators.

## A compound dataset does not show in matrix mode

That is expected for the compound container itself. The root compound node shows a recursive schema preview. Drill down to a projected leaf field if you want normal preview or matrix rendering.

## Large fixed strings look truncated

Fixed strings are read with a `32768` byte cap.

## Very large previews are chunked

Chart previews are segmented with a maximum segment size of `250000` elements. Use `PageUp` and `PageDown` to move through the data.

## A numeric preview fails to render bounds

If the current slice contains only invalid numeric values such as `NaN` or infinity, h5v cannot compute chart bounds and reports that state in the preview.

## Very wide or tall images

If an image is much wider or taller than the available content pane, h5v may show a windowed image view instead of shrinking everything into a tiny thumbnail. That is expected and is how the pannable image experience works.

Use the bundled `/images/wide_grayscale` example if you want a known-good dataset for testing that behavior.
