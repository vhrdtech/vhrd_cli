# Changelog

All notable changes to the vhrd_cli crates and docs. The format follows [Keep a Changelog](https://keepachangelog.com/), versions follow [Semantic Versioning](https://semver.org/). Feature IDs refer to [FEATURES.md](FEATURES.md).

## [Unreleased]

### Added

- `vc_complete`: bash, zsh and fish completion for clap CLIs with live values and help texts, on clap_complete's dynamic completion. One call at the top of `main`; a bash adapter that quotes values and handles the `@`, `:` and `=` word breaks without falling back to file names; zsh offers candidates that replace the typed word in a menu; fish takes a picker's choice. A failing or panicking completer gives no candidates and prints nothing. Helpers for candidates (`cand`, `filtered`, `values`) and the line being completed (`split_line`, `comp_words`, `word_after`), and the example CLI `vcdemo` (COMP-1).
- `vc_complete::rc`: the marked completion block in the bash, zsh and fish config files for a list of CLIs, written once, replaced when it changes, and taking the place of hand-written registration lines (COMP-2).

## [0.0.0] - 2026-11-03

### Added

- The Cargo workspace, dual-licensed MIT or Apache-2.0, with README, AGENTS.md, FEATURES.md and the just recipes `test` (with `--changed`), `lint`, `install` and `deploy` (REPO-1).
- The CLI UX guide, `docs/ux-guide.md`: how the command-line tools look and behave, from completion and help to output, errors, progress, prompts, configuration, `--version` and tests. The rules every UI shares are linked from vhrd_egui's UX guide, section 2 (GUIDE-1).
- `repo_checks`, a test-only crate: the guide stays at its path with its sections and its link to the shared UI rules, AGENTS.md links to it, and no Markdown file carries a private path (REPO-1).
