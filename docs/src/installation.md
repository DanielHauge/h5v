# Installation

## Recommended: shell installer

Linux and macOS can install the latest release directly from GitHub:

```bash
curl -fsSL https://raw.githubusercontent.com/DanielHauge/h5v/main/install.sh | sh
```

The shell installer also works in Git Bash, MSYS2, and Cygwin on Windows. It prefers conventional install locations: `/usr/local/bin` when writable, `~/.local/bin` on Unix-like systems without a writable system prefix, and `%LOCALAPPDATA%\Programs\h5v\bin` on Windows shells. On Windows, the PowerShell installer or Scoop is usually the more natural choice.

The installer also supports:

```bash
install.sh --version VERSION --install-dir PATH --repo OWNER/REPO --dry-run
```

## Other install paths

### PowerShell

```powershell
irm https://raw.githubusercontent.com/DanielHauge/h5v/main/install.ps1 | iex
```

The PowerShell installer installs into `%LOCALAPPDATA%\Programs\h5v\bin` and adds that directory to the user `PATH`.

### Homebrew

```bash
brew install DanielHauge/h5v/h5v
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

## Terminal graphics support

h5v works in regular terminals, but **image previews and chart previews work best when the terminal supports a real graphics protocol**, especially the Kitty graphics protocol.

- Kitty graphics protocol: <https://sw.kovidgoyal.net/kitty/graphics-protocol/>
- ratatui-image project: <https://github.com/ratatui/ratatui-image>
- ratatui-image terminal screenshot matrix: <https://benjajaja.github.io/ratatui-image-screenshots/>

h5v uses `ratatui-image` to detect and drive terminal image backends such as Kitty, Sixel, and iTerm2. If no graphics protocol is available, preview quality depends on the available fallback path and will usually be less crisp than in Kitty-class terminals.

If your terminal shows a blank or badly garbled screen, start h5v with:

```bash
h5v --no-terminal-graphics path/to/file.h5
```

That disables terminal graphics probing and forces the safer text-only preview path. It is especially useful in browser-backed terminals and other xterm-like environments with partial graphics support.

## Write mode

h5v opens files read-only unless you pass `-w`:

```bash
h5v -w path/to/file.h5
```

Without `-w`, edit actions stay available in the UI but report that the file must be reopened in write mode before the change is applied.
