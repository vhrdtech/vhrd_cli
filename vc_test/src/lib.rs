//! The test harness of the vhrd_cli crates: example binaries, scratch directories, a clean environment and the real
//! shells in a pty.
//!
//! A test starts a shell with [`Pty::session`], types a line and keys with [`Pty::send`], waits for output with
//! [`Pty::wait`] and reads the shell's line buffer back with [`Pty::line`]. A shell that is not installed gives
//! `None` from [`shell`] with a line on stderr, and the test returns early.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// How long [`Pty::wait`] waits before it fails with the output so far. It only turns a hang into a failure; it
/// decides no outcome.
const WAIT_MAX: Duration = Duration::from_secs(60);

/// An example binary that cargo builds with the tests: `<target>/<profile>/examples/<name>-<hash>` or `<name>` (the
/// newest, should older builds lie beside it). The test binary sits in `<target>/<profile>/deps`, or in a separate
/// build directory (cargo's `build.build-dir`); then the target directory is `CARGO_TARGET_DIR` or the workspace's
/// `target`.
///
/// # Panics
///
/// When the example was not built.
pub fn example(name: &str) -> PathBuf {
    let exe = std::env::current_exe().expect("the test binary's path");
    let profile = if exe.components().any(|c| c.as_os_str() == "release") {
        "release"
    } else {
        "debug"
    };
    let target = std::env::var_os("CARGO_TARGET_DIR").map_or_else(
        || Path::new(env!("CARGO_MANIFEST_DIR")).join("../target"),
        PathBuf::from,
    );
    let beside = exe
        .parent()
        .and_then(Path::parent)
        .map(|p| p.join("examples"));
    let examples = beside
        .filter(|d| d.is_dir())
        .unwrap_or_else(|| target.join(profile).join("examples"));
    let built = |file: &str| {
        file == name
            || file
                .strip_prefix(name)
                .and_then(|r| r.strip_prefix('-'))
                .is_some_and(|h| h.len() == 16 && h.chars().all(|c| c.is_ascii_hexdigit()))
    };
    let newest = std::fs::read_dir(&examples)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| built(&e.file_name().to_string_lossy()))
        .filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path())))
        .max();
    match newest {
        Some((_, bin)) => bin,
        None => panic!(
            "no {name} in {}: cargo builds the examples with the tests (cargo test or cargo nextest run)",
            examples.display()
        ),
    }
}

/// A scratch directory for one test, removed when it is dropped: `home/`, `bin/` (first on PATH in [`clean`]) and a
/// file `some-file.txt`, so a test can see that file names are never offered in place of values.
pub struct Scratch(pub PathBuf);

impl Scratch {
    /// A new, empty scratch directory named after the test.
    ///
    /// # Panics
    ///
    /// When the directory cannot be made.
    pub fn new(name: &str) -> Self {
        // tests of one binary share the process under `cargo test`: a counter keeps their directories apart
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("vc-test-{name}-{}-{n}", std::process::id()));
        if dir.exists() {
            std::fs::remove_dir_all(&dir).expect("an old scratch directory is removed");
        }
        std::fs::create_dir_all(dir.join("home")).expect("scratch home");
        std::fs::create_dir_all(dir.join("bin")).expect("scratch bin");
        std::fs::write(dir.join("some-file.txt"), "").expect("scratch file");
        Self(dir)
    }

    /// The scratch directory with `bin` in its `bin/` under its own `name`, as the shells call it.
    ///
    /// # Panics
    ///
    /// When the link cannot be made.
    pub fn with_bin(self, name: &str, bin: &Path) -> Self {
        std::os::unix::fs::symlink(bin, self.0.join("bin").join(name)).expect("bin link");
        self
    }

    /// The scratch home.
    pub fn home(&self) -> PathBuf {
        self.0.join("home")
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        // a test that failed leaves nothing to clean up after; a left-over directory is removed by the next run
        std::fs::remove_dir_all(&self.0).ok();
    }
}

/// A command with nothing of the test's environment (no `CARGO_*`), only a scratch home, a terminal type, a UTF-8
/// locale and PATH with the scratch `bin/` first.
pub fn clean(bin: &Path, dir: &Scratch) -> Command {
    let mut c = Command::new(bin);
    c.current_dir(&dir.0)
        .env_clear()
        .env("HOME", dir.home())
        .env("TERM", "xterm")
        .env("LANG", "C.UTF-8")
        .env(
            "PATH",
            format!(
                "{}:{}",
                dir.0.join("bin").display(),
                std::env::var("PATH").unwrap_or_default()
            ),
        );
    c
}

/// A program on PATH, or `None` with a line on stderr saying what is not tried.
pub fn which(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    let found = std::env::split_paths(&path)
        .map(|d| d.join(name))
        .find(|p| p.is_file());
    if found.is_none() {
        eprintln!("skipped: {name} missing, not on PATH");
    }
    found
}

/// The shell to run, or `None` with a line saying why not: not on PATH, or zsh without its completion functions
/// (`compinit`; zsh takes them from `FPATH` too).
pub fn shell(name: &str) -> Option<PathBuf> {
    let p = which(name)?;
    if name == "zsh" {
        let ok = Command::new(&p)
            .args(["-f", "-c", "autoload -Uz compinit && compinit -u -D"])
            .output()
            .is_ok_and(|o| o.status.success() && o.stderr.is_empty());
        if !ok {
            eprintln!(
                "skipped: zsh's compinit missing ({} has no completion functions in its fpath or FPATH)",
                p.display()
            );
            return None;
        }
    }
    Some(p)
}

/// A shell in a pty of its own, 120 x 30, as a terminal emulator runs it. The pty answers a terminal's questions:
/// what it is (`ESC[c`, as a VT220) and where the cursor is (`ESC[6n`, on the fifth line).
pub struct Pty {
    master: std::fs::File,
    child: Child,
    out: Arc<Mutex<String>>,
    seen: usize,
}

impl Pty {
    /// `bin` with `args` in a new pty, with [`clean`]'s environment and `env` on top.
    ///
    /// # Panics
    ///
    /// When the pty cannot be made or the program not started.
    pub fn start(dir: &Scratch, bin: &Path, args: &[&str], env: &[(&str, &str)]) -> Self {
        use rustix::pty::{OpenptFlags, grantpt, openpt, ptsname, unlockpt};
        let master = openpt(OpenptFlags::RDWR | OpenptFlags::NOCTTY).expect("openpt");
        grantpt(&master).expect("grantpt");
        unlockpt(&master).expect("unlockpt");
        let size = rustix::termios::Winsize {
            ws_row: 30,
            ws_col: 120,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };
        rustix::termios::tcsetwinsize(&master, size).expect("pty size");
        let name = ptsname(&master, Vec::new()).expect("ptsname");
        let slave = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(name.to_str().expect("a pty name is ASCII"))
            .expect("pty slave");
        let mut c = clean(bin, dir);
        c.args(args)
            .stdin(Stdio::from(slave.try_clone().expect("slave fd")))
            .stdout(Stdio::from(slave.try_clone().expect("slave fd")))
            .stderr(Stdio::from(slave));
        if let Some(f) = std::env::var_os("FPATH") {
            c.env("FPATH", f);
        }
        for (k, v) in env {
            c.env(k, v);
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
        let child = c.spawn().expect("the program starts");
        let out = Arc::new(Mutex::new(String::new()));
        let mut rd = std::fs::File::from(master.try_clone().expect("master fd"));
        let mut wr = std::fs::File::from(master.try_clone().expect("master fd"));
        let o = out.clone();
        std::thread::spawn(move || {
            let mut buf = [0u8; 4096];
            while let Ok(n) = rd.read(&mut buf) {
                if n == 0 {
                    break;
                }
                let s = String::from_utf8_lossy(&buf[..n]).into_owned();
                // a write that fails means the shell is gone; the test then fails on its wait
                if s.contains("\x1b[c") || s.contains("\x1b[0c") {
                    wr.write_all(b"\x1b[?62c").ok();
                }
                if s.contains("\x1b[6n") {
                    wr.write_all(b"\x1b[5;3R").ok();
                }
                o.lock().expect("output lock").push_str(&s);
            }
        });
        Self {
            master: std::fs::File::from(master),
            child,
            out,
            seen: 0,
        }
    }

    /// The shell `name` started interactively without its own config, `rc` run in it, and `^X` bound to print the
    /// line buffer for [`Pty::line`]; it returns once the shell is ready for a line.
    pub fn session(dir: &Scratch, name: &str, bin: &Path, rc: &str, env: &[(&str, &str)]) -> Self {
        let (args, setup): (&[&str], String) = match name {
            "bash" => (
                &["--norc", "--noprofile", "-i"],
                format!(
                    "PS1='$ '; {rc}; bind -x '\"\\C-x\": printf \"LINE<%s>END\\n\" \"$READLINE_LINE\"'; echo RE''ADY\n"
                ),
            ),
            "zsh" => (
                &["-f", "-i"],
                format!(
                    "PS1='$ '; autoload -Uz compinit; compinit -u -D; {rc}; \
                     show() {{ zle -I; print -r -- \"LINE<$BUFFER>END\" }}; zle -N show; bindkey '^X' show; echo RE''ADY\n"
                ),
            ),
            _ => (
                &["--no-config", "-i"],
                format!("{rc}; bind ctrl-x 'echo \"LINE<\"(commandline)\">END\"'; echo RE''ADY\n"),
            ),
        };
        let mut p = Self::start(dir, bin, args, env);
        p.send(&setup);
        p.wait("READY");
        p
    }

    /// Types `s` into the terminal.
    ///
    /// # Panics
    ///
    /// When the pty is gone.
    pub fn send(&mut self, s: &str) {
        self.master.write_all(s.as_bytes()).expect("pty write");
    }

    /// The output after what was read before, up to and with `marker`, once it is there.
    ///
    /// # Panics
    ///
    /// When the marker does not come within a minute, with the output so far.
    pub fn wait(&mut self, marker: &str) -> String {
        let t0 = Instant::now();
        loop {
            {
                let all = self.out.lock().expect("output lock");
                if let Some(i) = all[self.seen..].find(marker) {
                    let got = all[self.seen..self.seen + i + marker.len()].to_owned();
                    self.seen += i + marker.len();
                    return got;
                }
            }
            assert!(
                t0.elapsed() < WAIT_MAX,
                "no {marker:?} in:\n{}",
                &self.out.lock().expect("output lock")[self.seen..]
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    /// The line buffer by the key bound to print it (`^X`), then the line is cleared (`^U`). Returns the line and
    /// what the terminal showed before it.
    pub fn line(&mut self) -> (String, String) {
        self.send("\x18");
        let before = self.wait("LINE<");
        let rest = self.wait(">END");
        self.send("\x15");
        (rest.trim_end_matches(">END").to_owned(), before)
    }
}

impl Drop for Pty {
    fn drop(&mut self) {
        // the shell may have exited by itself already
        self.child.kill().ok();
        self.child.wait().ok();
    }
}
