//! The picker in the real shells: bash, zsh and fish run interactively in a pty with `pickdemo`'s registration
//! sourced. Tab on a value with several hits opens the picker; words typed into it narrow the rows, Enter (or
//! Ctrl-Y) puts the pick in place of the last word, Esc leaves the line as it was. With `PICKDEMO_TAB_PICKER=0`
//! Tab offers plain candidates. Alt+D, bound by the rc block, opens it from the prompt: on an empty line the line
//! becomes `pickdemo open PICK`, else the pick goes in at the cursor.
//!
//! A shell that is not on PATH is skipped with a line on stderr, so is zsh without its completion functions.
use super::scratch;
use std::path::Path;
use vc_complete::rc::{Block, Key, Shell};
use vc_test::*;

/// What the picker draws while it waits for keys (its hint), and what it writes when it ends (the rows it drew
/// cleared): keys sent before the end would go to the picker, not the shell.
const PICKER_UP: &str = "Enter insert";
const PICKER_DONE: &str = "\x1b[J";

/// The shell with `pickdemo`'s completion block, as a setup step writes it into the shell's config: the
/// registration and, with `alt_d`, Alt+D bound to the pick widget.
fn session(dir: &Scratch, name: &str, bin: &Path, alt_d: bool, env: &[(&str, &str)]) -> Pty {
    let shell = match name {
        "bash" => Shell::Bash,
        "zsh" => Shell::Zsh,
        _ => Shell::Fish,
    };
    let mut block = Block::new("demo", &["pickdemo"]);
    if alt_d {
        block = block.pick_key("pickdemo", Key::Alt('d'));
    }
    let rc = dir.0.join("rc");
    std::fs::write(&rc, block.text(shell)).unwrap();
    Pty::session(dir, name, bin, &format!("source {}", rc.display()), env)
}

/// What a value on the line stands for: the shell's quotes and escapes taken off.
fn plain(s: &str) -> String {
    s.replace("\\ ", " ").replace(['\'', '"'], "")
}

/// Tab opens the picker with the words as its query; Enter, Ctrl-Y and Esc.
fn tab_picks(name: &str) {
    let Some(bin) = shell(name) else { return };
    let dir = scratch(&format!("tab-{name}"));
    let mut p = session(&dir, name, &bin, false, &[]);

    p.send("pickdemo open widget ds100\t");
    let drawn = p.wait(PICKER_UP);
    let picker = drawn.rsplit("\x1b[6n").next().unwrap_or_default();
    assert!(
        picker.contains("widget") && picker.contains("ds100") && picker.contains("2/4"),
        "{name}: the words are the query: {drawn:?}"
    );
    p.send(" rev2\r");
    p.wait(PICKER_DONE);
    let (line, _) = p.line();
    assert_eq!(
        plain(&line).trim_end(),
        "pickdemo open widget widget-ds100-rev2.txt",
        "{name}: Enter"
    );

    p.send("pickdemo open widget\t");
    p.wait(PICKER_UP);
    p.send("manual\x19");
    p.wait(PICKER_DONE);
    let (line, _) = p.line();
    assert_eq!(
        plain(&line).trim_end(),
        "pickdemo open /docs/widget-rm200.txt",
        "{name}: Ctrl-Y"
    );

    p.send("pickdemo open widget\t");
    p.wait(PICKER_UP);
    p.send("\x1b");
    p.wait(PICKER_DONE);
    let (line, _) = p.line();
    assert_eq!(
        plain(&line).trim_end(),
        "pickdemo open widget",
        "{name}: Esc"
    );

    p.send("pickdemo open an7\t");
    let (line, shown) = p.line();
    assert_eq!(
        plain(&line).trim_end(),
        "pickdemo open an7-start.txt",
        "{name}: one hit"
    );
    assert!(
        !shown.contains(PICKER_UP),
        "{name}: one hit opens no picker: {shown:?}"
    );
}

/// With `PICKDEMO_TAB_PICKER=0` Tab never opens the picker: several hits stay candidates.
fn tab_without_picker(name: &str) {
    let Some(bin) = shell(name) else { return };
    let dir = scratch(&format!("plain-{name}"));
    let mut p = session(&dir, name, &bin, false, &[("PICKDEMO_TAB_PICKER", "0")]);
    p.send("pickdemo open rev\t");
    let (line, shown) = p.line();
    assert_eq!(
        plain(&line).trim_end(),
        "pickdemo open widget-ds100-rev2.txt",
        "{name}"
    );
    p.send("pickdemo open widget\t");
    let (_, shown2) = p.line();
    for s in [shown, shown2] {
        assert!(!s.contains(PICKER_UP), "{name}: no picker: {s:?}");
    }
}

/// Alt+D on an empty line: the line becomes `pickdemo open PICK`; on a line with text the pick goes in at the
/// cursor; Esc changes nothing.
fn alt_d_picks(name: &str) {
    let Some(bin) = shell(name) else { return };
    let dir = scratch(&format!("alt-d-{name}"));
    let mut p = session(&dir, name, &bin, true, &[]);

    p.send("\x1bd");
    p.wait(PICKER_UP);
    p.send("rm200\r");
    p.wait(PICKER_DONE);
    let (line, _) = p.line();
    assert_eq!(
        plain(&line).trim_end(),
        "pickdemo open widget-rm200.txt",
        "{name}: empty line"
    );

    p.send("cat ");
    p.send("\x1bd");
    p.wait(PICKER_UP);
    p.send("an7\r");
    p.wait(PICKER_DONE);
    let (line, _) = p.line();
    assert_eq!(
        plain(&line).trim_end(),
        "cat an7-start.txt",
        "{name}: at the cursor"
    );

    p.send("cat ");
    p.send("\x1bd");
    p.wait(PICKER_UP);
    p.send("\x1b");
    p.wait(PICKER_DONE);
    let (line, _) = p.line();
    assert_eq!(line.trim_end(), "cat", "{name}: Esc");
}

/// Without the key in the block, Alt+D opens no picker.
fn alt_d_is_a_setting(name: &str) {
    let Some(bin) = shell(name) else { return };
    let dir = scratch(&format!("no-alt-d-{name}"));
    let mut p = session(&dir, name, &bin, false, &[]);
    p.send("x\x1bd");
    let (_, shown) = p.line();
    assert!(!shown.contains(PICKER_UP), "{name}: {shown:?}");
}

#[test]
fn tab_opens_the_picker_in_bash() {
    tab_picks("bash");
}

#[test]
fn tab_opens_the_picker_in_zsh() {
    tab_picks("zsh");
}

#[test]
fn tab_opens_the_picker_in_fish() {
    tab_picks("fish");
}

#[test]
fn tab_gives_plain_candidates_when_turned_off_in_bash() {
    tab_without_picker("bash");
}

#[test]
fn tab_gives_plain_candidates_when_turned_off_in_zsh() {
    tab_without_picker("zsh");
}

#[test]
fn tab_gives_plain_candidates_when_turned_off_in_fish() {
    tab_without_picker("fish");
}

#[test]
fn alt_d_opens_the_picker_in_bash() {
    alt_d_picks("bash");
}

#[test]
fn alt_d_opens_the_picker_in_zsh() {
    alt_d_picks("zsh");
}

#[test]
fn alt_d_opens_the_picker_in_fish() {
    alt_d_picks("fish");
}

#[test]
fn alt_d_is_off_without_the_setting_in_bash() {
    alt_d_is_a_setting("bash");
}

#[test]
fn alt_d_is_off_without_the_setting_in_zsh() {
    alt_d_is_a_setting("zsh");
}

#[test]
fn alt_d_is_off_without_the_setting_in_fish() {
    alt_d_is_a_setting("fish");
}
