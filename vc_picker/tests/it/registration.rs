//! What `COMPLETE=<shell> pickdemo` prints: the completion and the pick widget, every placeholder filled.
use super::scratch;
use vc_test::{clean, without_terminal};

#[test]
fn the_registration_holds_the_pick_widget() {
    let dir = scratch("registration");
    for (shell, wants) in [
        (
            "bash",
            &[
                "_vc_pick_pickdemo() {",
                "' pick 2>/dev/null",
                "READLINE_LINE=\"pickdemo open $v\"",
            ][..],
        ),
        (
            "zsh",
            &[
                "function _vc_pick_pickdemo() {",
                "zle -N _vc_pick_pickdemo",
                "BUFFER=\"pickdemo open ${(q)v}\"",
            ][..],
        ),
        (
            "fish",
            &[
                "function __vc_pick_pickdemo",
                "commandline --replace -- \"pickdemo open \"",
            ][..],
        ),
    ] {
        let out = clean(&dir.0.join("bin/pickdemo"), &dir)
            .env("COMPLETE", shell)
            .output()
            .unwrap();
        let script = String::from_utf8(out.stdout).unwrap();
        for want in wants {
            assert!(script.contains(want), "{shell}: no {want:?} in\n{script}");
        }
        for placeholder in ["BINID", "PICK_RUN", "PICK_LINE", "COMPLETER", "EXTRA"] {
            assert!(
                !script.contains(placeholder),
                "{shell}: {placeholder} left in\n{script}"
            );
        }
    }
}

#[test]
fn pick_without_a_terminal_says_so_and_fails() {
    let dir = scratch("no-tty");
    let mut c = clean(&dir.0.join("bin/pickdemo"), &dir);
    c.arg("pick").stdin(std::process::Stdio::null());
    let out = without_terminal(&mut c).output().unwrap();
    assert!(!out.status.success());
    assert!(out.stdout.is_empty());
    let err = String::from_utf8(out.stderr).unwrap();
    assert_eq!(
        err,
        "pickdemo: the picker draws on the terminal, and there is none (/dev/tty)\n"
    );
}
