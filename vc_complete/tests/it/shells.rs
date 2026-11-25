//! TAB in the real shells: bash, zsh and fish run interactively in a pty with `vcdemo`'s registration sourced, a
//! line is typed, then TAB; a key bound to print the shell's line buffer (`LINE<...>END`) says what the line became.
//!
//! A shell that is not on PATH is skipped with a line on stderr, so is zsh without its completion functions
//! (`compinit`; zsh takes them from `FPATH` too).
use super::scratch;
use std::path::Path;
use vc_test::*;

/// The shell started with `vcdemo`'s completion and the `^X` key, ready for a line.
fn session(dir: &Scratch, name: &str, bin: &Path) -> Pty {
    let rc = match name {
        "fish" => "COMPLETE=fish vcdemo | source",
        _ => &format!("source <(COMPLETE={name} vcdemo)"),
    };
    Pty::session(dir, name, bin, rc, &[])
}

/// The words of a line as the shell would pass them, its quotes and escapes taken off.
fn words(line: &str) -> Vec<String> {
    let (mut w, _) = vc_complete::split_line(line, "");
    if w.last().is_some_and(String::is_empty) {
        w.pop();
    }
    w
}

/// In one shell: each line typed, TAB, and the words the line then holds.
fn tab(name: &str) {
    let Some(bin) = shell(name) else { return };
    let dir = scratch(&format!("shell-{name}"));
    let mut p = session(&dir, name, &bin);
    let mut cases = vec![
        ("vcdemo open it", vec!["vcdemo", "open", "it's"]),
        ("vcdemo open 'alpha t", vec!["vcdemo", "open", "alpha two"]),
        ("vcdemo open b", vec!["vcdemo", "open", "beta"]),
        ("vcdemo host user@c", vec!["vcdemo", "host", "user@core"]),
        ("vcdemo find two", vec!["vcdemo", "find", "alpha two"]),
        ("vcdemo fail x", vec!["vcdemo", "fail", "x"]),
        ("vcdemo panic x", vec!["vcdemo", "panic", "x"]),
        // fish drops a candidate unlike the typed token; each shell's name holds an `s`
        ("vcdemo shell s", vec!["vcdemo", "shell", name]),
    ];
    if name == "fish" {
        cases.push(("vcdemo pick x", vec!["vcdemo", "pick", "alpha two"]));
    }
    for (typed, want) in cases {
        p.send(typed);
        p.send("\t");
        let (line, shown) = p.line();
        assert_eq!(
            words(&line),
            want,
            "{name}: {typed:?} then TAB gave {line:?}"
        );
        for noise in ["panicked", "no bus", "some-file", "error"] {
            assert!(
                !shown.contains(noise),
                "{name}: {typed:?} showed {noise:?}:\n{shown}"
            );
        }
    }
}

#[test]
fn tab_completes_in_bash() {
    tab("bash");
}

#[test]
fn tab_completes_in_zsh() {
    tab("zsh");
}

#[test]
fn tab_completes_in_fish() {
    tab("fish");
}
