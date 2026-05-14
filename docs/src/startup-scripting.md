# Startup scripting

![Bundled scripting and code-style datasets](./assets/code.png)

## Sources

1. `--script <PATH>`
2. `--script -`
3. piped stdin
4. repeated `--command` / `-c`

Order: scripts first, then stdin, then inline commands.

Startup scripts use the same command parser as the minibuffer. That includes:

- navigation commands
- view and focus commands
- attribute commands
- heatmap range commands
- multichart commands
- `press <keys>` sequences

Heatmap can be scripted with generic movement commands, `heatmap range ...` commands, and `press` for zoom, reset, clear selection, and viewport pan.

`press` uses the effective keymaps after config load, so startup scripts follow any configured key remaps.

## Validation mode

Use `--script-test` or `-ct` to validate a script without launching the UI:

```bash
h5v file.h5 --script-test --script setup.h5v
```

## Bundled example script

```bash
h5v examples/h5v-example.h5 --script examples/h5v-example.h5v
```

Regenerate the example file if needed:

```bash
python scripts/generate_example_h5.py
```

## Script format

- newline-separated commands
- semicolon-separated commands
- blank lines
- comment lines beginning with `#`

Examples:

```bash
h5v file.h5 -c "focus content" -c "mode matrix"
h5v file.h5 --script setup.h5v
printf 'toggle-tree; mode preview\nreload\n' | h5v file.h5
```

Example `setup.h5v`:

```text
# open with a clean content layout
toggle-tree
focus content
mode preview
mchart add !/group/dataset[..,0]
```

Example heatmap script fragment:

```text
focus content
mode heatmap
heatmap range add 5% 80% "5-80%"
page-down
press z
press L
press v
```

Bundled `examples/h5v-example.h5v`:

```text
goto /signals/sine_wave
focus content
mchart add
goto /signals/cosine_wave
mchart add
mchart open
mchart select prev
mchart visible
mchart select next
mchart prompt
mchart expr "$1 - $2"
mchart zoom in 20
```

See [Command reference](./command-reference.md) for the full command surface.
