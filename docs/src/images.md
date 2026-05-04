# Images

## Supported image families

![Image preview modes](./assets/images.png)

h5v recognizes these HDF5 image subclasses:

| Image subclass | Notes |
| --- | --- |
| `IMAGE_GRAYSCALE` | Standard grayscale image data |
| `IMAGE_BITMAP` | Two-dimensional bitmap data |
| `IMAGE_TRUECOLOR` | Color images with explicit interlace mode |
| `IMAGE_INDEXED` | Indexed images with explicit interlace mode |
| `IMAGE_JPEG` | Raw JPEG byte payloads stored in HDF5 |
| `IMAGE_PNG` | Raw PNG byte payloads stored in HDF5 |

When the metadata matches the HDF5 image convention, h5v renders the dataset inline instead of treating it as a plain numeric matrix.

## Terminal support

Image previews are best in terminals with a full graphics protocol. In practice, Kitty is the strongest target and gives the cleanest inline rendering experience.

- Kitty graphics protocol: <https://sw.kovidgoyal.net/kitty/graphics-protocol/>
- ratatui-image backend support: <https://github.com/ratatui/ratatui-image>
- ratatui-image terminal screenshot matrix: <https://benjajaja.github.io/ratatui-image-screenshots/>

h5v uses `ratatui-image` for inline image rendering and terminal capability detection. That means support quality follows the terminal/backend combination that `ratatui-image` can drive, including Kitty, Sixel, and iTerm2-style environments.

## Raw JPEG and PNG payloads

h5v supports raw encoded image payloads stored as:

- `u8` byte streams
- variable-length arrays of `u8`

That makes it possible to keep encoded image assets inside HDF5 and still inspect them from the terminal.

## Multi-frame image datasets

Image navigation works for multi-frame data as well:

- grayscale and truecolor image stacks can use the leading dimension as a frame axis
- raw JPEG and PNG payloads can be stored as variable-length byte arrays
- frame movement is clamped to the available range

## Viewport behavior

The image renderer uses smart scaling so the preview fills the available terminal area without clipping too aggressively. Scroll and paging controls work well when the content area is smaller than the image or when you are moving through frame stacks.

For very wide or tall datasets, h5v switches to a windowed image viewport. That mode now shows a compact two-line viewport HUD with:

- the currently visible row or column range
- total coverage percentage
- how many rows or columns are visible
- how far each arrow-key press pans the viewport

The bundled example file includes both `/images/truecolor_rgb` for a normal inline image and `/images/wide_grayscale` for a very wide dataset that demonstrates the windowed image view.

## Related chapters

- [Image conventions](./image-conventions.md)
- [Preview types](./previews.md)
- [Troubleshooting and limits](./troubleshooting.md)
