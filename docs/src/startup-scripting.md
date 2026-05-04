# Startup scripting

![Bundled scripting and code-style datasets](./assets/code.png)

## Available sources

h5v can execute startup commands from several places:

1. `--script <PATH>`
2. `--script -`
3. piped stdin
4. repeated `--command` / `-c`

Scripts are collected first, then stdin, then inline commands.

## Supported command surface

Startup automation uses the same command parser and catalog as the interactive minibuffer. That means startup scripts can use:

- navigation commands
- view and focus commands
- attribute commands
- multichart commands
- `press <keys>` sequences

## Validation mode

Use `--script-test` or `-ct` to parse and summarize a startup script without launching the UI:

```bash
h5v file.h5 --script-test --script setup.h5v
```

This is the quickest way to validate a scripted workflow before handing it to someone else or checking it into a repository.

## Bundled example script

This repository ships a ready-to-run example:

```bash
h5v examples/h5v-example.h5 --script examples/h5v-example.h5v
```

The script opens the bundled signal datasets, builds a small multichart workspace, marks a base series, and creates a derived difference series so users can see a scripted workflow immediately.

The example file and example script are meant to stay in sync. If you change the data layout or the walkthrough, regenerate the file with:

```bash
python scripts/generate_example_h5.py
```

## Script format

Startup scripts accept:

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

Actual bundled `examples/h5v-example.h5v`:

```text
goto /signals/sine_wave
focus content
mchart add
goto /signals/cosine_wave
mchart add
mchart open
mchart select prev
mchart visible
mchart base toggle
mchart select next
mchart visible
mchart derive difference
mchart zoom in 20
```
