# Working on vhrd_cli

Guidance for AI agents and contributors. Read this before changing code.

vhrd_cli is the common platform for the Rust command-line tools in the vhrdtech repos: the CLI UX guide plus small shared crates (`vc_*`) for shell completion, the `--version` line, colour, errors, progress and deploy reports, the command registry, usage records and the picker. It is the CLI twin of [vhrd_egui](https://github.com/vhrdtech/vhrd_egui). The repo is public: see *Public repo* below.

## The CLI UX guide

[docs/ux-guide.md](docs/ux-guide.md) says how every CLI looks and behaves. Read it before building or changing a CLI, here or in a repo that uses these crates.

- The crates here are the code form of the guide. A crate that does something the guide does not say, or the other way round, gets one of them fixed in the same piece of work.
- The AGENTS.md files of the CLI repos link to `docs/ux-guide.md` by that path, and its sections by anchor: keep the path and the section headings (`repo_checks/tests/docs.rs` checks them). A new section goes at the end, before *Sources*.
- The rules every UI shares (nothing hidden, disabled says why, measure then confirm, say why, plain words) live once, in [vhrd_egui's UX guide, section 2](https://github.com/vhrdtech/vhrd_egui/blob/main/docs/ux-guide.md#2-rules-for-every-ui). Section 2 here gives only their CLI form and links there.
- The guide explains the rules as they are. Open questions and decisions still to be made stay out of the repo, in the project tracker.

## FEATURES.md is the source of truth

[FEATURES.md](FEATURES.md) lists every feature with its status, every known bug, and what is planned, with stable IDs per area (`COMP-1`, `UI-1`, ...).

- **Read the relevant area before starting.**
- **Name IDs with a short slug when talking to the user** (answers, plans, summaries): `COMP-1 vc-complete`, never a bare `COMP-1`. The slug is 2-4 kebab-case words from the item's title. Commit messages, CHANGELOG and code `TODO`s keep the bare ID.
- **Update it in the same commit** as the code: mark items ✅/🚧/🐛 with a pointer to the code, add bugs you find but don't fix (next free ID of the area), move obsolete items to *Dropped and superseded*. Never renumber or reuse IDs.
- Don't track status anywhere else (README checklists, TODO files). Code `TODO`s that matter reference an ID: `// TODO(COMP-1): ...`.

## CHANGELOG.md records every change

[CHANGELOG.md](CHANGELOG.md) is the history, FEATURES.md the current state; keep both.

- Every change a user of the crates or the guide would notice gets an entry under `## [Unreleased]` in the same commit: `### Added`, `### Changed`, `### Fixed`, `### Removed`. Short, with the feature ID in parentheses. A break is said in the entry itself: `**Breaking:** ...` under Changed or Removed.
- Pure refactors and typo fixes don't need an entry.
- Every version gets its own section, written by the landing (*Landing and versions*), so `[Unreleased]` on main stays empty apart from docs-only work.
- Questions like "what's new" or "what changed since X" are answered from CHANGELOG.md, newest sections first, with FEATURES.md for the current state.

## Layout

A Cargo workspace from the start, edition 2024, one shared version (`[workspace.package]`), dual-licensed MIT or Apache-2.0.

- `docs/ux-guide.md`: the CLI UX guide.
- `repo_checks/`: not published; tests only (`tests/docs.rs`): the guide's path, sections and links, and no private paths in any Markdown file.
- The `vc_*` crates join as they are written (FEATURES.md lists them): `vc_build`, `vc_ui`, `vc_complete` (v0.1), `vc_cmd`, `vc_usage`, `vc_steps`, `vc_template` (v0.2), `vc_picker` (v0.3).

**The workspace rule:** each crate does one job, so a CLI depends on only what it uses. Heavy parts (indicatif, ratatui with crossterm, a fuzzy matcher) sit in crates of their own and are never pulled in by a light crate such as `vc_build` or `vc_ui`. A crate that grows past about 20,000 lines, or holds parts that change at their own pace, is split before it grows on. Shared dependency versions go in the root `[workspace.dependencies]`.

## Commands

- `just test`: every test (cargo nextest, then the doc tests). `just test --changed`: only the crates changed against main and the crates that depend on them; run it before handing work over.
- `just lint`: clippy with warnings as errors, `cargo fmt --check`, `cargo machete`. It only checks, never edits.
- `just install`: build with the deploy profile (release, incremental). The crates are libraries, so nothing goes on PATH yet; when a binary comes (`vc_template`), `install` puts it on PATH exactly once: through our internal installer when it's available, otherwise `cargo install --locked --path CRATE`, never both.
- `just deploy`: refuse on local changes, `git pull --ff-only`, then `just install`.

Tools: [cargo-nextest](https://nexte.st/), [cargo-machete](https://github.com/bnjbvr/cargo-machete), and `tpm` for `just test --changed` (without it the full run).

## Dependencies

- Added with `default-features = false` and only the features in use.
- Pure-Rust crates only: no `-sys` crates, no bundled C, no `cc` or `bindgen` build steps. Check `cargo tree` when adding one. An exception needs a written reason here.
- Touching a Cargo.toml includes `cargo tree -d` for duplicated versions and `cargo machete` for unused ones; a big new dependency names its build cost (`cargo build --timings`) in the commit.
- The outside crates cover the low layers (clap and clap_complete with `unstable-dynamic`, anstyle and anstream, indicatif, ratatui with crossterm, nucleo-matcher, linkme); these crates are the glue on top.

## Code conventions

- Public API gets doc comments with an example; these crates exist to be reused.
- No `unwrap`/`expect` on data from files, the network, a device or the user. Library crates return typed errors.
- Markdown is not hard-wrapped: one line per paragraph and list item.

## Tests

Pure logic: unit tests. Terminal behaviour (colour, completion, frames): tests that drive the real thing, as the guide's section 12 says. Tests are deterministic (no real time decides an outcome), leave no trace on the machine (scratch directories only, never the real home or `/etc`), and a test that starts a binary clears the `CARGO_*` environment first. A bug fix starts with a failing test when the bug can be reproduced in one.

## Commits

Conventional Commits with the crate as scope: `feat(vc_ui): ...`, `fix(vc_complete): ...`, `docs: ...`, `build: ...`. Short imperative summary, blank line, body with what and why; reference feature IDs (`feat(vc_build): the --version line (VER-1)`). Each finished step is one commit with FEATURES.md and CHANGELOG.md updated in it.

## Landing and versions

Work happens on a session branch. It reaches main only through the maintainer's landing process, never a direct push or merge, and only when the maintainer agrees.

- A branch never touches the version: no manifest bump, no `## [x.y.z]` heading, no tag. Its CHANGELOG entries go under `[Unreleased]`.
- Landing rebases the branch onto main, runs `just lint` and `just test` as the gate, and makes one `release: x.y.z` commit that bumps `[workspace.package]` and turns `[Unreleased]` into `## [x.y.z] - date`, with an annotated tag `vX.Y.Z`. Tags are never moved.
- Before 1.0, patch for additive or compatible changes, minor only for a breaking one. From 1.0, minor for Added/Deprecated, patch for fixes, major only when the maintainer says so.
- Publishing a crate to crates.io waits for the maintainer's OK.

## Public repo

Everything committed here is public. Text in the repo (docs, READMEs, recipe comments, CHANGELOG, commit messages) says what happens in neutral words, and carries no private names, home paths, machine names or paths into private trackers; `repo_checks` catches the paths. Design notes, review rounds and open questions stay in the project tracker; what lands here is a docs page that explains a thing as it is. Test fixtures are the repo's own or truly open source.
