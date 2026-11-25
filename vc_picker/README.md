# vc_picker

A fuzzy picker drawn below the shell prompt, for the Tab completion and key bindings of Rust CLIs: rows in columns with a header, type to filter with fzf's syntax (`'exact`, `^start`, `end$`, `!not`), the matched letters lit, a line about the selected row. It draws on `/dev/tty`, so it works from inside a completion function.

- `vc_picker::run` draws the picker and returns the row picked.
- `vc_picker::tab` runs it inside a `vc_complete` completer when a value has several hits, and gives the one candidate picked; where it cannot run (no terminal, `TOOL_TAB_PICKER=0`) the completer gives its plain candidates.
- A key binding (Alt+D) set in the shell config block of `vc_complete::rc` opens it from the prompt.

The rules are in the CLI UX guide, [docs/ux-guide.md](../docs/ux-guide.md) section 5.1. The example CLI `pickdemo` shows all three.
