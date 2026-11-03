//! TAB in the real shells: bash, zsh and fish run interactively in a pty with `vcdemo`'s registration sourced, a
//! line is typed, then TAB; a key bound to print the shell's line buffer (`LINE<...>END`) says what the line became.
//!
//! A shell that is not on PATH is skipped with a line on stderr, so is zsh without its completion functions
//! (`compinit`; zsh takes them from `FPATH` too).
mod support;

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use support::*;

/// The shell to run, or `None` with a line saying why not.
fn shell(name: &str) -> Option<PathBuf> {
    let p = which(name)?;
    if name == "zsh" {
        let ok = std::process::Command::new(&p)
            .args(["-f", "-c", "autoload -Uz compinit && compinit -u -D"])
            .output()
            .is_ok_and(|o| o.status.success() && o.stderr.is_empty());
        if !ok {
            eprintln!(
                "vc_complete tests: {} has no compinit (its functions are not in its fpath or FPATH), zsh is not tried",
                p.display()
            );
            return None;
        }
    }
    Some(p)
}

/// A shell in a pty of its own, as a terminal emulator runs it.
struct Pty {
    master: std::fs::File,
    child: Child,
    out: Arc<Mutex<String>>,
    seen: usize,
}

impl Pty {
    fn start(dir: &Scratch, bin: &Path, args: &[&str]) -> Pty {
        use rustix::pty::{OpenptFlags, grantpt, openpt, ptsname, unlockpt};
        let master = openpt(OpenptFlags::RDWR | OpenptFlags::NOCTTY).expect("openpt");
        grantpt(&master).unwrap();
        unlockpt(&master).unwrap();
        let size = rustix::termios::Winsize {
            ws_row: 30,
            ws_col: 120,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };
        rustix::termios::tcsetwinsize(&master, size).unwrap();
        let name = ptsname(&master, Vec::new()).unwrap();
        let slave = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(name.to_str().unwrap())
            .unwrap();
        let mut c = clean(bin, dir);
        c.args(args)
            .stdin(Stdio::from(slave.try_clone().unwrap()))
            .stdout(Stdio::from(slave.try_clone().unwrap()))
            .stderr(Stdio::from(slave));
        if let Some(f) = std::env::var_os("FPATH") {
            c.env("FPATH", f);
        }
        // SAFETY: only raw syscalls between fork and exec
        unsafe {
            use std::os::unix::process::CommandExt;
            c.pre_exec(|| {
                rustix::process::setsid()?;
                rustix::process::ioctl_tiocsctty(std::io::stdin())?;
                Ok(())
            });
        }
        let child = c.spawn().expect("shell");
        let out = Arc::new(Mutex::new(String::new()));
        let mut rd = std::fs::File::from(master.try_clone().unwrap());
        let mut wr = std::fs::File::from(master.try_clone().unwrap());
        let o = out.clone();
        std::thread::spawn(move || {
            let mut buf = [0u8; 4096];
            while let Ok(n) = rd.read(&mut buf) {
                if n == 0 {
                    break;
                }
                let s = String::from_utf8_lossy(&buf[..n]).into_owned();
                // fish asks the terminal what it is (ESC[0c) and waits for the answer: a plain VT220
                if s.contains("\x1b[c") || s.contains("\x1b[0c") {
                    let _ = wr.write_all(b"\x1b[?62c");
                }
                // and where the cursor is
                if s.contains("\x1b[6n") {
                    let _ = wr.write_all(b"\x1b[5;3R");
                }
                o.lock().unwrap().push_str(&s);
            }
        });
        Pty {
            master: std::fs::File::from(master),
            child,
            out,
            seen: 0,
        }
    }

    fn send(&mut self, s: &str) {
        self.master.write_all(s.as_bytes()).unwrap();
    }

    /// The output after what was read before, up to and with `marker`, once it is there. The time limit only
    /// turns a hang into a failure with the output; it decides no outcome.
    fn wait(&mut self, marker: &str) -> String {
        let t0 = Instant::now();
        loop {
            {
                let all = self.out.lock().unwrap();
                if let Some(i) = all[self.seen..].find(marker) {
                    let got = all[self.seen..self.seen + i + marker.len()].to_string();
                    self.seen += i + marker.len();
                    return got;
                }
            }
            assert!(
                t0.elapsed() < Duration::from_secs(60),
                "no {marker:?} in:\n{}",
                &self.out.lock().unwrap()[self.seen..]
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    /// The line buffer, by the key bound to print it (`^X`), then the line is cleared (`^U`).
    fn line(&mut self) -> (String, String) {
        self.send("\x18");
        let before = self.wait("LINE<");
        let rest = self.wait(">END");
        self.send("\x15");
        (rest.trim_end_matches(">END").to_string(), before)
    }
}

impl Drop for Pty {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// The shell started with `vcdemo`'s completion and the `^X` key, ready for a line.
fn session(dir: &Scratch, name: &str, bin: &Path) -> Pty {
    let (args, setup): (&[&str], &str) = match name {
        "bash" => (
            &["--norc", "--noprofile", "-i"],
            "PS1='$ '; source <(COMPLETE=bash vcdemo); \
             bind -x '\"\\C-x\": printf \"LINE<%s>END\\n\" \"$READLINE_LINE\"'; echo RE''ADY\n",
        ),
        "zsh" => (
            &["-f", "-i"],
            "PS1='$ '; autoload -Uz compinit; compinit -u -D; source <(COMPLETE=zsh vcdemo); \
             show() { zle -I; print -r -- \"LINE<$BUFFER>END\" }; zle -N show; bindkey '^X' show; echo RE''ADY\n",
        ),
        _ => (
            &["--no-config", "-i"],
            "COMPLETE=fish vcdemo | source; bind \\cx 'echo \"LINE<\"(commandline)\">END\"'; echo RE''ADY\n",
        ),
    };
    let mut p = Pty::start(dir, bin, args);
    p.send(setup);
    p.wait("READY");
    p
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
    let dir = Scratch::new(&format!("shell-{name}"));
    let mut p = session(&dir, name, &bin);
    let mut cases = vec![
        ("vcdemo open it", vec!["vcdemo", "open", "it's"]),
        ("vcdemo open 'alpha t", vec!["vcdemo", "open", "alpha two"]),
        ("vcdemo open b", vec!["vcdemo", "open", "beta"]),
        ("vcdemo host user@c", vec!["vcdemo", "host", "user@core"]),
        ("vcdemo find two", vec!["vcdemo", "find", "alpha two"]),
        ("vcdemo fail x", vec!["vcdemo", "fail", "x"]),
        ("vcdemo panic x", vec!["vcdemo", "panic", "x"]),
        ("vcdemo shell x", vec!["vcdemo", "shell", name]),
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
