# CLI UX guide: how the vhrdtech command-line tools look and behave

This page is for every command-line tool in the vhrdtech repos: `ww`, `dg`, `gw121`, `saleae`, `m2k`, `mdcheck`, `choreograph`, `solidcraft`, our other internal CLIs, and any new Rust CLI. Read it before building or changing a CLI. The vhrd_cli crates are the shared code that follows it.

It collects what was scattered:
- the house rules on CLIs, Rust repos and development practice;
- the AGENTS.md files of wire_weaver, typec_dongle (with its CLI design, `docs/design/cli/index.md`), gw121, saleae_rs, m2k and mdcheck, and other internal CLIs;
- an internal CLI's FEATURES.md area `CLI` and the picker (LIB-11);
- tasks and notes about completion, pickers, help, colour, progress, versions and confirmation;
- outside practice: clig.dev, the POSIX Utility Syntax Guidelines, the Heroku CLI style guide, no-color.org and BSD sysexits.

Its GUI twin is the [UX guide in vhrd_egui](https://github.com/vhrdtech/vhrd_egui/blob/main/docs/ux-guide.md). The rules every UI shares live once, in [its section 2](https://github.com/vhrdtech/vhrd_egui/blob/main/docs/ux-guide.md#2-rules-for-every-ui); section 2 here gives their CLI form.

How to read it:
- Each rule is a bold line, then **Why**, **Example** and its source in brackets. The keys are listed under "Sources" at the end.
- 🔒 marks a house rule: the guide restates it and must not drift from it. When they differ, the house rule wins and this page gets fixed.
- 📘 marks a convention one of the repos already follows, which this page makes the default for all.
- 💡 marks a point borrowed from outside practice: recommended, not a rule yet.
- 🔜 marks a part that is planned, not built yet, with its task.
- Feature IDs (`CLI-12`, `LIB-11`, ...) point into the named repo's FEATURES.md; task ids are references into the internal project tracker.

## 1. Short version

1. 🔒 **Completion is the default.** bash, zsh and fish complete live values with a help text each (ids with titles, devices, files), not just flags; never an error, at worst no candidates.
2. 🔒 **Help explains.** `-h` is one line per item; `--help` says what a command is for, what it reads and writes, and gives examples.
3. 🔒 **Colour helps reading** (statuses, ids, dates, warnings, errors, `next:` hints) and is off when not a TTY, with `NO_COLOR` or `--color never`.
4. 🔒 **Anything over a few seconds shows progress**, and is quiet when not a TTY: one line per step for logs and agents.
5. 🔒 **`--version` shows version, git SHA and commit time.**
6. 📘 **stdout is data, stderr is everything else.** Errors, progress and warnings never mix into what a pipe or an agent reads.
7. 📘 **Every command has a machine-readable output** (`--json`), and that output is an interface: changed on purpose, with a CHANGELOG entry.
8. 📘 **Errors start with the tool's name and say what to do.** Exit codes are a documented contract.
9. 🔒 **Nothing is hidden.** A list cut for room says what was left out and offers the rest (`--all`).
10. 🔒 **Disabled says why.** A menu entry or action that can't run now is shown with the reason and what would enable it.
11. 💡 **Prompts only at a terminal**, always with a flag that answers them; dangerous steps confirm, and `--dry-run` shows the plan first.
12. 🔒 **Commands register themselves** in their own module; help, completion and usage signals come from that one registry.
13. 🔒 **Every invocation records its usage** per subcommand, and each learnable feature lands in the skill tree.
14. 🔒 **Deploys read like a good installer**: a step per node, times, real progress bars and a summary table.

## 2. Rules for every UI

The rules every UI shares live once, in [vhrd_egui's UX guide, section 2](https://github.com/vhrdtech/vhrd_egui/blob/main/docs/ux-guide.md#2-rules-for-every-ui): nothing is hidden, disabled things say why, measure the input then confirm, say why when doing something non-obvious, and plain words in the person's language. Read them there. This section gives only their CLI form.

- 🔒 **Nothing is hidden: in a CLI the rest is offered, never a bare count.** A list cut for room says what it left out and the flag that shows it (`--all`). Nothing is dropped silently.
  - Why: a bare count makes the person run a second command or guess, and an agent cannot open what was never printed.
  - Example (the shape to aim for): a session list ends with `4 landed sessions not shown (see --all)`, never with a bare `… 4 more`.
  - [rule: "Nothing is hidden"; vhrd_egui ux-guide section 2]
- 🔒 **Disabled things say why: in a CLI as a short line.** A menu entry, picker row or completion candidate that can't be used now stays in the list, with the reason and what would enable it in its help text.
  - Why: a missing command looks like a missing feature, and the person never learns what to change.
  - Example (the shape): `land   not now: 2 uncommitted files in the worktree (commit them, or close the session first)`.
  - [rule: "Disabled things say why"; vhrd_egui ux-guide section 2]
- 🔒 **Measure the input, then confirm: in a CLI as a `calibrate` or `setup` subcommand.** It asks for a few motions one at a time (hold about 2 s, release about 2 s), records the raw input with timestamps, works out the mapping and prints a table of expected behaviour for the person to confirm. It is a step of the tool, never a one-off script.
  - Why: people describe motion badly; data plus one confirmation is right the first time.
  - Example: `tool calibrate joystick` prints "push the stick fully right and hold", records, then shows `axis 0 → X, +right` and asks "Does this match? [y/N]".
  - [rule: "Measure the input, then let the person confirm"; vhrd_egui ux-guide section 2]
- **Say why: in a CLI as a reason on the step line.**
  - Why: an action with no reason looks like a bug.
  - Example: `installing ve_theme 0.32 (the gallery needs THM-15)`; `skipped: server-a, server-b (servers: monitoring only)` (an internal CLI's CLI-11).
  - [vhrd_egui ux-guide section 2; an internal CLI's CLI-11]
- **Plain words: ids come with their names.** Output a person reads names a thing, not only its internal id. CLIs are written in English; text for people who read another language goes through locale files.
  - Example: a task id shown together with its project name, never a bare id; completion shows a task ref with its title.
  - [vhrd_egui ux-guide section 2; rule: "Project IDs carry their slug"]

## 3. Shape of the command line

- 💡 **Commands are `noun verb`, short and lowercase, the same verbs for every noun.** A noun with one obvious action may be a bare verb.
  - Why: `--help` at each level then reads like a map, and the same verb means the same thing everywhere.
  - Example: `dg power on`, `dg gpio read 3`; not `update` beside `upgrade`.
  - [clig "Subcommands"; dg design "Naming"]
- 💡 **Flags over positional arguments**, except for the one main thing a command acts on (`cp SRC DST` style).
  - Why: flags are self-describing and complete better: the shell knows which value it is completing.
  - Example: adding a task keeps its id as the one positional argument and the rest (status, title) as flags.
  - [heroku "Flags and args"; clig "Arguments and flags"]
- 💡 **Standard flag names, standard meanings.** `-h/--help` (only help), `-V/--version`, `-v/--verbose` (repeatable), `-q/--quiet`, `-n/--dry-run`, `-a/--all`, `-o/--output`, `-f/--force`, `--json`, `--color`, `-y/--yes`. A flag means the same in every subcommand of a tool and across the tools.
  - Why: a person guesses a flag without reading help, and two tools with the same flag behave the same.
  - Example: `-d/--device` selects a device in `dg`; `ww` uses the same idea with its `--serial` and `WW_*` variables.
  - [clig "Use standard names for flags"; dg design "Global flags"]
- 💡 **POSIX syntax:** single-letter short options may be grouped (`-qv`), `--` ends the options, `-` as a file name means stdin or stdout, and the order of options does not matter. Global flags work before and after the subcommand.
  - Why: scripts and fingers both expect it, and pressing ↑ then adding a flag at the end must work.
  - Example: `dg i2c write 0x50 - < data.bin`; `tool search -- -foo` passes a dash-prefixed search term through.
  - [posix Guidelines 5, 10, 11, 13; clig "make arguments, flags and subcommands order-independent"]
- 💡 **No catch-all subcommand and no prefix abbreviations.** Aliases are explicit and stay stable.
  - Why: once `tool i` means `install`, no command starting with `i` can ever be added.
  - Example: a short alias like `d` is explicit for a longer command, not a prefix match.
  - [clig "Future-proofing"]
- 💡 **Secrets never come from a flag value.** Take them from a file (`--token-file`, mode 0600) or stdin.
  - Why: a flag value lands in `ps`, shell history and logs.
  - Example: the records admin binary reads its token only from a 0600 `--token-file`.
  - [clig "Do not read secrets directly from flags"; an internal records tool's REC-70]
- 📘 **One front end, plugins on `PATH`.** A tool with outside extensions runs `TOOL-NAME` from `PATH` for an unknown `TOOL NAME`, cargo-style, and hands it the resolved context (device selection, output format) as `TOOL_*` environment variables. Plugins show in `TOOL help` and in completion.
  - Why: one thing to install and discover, while pieces ship on their own.
  - Example: `ww chunky ...` runs `ww-chunky` with `WW_SERIAL=...`; `donguru-vh-scan` runs as `dg vh-scan`.
  - [wire_weaver `wire_weaver_cli/src/plugin.rs`; dg design "Extensibility"]
- 🔒 **Commands register themselves.** A CLI with more than a handful of subcommands lets each command group register itself in its own module (a distributed slice with linkme, or inventory, on clap's builder API), never in one central enum or match that every change edits. A test checks the registered commands against the expected list. Help, completion and usage signals come from the same registry.
  - Why: main.rs changed in 67 % of landings on one internal CLI while the tree was central; one registry also keeps help, completion and usage from drifting apart.
  - Example: one internal CLI's CLI-12: each family's clap enum, dispatch, long-help rows, usage-signal rows and completers live in its own module's `cli`, described by a `cli_slice::Slice`.
  - [rule: "Commands register themselves"; an internal CLI's CLI-12]
- 🔒 **The CLI is thin over a library.** A tool that drives an instrument, device or service keeps its logic in a typed library crate; the CLI is for quick tests and simple scripts, and complex flows live in the library and the Python wrapper, not in CLI flags.
  - Why: a test bench, a notebook and the CLI then all get the same behaviour.
  - Example: `gw121` (lib), `gw121_cli`, `gw121_py`.
  - [rule: "Tools are a library first"]

## 4. Help and discovery

- 🔒 **`-h` is short, `--help` explains.** `-h` gives one line per command and argument. `--help` says what a command is for, what it reads and writes, and ends in an `Examples:` block. Every argument has a help line.
  - Why: `-h` is the quick reminder; `--help` is the documentation that always matches the installed version.
  - Example: a `--help` on a session-spawning command names the files and records it touches and shows three real command lines.
  - [rule: "CLIs are high quality"; an internal CLI's CLI-3; clig "Help"]
- 📘 **A test fails when help is missing.** A command without `about`, `long_about` or (for a leaf) an `Examples:` block, or an argument without help, fails the test suite.
  - Why: help written later is help never written.
  - Example: one internal CLI's CLI-6 help-coverage, in the `main` tests and `help.rs`.
  - [an internal CLI's CLI-6]
- 💡 **`-h` works anywhere on the line** and ignores the other arguments.
  - Why: the person adds `-h` to the line they were typing.
  - Example: `tool deploy service --target prod -h` shows the help of `deploy`, it does not deploy.
  - [clig "Help"]
- 📘 **Say what to run next.** After a state view or a write, a `next:` hint names the command that usually follows. The hint is coloured; when stdout is not a terminal, a hint that is not the asked data goes to stderr so the data stays the same for bots.
  - Why: workflows are learned from the tool itself.
  - Example: `next: mark the session ready --note "..."`; adding a task without a release hints at the open releases on stderr when not at a terminal (TASK-8).
  - [rule: "Colour" names `next:` hints; an internal CLI's TASK-8; clig "Suggest commands the user should run"]
- 💡 **On a typo, suggest; don't guess and run.** clap's "a similar argument exists" is the floor.
  - Why: a wrong word may be a logic mistake, not a typo, so running the guess can do harm.
  - Example: `tip: a similar argument exists: '--prio'`.
  - [clig "If the user did something wrong and you can guess what they meant, suggest it"]

### 4.1 🔜 Planned, not built yet: help lookup

- **A fuzzy search over the whole command tree.** A key (Alt+H is the working name) or a help-completion keystroke opens a picker over all commands, flags and their help texts, with filters. Enter shows the full help, or puts the command on the line.
  - Why: a CLI with hundreds of commands can't be learned from `--help` level by level.
  - Example: Alt+H, type `release date`, the rows show matching commands with their one-line help; Enter puts the chosen one on the line.
  - It is built on the command registry (section 3) and the generic picker (section 5.2), so every CLI can get it, not only the one it was first built for.
  - [synergy with the Tab picker (section 5.1) and the Alt+D picker (section 5.3)]

## 5. Shell completion

- 🔒 **Completion for bash, zsh and fish is the default for every CLI, with live values.** It completes project and task ids with their titles, names, devices and files, each with a help text, not just flags. When touching a CLI that lacks it, add it.
  - Why: completion is where people discover values; a bare list of flags teaches nothing.
  - Example: completing a task id lists that project's open tasks with titles; `ww --serial <TAB>` lists connected devices with product and label.
  - [rule: "CLIs are high quality"]
- 🔒 **Rust: `clap_complete` dynamic completion** (`CompleteEnv`, feature `unstable-dynamic`) with `ww`'s bash adapter, registered with `source <(COMPLETE=bash <bin>)`. The binary answers its own completion, so it always matches the installed version.
  - Why: the adapter does what clap's own bash adapter does not: quoting values with spaces, and bash's word breaks at `@`, `:` and `=` (`--api name@^0.1`).
  - Example: `wire_weaver_cli/src/complete.rs` is the model; gw121, saleae_rs, m2k, mdcheck and other internal CLIs carry copies.
  - [rule: "CLIs are high quality"; wire_weaver `complete.rs`]
- 📘 **Completion never fails loudly.** Any error gives no candidates. The bash registration has no `-o bashdefault`, so an error never falls back to file names.
  - Why: an error message in the middle of a typed line breaks the line; file names offered for an id are wrong answers.
  - Example: a device that can't be listed gives an empty list, not a backtrace.
  - [an internal CLI's AGENTS.md "Conventions", CLI-7; wire_weaver `complete.rs`]
- 📘 **Slow values come from a cache.** A value that takes seconds to list (a BLE scan, a network query) is cached by the command that finds it, and completion reads the cache.
  - Why: a Tab press must answer at once.
  - Example: `gw121 scan` caches its results for `--address` completion.
  - [gw121 AGENTS.md]
- 📘 **Plugins complete their own arguments,** but only after they answered a probe. `ww` runs `ww-NAME --ww-plugin` and passes the typed words only to a program that answers `ww-plugin 1`.
  - Why: an unrelated program named `ww-something` is never run with the user's words.
  - [wire_weaver `plugin.rs`]
- 📘 **Completion is tested in the real shells.** Tests drive bash the way readline does (`COMP_LINE`, `COMP_WORDS`), and bash, zsh and fish in a pty where they are installed; a missing shell is skipped with a line.
  - Example: one internal CLI's `tests/completion.rs`, `tests/complete_shells.rs`.
  - [an internal CLI's CLI-7]
- 🔜 **One shared adapter crate** instead of copies of `ww`'s bash adapter: `vc_complete` in this repo (v0.1). Until then a copy says where it came from and is kept in sync.

### 5.1 Built: the picker on Tab (LIB-11)

- 📘 **When a value has several hits, Tab opens a picker instead of a bare list.** It is drawn inline below the prompt on `/dev/tty` (like fzf `--height`, about 60 % of the terminal, not the alternate screen), so it works inside a completion function or a key binding. Rows have clear columns and a header, the matched letters are lit, a bottom line shows the details of the selected row.
  - Keys: type to filter, Up/Down, Ctrl-P/N, Tab, PgUp/PgDn, Enter takes it, Esc/Ctrl-C/Ctrl-G cancels.
  - Matching: the tool's own ranking first, then fuzzy matching with fzf syntax (`'exact`, `^start`, `end$`, `!not`).
  - Where it can't run (no terminal) or with `TOOL_TAB_PICKER=0`, plain candidates come back.
  - Colour follows section 6.
  - The UI is generic (`vc_picker`-style: `Column`, `Item`, `Source`), so other CLIs can reuse it.
  - [an internal CLI's LIB-11, its picker module]

### 5.2 🔜 Planned, not built yet: the picker for every CLI and value

- **Completion shows Alt+D-style results whenever a value has several hits**, not only for documents: several devices, several tasks, several sessions. The picker has clear columns (for a device: serial, product, label, port) instead of a bare list.
  - Why: a list of serial numbers says nothing about which board is which.
  - Example: `ww --serial <TAB>` with three boards opens a picker with serial, product, label and USB path columns.

### 5.3 🔜 Planned, not built yet: the Alt+D picker as a setting

- **Alt+D is a setting that the provisioning step writes into its `~/.bashrc` completion block**, not an environment variable a person sets by hand.
  - Why: the block is rewritten, so a line put inside it by hand is lost. Today the enabling variable must come before the completion script is sourced, so the PC setup puts it above the block.
  - [rule: "Settings on a PC are provisioned, never written by hand"]

## 6. Output

- 📘 **stdout carries the data asked for; stderr carries diagnostics** (progress, warnings, errors, actions in flight).
  - Why: `tool ... | jq` and an agent reading stdout never see noise.
  - Example: `dg power meter -f json | jq .current_ma`.
  - [dg design "Unix-philosophy"; clig "The Basics"; heroku "Stdout/Stderr"]
- 🔒 **Colour for statuses, ids, dates, warnings, errors and `next:` hints, from one set of styles.** It is off when stdout is not a TTY, with `NO_COLOR` set to anything non-empty, or with `--color never`. Rust: `anstream`/`anstyle`, as clap does; no ncurses. Status emoji stay as they are.
  - Why: colour makes a view scannable; escape codes in a pipe or a log are garbage.
  - Example: one internal CLI's CLI-9: statuses green, yellow or red, ids one colour, dates dim, warnings yellow, errors red.
  - [rule: "Colour"; no-color; an internal CLI's CLI-9]
- 📘 **Column widths are measured on the visible text,** never on a string with escape codes or a wide emoji counted as one.
  - Why: otherwise coloured tables run out of line.
  - [an internal CLI's CLI-9, CLI-11]
- 💡 **Use few colours, and red only for errors.**
  - Why: too many contrasting colours compete for attention, and the eye goes to red first.
  - [heroku "Colors"; clig "Errors"]
- 📘 **Text output is short and line-oriented,** one record per line, units spelled out.
  - Why: agents read it, and `grep` should find a whole record on one line.
  - Example: `-0.0021 V`, `24.0 degC`; a table where each row stands alone.
  - [gw121, saleae_rs and m2k AGENTS.md; heroku "grep-parseable"]
- 📘 **Dates people read are `7 Oct 2026`;** tokens are `07oct2026`; ISO only in JSON, frontmatter and state.
  - [an internal CLI's AGENTS.md "Conventions"]
- 📘 **Every command has a machine-readable form.** JSON goes to stdout as one value; a streaming command writes one JSON object per line.
  - Why: scripts, bots and agents get a stable shape without parsing text.
  - Example: `gw121 read --json`; `dg gpio watch 3 -f jsonl >> gpio.log`.
  - [gw121, saleae_rs, m2k AGENTS.md; dg design "Unix-philosophy"; clig "Display output as formatted JSON if --json is passed"]
- 📘 **Output formats are an interface.** Views, log lines and JSON for the bots change on purpose, with a CHANGELOG entry. A JSON contract carries a version field (`v`). Text for people may change freely.
  - Why: the bots, the dashboards and scripts parse it; a silent change breaks them.
  - Example: the records admin binary's plan JSON: `v`, `ok`, exit 0/1/2/78.
  - [an internal CLI's AGENTS.md "Conventions"; an internal records tool's REC-70; clig "Future-proofing"; rule: "Compatibility only for what is in use"]
- 📘 **Strings from outside are escaped before they reach the terminal.** A device name, a label, an error text from another process or a page is printed with control characters and bidi overrides escaped; machine output relies on JSON's own escaping.
  - Why: a device could otherwise move the cursor, recolour the screen or make a line read in the wrong order.
  - Example: `\u{1b}` and `\u{202e}` are shown as text. Escape the outside piece first, then add the tool's own colour around it.
  - [wire_weaver `untrusted.rs`, TOOL-19]
- 📘 **A run at a terminal stands apart from the shell around it.** A long command opens with a rule (`tool deploy REPO · 10 Oct 2026 16:11 · on PC · tool VERSION`), indents its output, and closes with `═══ tool deploy REPO took 12s · ok ═══`. Notices that would come first wait until after the header. The frame appears only at a terminal, so scripts and agents keep their lines.
  - [an internal CLI's CLI-11]
- 💡 **Long output goes through a pager only at a terminal,** and only where a person reads a whole document.
  - [clig "Use a pager"]

## 7. Errors and exit codes

- 📘 **An error is one line on stderr that starts with the tool's name and says what to do.**
  - Why: the person reads the fix, not a backtrace; the prefix tells which tool spoke inside a pipeline.
  - Example (the shape): `tool: no task 2618#99 (tool ls PROJECT lists its tasks)`.
  - [an internal CLI's AGENTS.md "Conventions"; dg design "Errors under json"; clig "Catch errors and rewrite them for humans"]
- 📘 **No `unwrap` or `expect` on data from files, the network, a device or the user.** Errors carry context (`anyhow` with `.context(...)` in the CLI, typed errors in the library).
  - [AGENTS.md of several internal CLIs, including typec_dongle and gw121]
- 📘 **A failed step shows its tail and keeps the rest.** Captured output of a step is shown only on failure (or with `-v`): its last 40 lines, then the path of the whole log in a private file.
  - Why: the cause is usually at the end; the full log is one command away.
  - Example: `... 812 earlier lines, the whole output: $XDG_STATE_HOME/TOOL/step-logs/TIME-NODE-WHAT.log`.
  - [an internal CLI's CLI-11; clig "if there is an error, make sure you print out the logs"]
- 📘 **Exit codes are a documented contract.** 0 is success, 1 a failure with no more specific category, 2 bad arguments (clap's usage errors). A tool with categories scripts must tell apart documents them in one table and never renumbers one; a new category takes the next free number.
  - Example: `dg`: 3 no device, 4 device error, 5 timeout (`src/exit.rs`).
  - [typec_dongle `docs/design/cli/index.md` "Exit codes"; clig "Map the non-zero exit codes to the most important failure modes"]
- 🔒 **78 means "can't work until someone acts".** A binary that can't run with its config (a bad setting, a database newer than the binary, a missing secret) exits 78 with one plain line, and its unit doesn't retry it. 75 is a temporary failure worth retrying (the deploy drain). Both come from BSD sysexits (`EX_CONFIG`, `EX_TEMPFAIL`).
  - [rule: "Services fail loudly, never in a loop"; sysexits]
- 📘 **Under a JSON output, an error is one JSON object on one line on stderr,** naming the exit category, its code and the message.
  - Why: a JSONL stream that fails halfway leaves only whole records on stdout, and `| jq` fails on the exit code instead of a surprise object.
  - Example: `{"error":{"exit":"device_error","code":4,"message":"power meter: ..."}}`.
  - [dg design "Errors under json"]

## 8. Long runs: progress, deploys and transfers

- 🔒 **Progress for every step that can take more than a few seconds:** what runs, how far, how fast and how long is left (Rust: `indicatif`; copies with `rsync --info=progress2`). Quiet when not a TTY, so logs and agents get one line per step.
  - Why: a silent program looks broken; a bar in a log is garbage.
  - Example: uploads, downloads, builds over ssh, imports, scans, deploys to many machines.
  - [rule: "Progress"; clig "Show progress if something takes a long time"]
- 💡 **Say something within 100 ms,** before a network call starts.
  - [clig "Responsive is more important than fast"]
- 🔒 **Deploys and other multi-step, multi-node runs read like a good installer:** a header per step saying what it does and on which node (`[server-a] uploading a service, 0.19.0`), start time and duration, colour for ok / skipped / failed, real progress bars (not raw rsync or cargo lines), and a summary table at the end (node, before, after, result). The logic lives in a Rust command (`deploy`, `ship`); recipes just call it.
  - Example: a shared internal UI crate's `Step`, `confirm`, the summary `table`.
  - [rule: "Deploys"; an internal CLI's CLI-8]
- 📘 **Progress bars never break the final lines.** A step's last lines are printed while the bars are suspended, never through indicatif's own `println`.
  - Why: indicatif pads lines to the terminal edge, and lines with icons then ran together.
  - [an internal CLI's CLI-11 "Final lines"]
- 🔒 **Efficient transfer:** only what changed is sent, compressed over slow links (`rsync -z` to a temporary name, then `mv`, not `ssh cat > file`). Nothing is rebuilt or re-uploaded when nothing changed, and independent steps on several machines run in parallel.
  - [rule: "Efficient transfer"]
- 💡 **Ctrl-C works at once.** The tool says something right away, gives clean-up a time limit, and says what a second Ctrl-C does. The next run copes with a run that never cleaned up.
  - [clig "Signals and control characters", "Make it crash-only"]
- 💡 **Network calls time out,** with a sensible default and a setting to change it.
  - [clig "Make things time out"; rule: "No hardcoded constants"]

## 9. Prompts, confirmation and dry runs

- 💡 **Prompt only when stdin is a terminal, and never require a prompt.** Every answer can also be given as a flag. Without a terminal the tool stops with a line naming the flag.
  - Why: scripts, bots and agents have no keyboard.
  - Example: adding a task without a release asks at a terminal (Enter = brewing); without one it prints one stderr hint and goes on (TASK-8).
  - [clig "Interactivity"; heroku "Prompting"; an internal CLI's TASK-8]
- 💡 **Confirm by the size of the danger.** A small local change needs no prompt. A bigger or remote change, or a bulk change that can't be undone, asks `[y/N]` and offers a dry run. A severe one (wiping a server, a prod database) makes the person type the name of the thing, or pass `--confirm NAME` in a script.
  - Example: `dg header vbus on --allow-voltage 20V` warns and asks above 5.5 V (CLI-16).
  - [clig "Confirm before doing anything dangerous"; dg design]
- 📘 **`--yes` answers the ordinary prompts, not the prod gate for an agent.** An AI agent session's `--yes` is refused for a ship to a server today; for every prod deploy it is planned (🔜). Instead the tool prints the line to type at a terminal, or files an approval request for a person to act on. A deploy held for a config key this binary does not understand asks a person even with `--yes`.
  - Why: prod waits for a person's OK, and an agent must not give it for them.
  - [an internal CLI's `deploy.rs`, CLI-17; rule: "Asking the person"]
- 📘 **`-n/--dry-run` shows what would happen** and changes nothing. Sending and publishing tools show the recipient and the exact text first.
  - Example: `tool deploy --dry-run`; a messaging tool's `send ... -n` previews the message.
  - [clig "-n, --dry-run"]
- 💡 **A tool that changes a config file it does not own says so and asks,** and prefers a file of its own or a marked block.
  - Example: the provisioning step keeps its own `~/.bashrc` completion block.
  - [clig "If you automatically modify configuration that is not your program's, ask the user for consent"]

## 10. Configuration and environment

- 📘 **Settings come in layers; the later wins per key.** Built-in defaults, then the system file, the user file (XDG: `~/.config/TOOL/`), the project file found by walking up from the current directory, then `TOOL_*` environment variables, then flags. `TOOL config show --origin` says where each value came from.
  - [dg design "Configuration"; clig "Apply configuration parameters in order of precedence", "Follow the XDG-spec"]
- 🔒 **Every tunable is a setting,** declared in the tool's typed settings schema with its default and a doc line. Code holds only the default.
  - Why: a config portal shows and changes it without a form written by hand.
  - [rule: "No hardcoded constants"]
- 📘 **Config is read forward compatible.** A command that only reads skips a key it does not know, with one warning on stderr (file, key path, this version). `TOOL check` refuses it. Must-understand sections (rights, approvals, secrets) are held whole instead of guessed.
  - Why: a newer machine's config must not stop an older binary, and a typo must still be caught.
  - [an internal CLI's CLI-13 to CLI-17]
- 📘 **Device selection follows one order:** the flag, then its environment variable, then the project file, then auto-selection when exactly one device is present. An ambiguous selector is an error that lists the choices.
  - [dg design "Device selection"; `ww` `DeviceArgs`]

## 11. Version, usage and the skill tree

- 🔒 **`--version` shows version, git SHA and commit time:** `tool 0.4.2 (a1b2c3d-dirty, commit 3 Oct 2026 18:20)`. Rust: a small `build.rs` without extra crates (`git rev-parse --short HEAD`; the commit time from `git log -1 --format=%ct`; `-dirty` when `git status --porcelain` isn't empty; worktree-safe `rerun-if-changed` on the real git dir's `HEAD` and `index`; `unknown` without git). An internal CLI's is the model, and `vc_build` in this repo is its shared form (v0.1). Python reads the SHA and commit date at run time; firmware reports through bedrock `fw_info`. A CLI that still says `built` is changed when touched.
  - Why: the same SHA gives the same line, and the installer and the deploys parse this line to tell releases apart.
  - [rule: "--version shows version, git SHA and commit time"]
- 🔒 **Every invocation records its usage** per subcommand, and per flag with a meaning of its own, in the binary itself, locally and cheaply: from a terminal, a script or an agent session.
  - Why: parts nobody uses can be found and removed.
  - Example: an internal CLI's own usage recorder (signals like `d picker`).
  - [rule: "Every invocation records its usage"]
- 🔒 **Every feature a user can learn lands in the skill tree** in the same piece of work: its usage signal, its skill in `skilltree.toml`, its strings in every `locales/<lang>/skilltree.ftl`. When that doesn't fit, a task names what is missing.
  - [rule: "Every feature a user can learn lands in the skill tree"]

## 12. Testing a CLI

- 📘 **Help coverage:** a test fails on a command or argument without help, or a leaf without examples (an internal CLI's CLI-6).
- 📘 **Completion:** driven the way readline does, and in real shells in a pty (an internal CLI's CLI-7).
- 📘 **Colour:** per command family, the plain output has no escape codes, and the coloured output minus its escapes equals the plain output (an internal CLI's CLI-9, `tests/colour.rs`).
- 📘 **Terminal behaviour:** frames, progress and final lines tested at a real pty at a fixed width (an internal CLI's CLI-11, `tests/frame.rs`).
- 📘 **Exit codes and JSON:** the built binary runs against fixtures and the test checks the exit code and the JSON shape (mdcheck `tests/check.rs`, dg's exit table).
- 🔒 **Registry:** a test checks the registered commands against the expected list (section 3).

## 13. Checklists

**A new CLI:**
- a library crate first, the CLI thin on top (section 3);
- commands registered per module, with the registry test;
- `-h` and `--help` with examples, and the help-coverage test;
- completion for bash, zsh and fish with live values and the shared bash adapter;
- colour from one style set, off without a TTY, with `NO_COLOR` or `--color never`;
- stdout for data, stderr for the rest, a JSON form for every command;
- errors prefixed with the tool's name, documented exit codes;
- progress bars for long steps;
- `--version` from the worktree-safe `build.rs`;
- usage signals and skill-tree entries;
- the tests of section 12.

**Touching an existing CLI:** add what is missing from the list when the touch is near it, or file a task for it.

## Sources

- [clig] Command Line Interface Guidelines, https://clig.dev/, read 3 Nov 2026. Sections cited by name.
- [heroku] Heroku CLI Style Guide, https://devcenter.heroku.com/articles/cli-style-guide, read 3 Nov 2026.
- [posix] POSIX.1-2024, XBD chapter 12.2 Utility Syntax Guidelines, https://pubs.opengroup.org/onlinepubs/9799919799/basedefs/V1_chap12.html, read 3 Nov 2026.
- [no-color] https://no-color.org/, read 3 Nov 2026: `NO_COLOR` present and not empty, whatever its value, turns colour off.
- [sysexits] BSD sysexits(3), https://man.freebsd.org/cgi/man.cgi?query=sysexits, read 3 Nov 2026: 64 `EX_USAGE`, 75 `EX_TEMPFAIL`, 78 `EX_CONFIG`.
- GNU Coding Standards, "Command-Line Interfaces" (https://www.gnu.org/prep/standards/html_node/Command_002dLine-Interfaces.html): not read for this version, so no point here rests on it. Worth a read for `--help` and `--version` output conventions.
- [rule: ...] the house rules on CLIs and UIs, Rust repos and development practice, as of 3 Nov 2026.
- Repos: wire_weaver (`wire_weaver_cli/src/complete.rs`, `plugin.rs`, `untrusted.rs`), typec_dongle (`docs/design/cli/index.md`, `src/exit.rs`, `AGENTS.md`), gw121, saleae_rs, m2k, mdcheck, choreograph, cnt, and our other internal CLIs and their AGENTS.md files.

## Where the rules point here

- **House rules:** "CLIs are high quality", "CLI quality", "--version shows version, git SHA and commit time", "Nothing is hidden", "Disabled things say why", "Measure the input", "Commands register themselves" and "Every invocation records its usage" are restated here.
- **Repo AGENTS.md files:** wire_weaver, typec_dongle, gw121, saleae_rs, m2k, mdcheck, choreograph, cnt, fpga_tools and our other internal CLIs point here for CLI work, as does this repo's [AGENTS.md](../AGENTS.md).
- **GUI twin:** [vhrd_egui's UX guide](https://github.com/vhrdtech/vhrd_egui/blob/main/docs/ux-guide.md) holds the rules every UI shares (its section 2); section 2 here links to it.
