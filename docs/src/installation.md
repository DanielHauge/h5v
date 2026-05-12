# Installation

## Recommended: shell installer

Linux and macOS can install the latest release directly from GitHub:

```bash
curl -fsSL https://raw.githubusercontent.com/DanielHauge/h5v/main/install.sh | sh
```

The shell installer also works in Git Bash, MSYS2, and Cygwin on Windows. It prefers conventional install locations: `/usr/local/bin` when writable, `~/.local/bin` on Unix-like systems without a writable system prefix, and `%LOCALAPPDATA%\Programs\h5v\bin` on Windows shells.

Options:

```bash
install.sh --version VERSION --install-dir PATH --repo OWNER/REPO --dry-run
```

## Other install paths

### PowerShell

```powershell
irm https://raw.githubusercontent.com/DanielHauge/h5v/main/install.ps1 | iex
```

### Homebrew

```bash
brew tap DanielHauge/h5v https://github.com/DanielHauge/h5v.git
brew install h5v
```

### Scoop

```powershell
scoop bucket add h5v https://github.com/DanielHauge/h5v
scoop install h5v/h5v
```

### Prebuilt binaries with cargo-binstall

```bash
cargo binstall h5v
```

### Build from source

```bash
cargo install h5v
```

On Linux, source builds may require native packages such as `cmake`, `pkg-config`, `libfontconfig`, `freetype`, and `expat` development headers.

## Terminal graphics

h5v works in plain terminals, but image and chart previews look best with a real graphics protocol such as Kitty.

Links:

- Kitty graphics protocol: <https://sw.kovidgoyal.net/kitty/graphics-protocol/>
- ratatui-image: <https://github.com/ratatui/ratatui-image>
- terminal support gallery: <https://benjajaja.github.io/ratatui-image-screenshots/>

If graphics probing causes trouble:

```bash
h5v --no-terminal-graphics path/to/file.h5
```

That disables graphics probing and uses the safer text-only path.

## Compatibility mode

If your terminal also struggles with icons or line drawing:

```bash
h5v --compatibility path/to/file.h5
```

This switches to simpler symbols and disables terminal graphics probing.

You can also enable it with `H5V_COMPATIBILITY_MODE` or `h5v.compatibility = true`.

```bash
H5V_COMPATIBILITY_MODE=true h5v path/to/file.h5
H5V_COMPATIBILITY_MODE=off h5v path/to/file.h5
```

Precedence:

1. `--compatibility`
2. `h5v.compatibility`
3. `H5V_COMPATIBILITY_MODE`
4. default `false`

For config details, see [Configuration and theming](./configuration.md). For display problems, see [Troubleshooting and limits](./troubleshooting.md).

To make the environment variable permanent:

```bash
# ~/.bashrc or ~/.zshrc
export H5V_COMPATIBILITY_MODE=true
```

```fish
# ~/.config/fish/config.fish
set -gx H5V_COMPATIBILITY_MODE true
```

Invalid values are rejected.

## Write mode

h5v opens files read-only unless you pass `-w`:

```bash
h5v -w path/to/file.h5
```

Without `-w`, edit actions report that the file must be reopened in write mode.
