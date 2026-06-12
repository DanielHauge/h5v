# Troubleshooting and limits

## "Cannot edit in read-only mode"

h5v only writes when the file is opened with `-w`:

```bash
h5v -w file.h5
```

If you forget `-w`, edit actions report that the file must be reopened in write mode.

## An image dataset does not render as an image

Check the image metadata:

- `CLASS` must be `IMAGE`
- `IMAGE_SUBCLASS` must be a supported image subclass
- truecolor and indexed images also need `INTERLACE_MODE`

See [Image conventions](./image-conventions.md).

## The UI is blank or badly garbled

Try:

```bash
h5v --no-terminal-graphics file.h5
```

If the terminal also struggles with richer symbols or line drawing:

```bash
h5v --compatibility file.h5
```

Compatibility mode switches to simpler symbols and text/braille fallbacks. Multichart also falls back to a terminal-native braille plot.

See [Installation](./installation.md) for persistent compatibility settings.

## A Linux release says `GLIBC_x.y` was not found

Official Linux releases target Ubuntu 22.04 and newer.

If your distro is older than that baseline, build locally instead:

```bash
cargo install h5v
```

That links against your local system libraries instead of the release builder's glibc version.

## A compound dataset does not show in matrix mode

That is expected for the compound container itself. The root node shows a schema preview. Drill down to a projected leaf field for preview or matrix rendering.

## Large fixed strings look truncated

Fixed strings are read with a `32768` byte cap.

## Very large previews are chunked

Chart previews are segmented with a maximum segment size of `250000` elements. Use `PageUp` and `PageDown` to move through the data.

## A numeric preview fails to render bounds

If the current slice contains only invalid numeric values such as `NaN` or infinity, h5v cannot compute chart bounds.

## Very wide or tall images

If an image is much wider or taller than the content pane, h5v may switch to a pannable windowed view instead of shrinking it aggressively.

Use `/images/wide_grayscale` from the bundled example file to test this path.
