# vhrd_cli features and roadmap

This file is the single source of truth for what vhrd_cli does, what is broken and what is planned, for humans and AI agents alike. [CHANGELOG.md](CHANGELOG.md) records what changed and when; this file records the current state.

## How to use this file

- **Status** of each item:
  - ✅ done
  - 🚧 in progress or partially done (the note says what is missing)
  - 🐛 implemented, but with known bugs
  - ⬜ stub: types or API exist but do nothing yet
  - 📋 planned
  - 💡 idea, not committed to
  - ⛔ blocked (the note says on what)
  - 🔍 probably done or obsolete, needs a check before closing
- **IDs** (`COMP-1`, `UI-2`) are stable: never renumber or reuse one. New items take the next free number of their area. Use the ID in commit messages, CHANGELOG entries and code `TODO`s (`// TODO(COMP-1): ...`).
- Items are grouped by area. Each area lists what works first, then open items by priority.
- When finishing work, update the item in the same commit: mark it ✅, add a pointer (crate, module, test) and move it up to the done items of its area. Don't delete done items. Items that turn out obsolete go to [Dropped and superseded](#dropped-and-superseded) with a one-line reason.
- A bug you find but don't fix gets an entry (🐛 on the feature, or a new item) with what triggers it.

## Crates

None with code yet. The planned `vc_*` crates are listed under their areas below; `repo_checks` (not published) holds the repo-wide tests.

## GUIDE: the CLI UX guide

- ✅ **GUIDE-1 cli-ux-guide**: [docs/ux-guide.md](docs/ux-guide.md), how the CLIs look and behave, in sections from the command line's shape to testing, with a checklist for a new CLI. The rules every UI shares are linked from vhrd_egui's UX guide, section 2. Checked by `repo_checks/tests/docs.rs`.

## REPO: the repo itself

- ✅ **REPO-1 workspace-skeleton**: the Cargo workspace, dual MIT or Apache-2.0, the just recipes `test` (with `--changed`), `lint`, `install` and `deploy`, nextest with a slow-test kill, the Rust build profiles. `repo_checks/tests/docs.rs` keeps the guide's path, sections and links, and keeps private paths out of the Markdown.

## VER: the --version line

- 📋 **VER-1 vc-build**: `vc_build`, the `--version` line `tool 0.4.2 (a1b2c3d-dirty, commit 3 Oct 2026 18:20)` from a worktree-safe `build.rs` helper without extra crates (guide section 11). v0.1.

## UI: colour, errors and safe printing

- 📋 **UI-1 vc-ui**: `vc_ui`, one style set on anstyle/anstream that turns off without a TTY, with `NO_COLOR` or `--color never`; the error line prefixed with the tool's name; the exit code table; escaping of outside strings before they reach the terminal (guide sections 6 and 7). v0.1.

## COMP: shell completion

- 📋 **COMP-1 vc-complete**: `vc_complete`, bash, zsh and fish completion with live values and help texts on clap_complete's dynamic completion, the quoting bash adapter, and the rc block (guide section 5). v0.1.

## CMD: the command registry

- 📋 **CMD-1 vc-cmd**: `vc_cmd`, commands that register themselves per module (linkme), their help rows, the registry test and the help-coverage checks (guide sections 3 and 4). v0.2.

## USAGE: usage records

- 📋 **USAGE-1 vc-usage**: `vc_usage`, a local, cheap record of each invocation per subcommand and per meaningful flag (guide section 11). v0.2.

## STEPS: long runs

- 📋 **STEPS-1 vc-steps**: `vc_steps`, steps with headers, timings and progress bars (indicatif), tables measured on visible text, and the deploy summary (guide section 8). v0.2.

## TPL: the example CLI

- 📋 **TPL-1 vc-template**: `vc_template`, a small CLI that uses every piece and shows how a new CLI starts. v0.2.

## PICK: the picker

- 📋 **PICK-1 vc-picker**: `vc_picker`, the inline picker on Tab when a value has several hits, the Alt+D binding as a setting in the rc block, and the help lookup over every command (guide sections 4.1, 5.1 to 5.3). v0.3.

## Dropped and superseded

None yet.
