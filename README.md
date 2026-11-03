# vhrd_cli

Small shared crates for Rust command-line tools, and the guide they follow: the twin of [vhrd_egui](https://github.com/vhrdtech/vhrd_egui) for CLIs.

- **The guide:** [docs/ux-guide.md](docs/ux-guide.md) says how a CLI looks and behaves: completion, help, colour, output, errors and exit codes, progress, prompts, configuration, `--version` and the tests that keep it so. Read it before building or changing a CLI.
- **The crates** (planned, `vc_*`) do the parts every CLI needs, so a new CLI gets them by adding a dependency instead of copying code:
  - `vc_build`: the `--version` line with version, git SHA and commit time.
  - `vc_ui`: colour that turns off without a terminal or with `NO_COLOR`, the error line, exit codes, safe printing of outside strings.
  - `vc_complete`: bash, zsh and fish completion with live values and help texts, on clap's dynamic completion.
  - `vc_cmd`, `vc_usage`, `vc_steps`: commands that register themselves, usage records, steps with progress, tables and deploy summaries.
  - `vc_picker`: the picker on Tab when a value has several hits.
  - `vc_template`: a small CLI that shows every piece.

Heavy parts (progress bars, the terminal UI of the picker) sit in crates of their own, so a small CLI pays only for what it uses.

What works and what is planned: [FEATURES.md](FEATURES.md). Changes: [CHANGELOG.md](CHANGELOG.md). Working on the repo: [AGENTS.md](AGENTS.md).

## Commands

- `just test`: all tests (`just test --changed`: only the crates changed against main and their dependents).
- `just lint`: clippy, formatting and unused dependencies; it only checks.
- `just install`, `just deploy`: build with the deploy profile; `deploy` first brings the checkout up to main.

The tests run through [cargo-nextest](https://nexte.st/); `just lint` needs [cargo-machete](https://github.com/bnjbvr/cargo-machete).

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT license](LICENSE-MIT) at your option. Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in this work, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
