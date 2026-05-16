# Commands

Press `:` to open the command minibuffer.

![Command minibuffer](./assets/cmd.png)

Minibuffer behavior:

- `Tab` cycles matching completions
- `Shift+Tab` and arrow keys move through suggestions
- `Ctrl+P` / `Ctrl+N` browse command history
- `help` or `help <command>` shows command help
- `.` repeats the last successful command

Use the in-app `Commands` help tab for the current command list and examples.

## Common commands

```text
goto /signals/sine_wave
mode preview
mchart add !/signals/sine_wave
mchart open
mchart fit all
configure
```

## Numeric shorthand

```text
:5
:+3
:-2
```

- `:5` -> `seek 5`
- `:+3` -> `down 3`
- `:-2` -> `up 2`

## Quoting

Use quoted strings for values with spaces, expression tuples, and `press ...` sequences.

```text
press ctrl+w o
press M j enter
```

`press` uses the effective keymaps after config load.

See [Startup scripting](./startup-scripting.md) for script files and automation.
