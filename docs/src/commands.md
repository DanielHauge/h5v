# Commands

Press `:` to open the command minibuffer.

![Command minibuffer](./assets/cmd.png)

Minibuffer behavior:

- `Tab` cycles matching completions
- `Shift+Tab` and arrow keys move through suggestions
- `Ctrl+P` / `Ctrl+N` browse command history
- `help` or `help <command>` shows command help
- `.` repeats the last successful command

## Common commands

Navigation:

```text
goto /signals/sine_wave
seek 5
down 3
left 2
page-down
```

View control:

```text
focus tree
focus attributes
focus content
mode preview
mode matrix
toggle-tree
reload
configure
configure reset
```

Selection:

```text
x next
row prev
col next
dim next
index next 10
```

Attributes:

```text
attr create title string "release candidate"
attr delete title
```

Multichart:

```text
mchart open
mchart add !/signals/sine_wave
mchart visible
mchart base toggle
mchart select next
mchart expr "($1, !/signals/cosine_wave)"
mchart derive difference
mchart zoom in 25
mchart pan right 10
```

For the full command list, aliases, and `mchart` action table, see [Command reference](./command-reference.md).

## Aliases and numeric shorthand

Numeric shorthand still works:

```text
:5
:+3
:-2
```

Mappings:

- `:5` -> `seek 5`
- `:+3` -> `down 3`
- `:-2` -> `up 2`

## Quoting

Quoted strings work in commands and scripts. Use them for:

- attribute values with spaces
- command scripts containing expression tuples
- `press ...` commands that need modifier sequences

`press` goes through the normal keymap:

```text
press ctrl+w o
press M j enter
```

Try commands against the bundled example:

```bash
h5v examples/h5v-example.h5
```

See [Configuration and theming](./configuration.md) and [Startup scripting](./startup-scripting.md).
