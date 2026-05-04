# Image conventions

## Required attributes

h5v follows the standard HDF5 image convention. A dataset must have:

1. `CLASS = "IMAGE"`
2. `IMAGE_SUBCLASS = ...`

Supported `IMAGE_SUBCLASS` values are:

- `IMAGE_GRAYSCALE`
- `IMAGE_TRUECOLOR`
- `IMAGE_BITMAP`
- `IMAGE_INDEXED`
- `IMAGE_JPEG`
- `IMAGE_PNG`

## Interlace mode

Truecolor and indexed images also need:

- `INTERLACE_MODE = "INTERLACE_PIXEL"` or
- `INTERLACE_MODE = "INTERLACE_PLANE"`

If that attribute is missing for formats that require it, h5v will not treat the dataset as an inline image.

## Shape expectations

| Image kind | Expected shapes |
| --- | --- |
| Grayscale | `[height, width]` or frame-first variants such as `[frames, height, width]` |
| Bitmap | `[height, width]` |
| Truecolor, pixel interlace | `[height, width, channels]` or `[frames, height, width, channels]` |
| Truecolor, plane interlace | `[channels, height, width]` or `[frames, channels, height, width]` |

The exact dataset interpretation depends on the subclass plus the interlace mode.

## Pannable wide or tall images

When an image would be clipped heavily by the current terminal aspect ratio, h5v switches from a simple fit-to-area preview to a windowed image view. The image chrome then shows which row or column range is currently visible, and the hidden portion can be explored like a pannable image slice.

In practice this works especially well for unusually wide or tall datasets. The bundled example path `/images/wide_grayscale` is intended to demonstrate that behavior.
