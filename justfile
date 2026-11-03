# vhrd_cli: library crates for Rust CLIs, plus the CLI UX guide (docs/ux-guide.md).

# Bare `just` only lists the recipes
default:
    @just --list

# The tests. `just test`: all of them (test-all). `just test --changed`: only the workspace crates changed against
# main and the crates that depend on them (needs tpm on PATH: `tpm test changed`); without tpm it runs them all.
test *flags:
    if [ "{{flags}}" = "--changed" ] && command -v tpm >/dev/null; then tpm test changed -- cargo nextest run --workspace; elif [ -z "{{flags}}" ] || [ "{{flags}}" = "--changed" ]; then {{just_executable()}} test-all; else echo "just test: unknown {{flags}} (--changed or nothing)" >&2; exit 2; fi

# Every test through nextest, then the doc tests (nextest doesn't run those)
test-all:
    cargo nextest run --workspace
    cargo test --workspace --doc

# Checks only, never edits: clippy (warnings are errors), formatting, no unused dependency
lint:
    cargo clippy --workspace --all-targets -- -D warnings
    cargo fmt --all --check
    cargo machete

# Build with the deploy profile. The crates are libraries that CLIs depend on, so nothing goes on PATH yet; the
# example CLI (vc_template, v0.2) will be installed here.
install:
    cargo build --profile deploy --locked --workspace

# Bring this checkout up to main and build it
deploy:
    test -z "$(git status --porcelain)" || { echo "local changes, commit or move them first:" >&2; git status --short >&2; exit 1; }
    git pull --ff-only
    just install
