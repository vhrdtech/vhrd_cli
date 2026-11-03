# Changelog

All notable changes to the vhrd_cli crates and docs. The format follows [Keep a Changelog](https://keepachangelog.com/), versions follow [Semantic Versioning](https://semver.org/). Feature IDs refer to [FEATURES.md](FEATURES.md).

## [Unreleased]

### Added

- The Cargo workspace, dual-licensed MIT or Apache-2.0, with README, AGENTS.md, FEATURES.md and the just recipes `test` (with `--changed`), `lint`, `install` and `deploy` (REPO-1).
- The CLI UX guide, `docs/ux-guide.md`: how the command-line tools look and behave, from completion and help to output, errors, progress, prompts, configuration, `--version` and tests. The rules every UI shares are linked from vhrd_egui's UX guide, section 2 (GUIDE-1).
- `repo_checks`, a test-only crate: the guide stays at its path with its sections and its link to the shared UI rules, AGENTS.md links to it, and no Markdown file carries a private path (REPO-1).
