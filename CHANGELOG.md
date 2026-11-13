# Changelog

All notable changes to the vhrd_cli crates and docs. The format follows [Keep a Changelog](https://keepachangelog.com/), versions follow [Semantic Versioning](https://semver.org/). Feature IDs refer to [FEATURES.md](FEATURES.md).

## [Unreleased]

### Added

- `vc_complete`: bash, zsh and fish completion for clap CLIs with live values and help texts, on clap_complete's dynamic completion. One call at the top of `main`; a bash adapter that quotes values and handles the `@`, `:` and `=` word breaks without falling back to file names; zsh offers candidates that replace the typed word in a menu; fish takes a picker's choice. A failing or panicking completer gives no candidates and prints nothing. Helpers for candidates (`cand`, `filtered`, `values`) and the line being completed (`split_line`, `comp_words`, `word_after`), and the example CLI `vcdemo` (COMP-1).
- `vc_complete::rc`: the marked completion block in the bash, zsh and fish config files for a list of CLIs, written once, replaced when it changes, and taking the place of hand-written registration lines (COMP-2).
- `vc_ui`: colour from one style set with `--color auto|always|never`, off without a terminal or with `NO_COLOR`; paint helpers, `visible_width` and `strip_ansi`; the error line with the tool's name and the whole cause chain; the `Exit` type (0, 1, 2, tool categories 3 to 63, 75 and 78) and `finish` for the end of `main`; `Untrusted` escaping of outside strings; `restore_sigpipe` so `tool | head` ends quietly (UI-1).
- The CLI UX guide says the range of a tool's own exit categories, 3 to 63 (UI-1).
- `vc_build`: the `--version` line `0.3.1 (efc600d, commit 26 Oct 2026 10:02 UTC)` from one call in `build.rs` and `build_info!()` in the binary; worktree-safe, std only (VER-1).
- `vc_build::version!()`: the line as a `&'static str` for clap's `#[command(version = ...)]` (VER-1).
- `vc_ui::HEADING`: bold for a heading in a report (UI-1).
- docs: README, AGENTS.md, FEATURES.md and the guide name the v0.1 crates and their entry points.

## [0.0.1] - 2026-11-07

### Changed

- docs: internal references removed

## [0.0.0] - 2026-11-03

### Added

- The Cargo workspace, dual-licensed MIT or Apache-2.0, with README, AGENTS.md, FEATURES.md and the just recipes `test` (with `--changed`), `lint`, `install` and `deploy` (REPO-1).
- The CLI UX guide, `docs/ux-guide.md`: how the command-line tools look and behave, from completion and help to output, errors, progress, prompts, configuration, `--version` and tests. The rules every UI shares are linked from vhrd_egui's UX guide, section 2 (GUIDE-1).
- `repo_checks`, a test-only crate: the guide stays at its path with its sections and its link to the shared UI rules, AGENTS.md links to it, and no Markdown file carries a private path (REPO-1).
