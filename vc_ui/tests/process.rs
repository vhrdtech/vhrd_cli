//! The parts that act on the whole process (SIGPIPE, the error line on stderr, the exit code), run the way a CLI
//! runs them: this test binary starts itself again with `VC_UI_CHILD` set, and the `child` test plays the CLI.

use std::io::{Read, Write};
use std::process::{Command, Output, Stdio};
use vc_ui::{ColorChoice, Exit};

const CHILD: &str = "VC_UI_CHILD";

/// The CLI side. Without `VC_UI_CHILD` (a normal test run) it does nothing.
#[test]
fn child() {
    let Ok(mode) = std::env::var(CHILD) else {
        return;
    };
    let code = match mode.as_str() {
        "sigpipe" | "sigpipe-ignored" => {
            if mode == "sigpipe" {
                vc_ui::restore_sigpipe();
            }
            let mut out = std::io::stdout().lock();
            loop {
                if writeln!(out, "a line of output").is_err() {
                    // only reached when SIGPIPE stays ignored
                    break 9;
                }
            }
        }
        "error" | "error-coloured" => {
            if mode == "error-coloured" {
                vc_ui::set_color(ColorChoice::Always);
            }
            let err = anyhow::anyhow!("no device answered\x1b[2J").context("reading the meter");
            let other = vc_ui::finish("demo", Err::<(), _>(err), |_| None);
            let config = vc_ui::finish(
                "demo",
                Err::<(), _>(Exit::CONFIG.error("no secret")),
                |_| None,
            );
            assert_eq!(other, Exit::FAILURE);
            config.code()
        }
        _ => 99,
    };
    std::process::exit(i32::from(code));
}

/// This binary, set to run only the `child` test in `mode`, without the `CARGO_*` environment.
fn child_cmd(mode: &str) -> Command {
    let exe = std::env::current_exe().unwrap_or_else(|e| panic!("the test binary's path: {e}"));
    let mut cmd = Command::new(exe);
    cmd.args(["child", "--exact", "--nocapture", "--test-threads=1"]);
    for (k, _) in std::env::vars_os() {
        if k.to_string_lossy().starts_with("CARGO_") {
            cmd.env_remove(k);
        }
    }
    cmd.env(CHILD, mode).env_remove("NO_COLOR");
    cmd
}

/// Starts the child, reads its stdout up to its first line, closes the pipe and waits for it to end.
fn read_then_close(mode: &str) -> std::process::ExitStatus {
    let mut child = child_cmd(mode)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|e| panic!("starting the child: {e}"));
    let mut stdout = child
        .stdout
        .take()
        .unwrap_or_else(|| panic!("no stdout pipe"));
    // until the child's own line: the test harness writes a header first, and a pipe closed after only that
    // would fail the harness's next write instead of the child's
    let mut seen = Vec::new();
    let mut buf = [0u8; 4096];
    while !String::from_utf8_lossy(&seen).contains("a line of output") {
        let n = stdout
            .read(&mut buf)
            .unwrap_or_else(|e| panic!("reading the child: {e}"));
        assert!(
            n > 0,
            "the child ended before its output: {}",
            String::from_utf8_lossy(&seen)
        );
        seen.extend_from_slice(&buf[..n]);
    }
    drop(stdout);
    child
        .wait()
        .unwrap_or_else(|e| panic!("waiting for the child: {e}"))
}

#[cfg(unix)]
#[test]
fn closed_pipe_ends_the_run_quietly_by_sigpipe() {
    use std::os::unix::process::ExitStatusExt;
    let status = read_then_close("sigpipe");
    assert_eq!(status.signal(), Some(libc_sigpipe()), "{status:?}");
    // the control: without restore_sigpipe the write fails instead, and the run goes on to its own exit
    let status = read_then_close("sigpipe-ignored");
    assert_eq!(status.code(), Some(9), "{status:?}");
}

/// SIGPIPE is 13 on every Unix this runs on.
#[cfg(unix)]
fn libc_sigpipe() -> i32 {
    13
}

fn run(mode: &str) -> Output {
    child_cmd(mode)
        .output()
        .unwrap_or_else(|e| panic!("running the child: {e}"))
}

#[test]
fn error_line_on_stderr_and_the_exit_code() {
    let out = run("error");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(
        stderr, "demo: reading the meter: no device answered\\u{1b}[2J\ndemo: no secret\n",
        "stderr is a pipe: no colour, the device's escape shown as text"
    );
    assert_eq!(out.status.code(), Some(78));
}

#[test]
fn color_always_colours_stderr_in_a_pipe() {
    let out = run("error-coloured");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("\x1b["), "{stderr:?}");
    assert_eq!(
        vc_ui::strip_ansi(&stderr),
        "demo: reading the meter: no device answered\\u{1b}[2J\ndemo: no secret\n"
    );
}
