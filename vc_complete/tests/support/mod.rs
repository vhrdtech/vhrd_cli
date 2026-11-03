//! What the completion tests share: the example CLI `vcdemo`, a scratch directory and a clean environment.
#![allow(dead_code)] // each test file uses its own part

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

/// The example CLI, which cargo builds with the tests: `target/<profile>/examples/vcdemo-<hash>` (the newest, should
/// older builds lie beside it), or `vcdemo` after a `cargo build --examples`.
pub fn vcdemo() -> PathBuf {
    let exe = std::env::current_exe().expect("the test binary's path");
    let examples = exe
        .parent()
        .and_then(Path::parent)
        .expect("the test binary sits in <profile>/deps")
        .join("examples");
    let built = |name: &str| {
        name == "vcdemo"
            || name
                .strip_prefix("vcdemo-")
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
            "no vcdemo in {}: cargo builds the examples with the tests (cargo test or cargo nextest run)",
            examples.display()
        ),
    }
}

/// A scratch directory for one test, removed when it is dropped. It holds a file, so a test can see that file
/// names are never offered in place of values.
pub struct Scratch(pub PathBuf);

impl Scratch {
    pub fn new(name: &str) -> Scratch {
        // tests of one binary share the process under `cargo test`: a counter keeps their directories apart
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let dir =
            std::env::temp_dir().join(format!("vc-complete-{name}-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("home")).unwrap();
        std::fs::write(dir.join("some-file.txt"), "").unwrap();
        // `vcdemo` by its own name, as the shells call it
        std::fs::create_dir_all(dir.join("bin")).unwrap();
        std::os::unix::fs::symlink(vcdemo(), dir.join("bin/vcdemo")).unwrap();
        Scratch(dir)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A command with nothing of the test's environment (no `CARGO_*`), only a scratch home, a terminal type and PATH
/// with `vcdemo` first.
pub fn clean(bin: &Path, dir: &Scratch) -> Command {
    let mut c = Command::new(bin);
    c.current_dir(&dir.0)
        .env_clear()
        .env("HOME", dir.0.join("home"))
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

/// The shell on PATH, or `None` with a line on stderr saying it is not tried.
pub fn which(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    let found = std::env::split_paths(&path)
        .map(|d| d.join(name))
        .find(|p| p.is_file());
    if found.is_none() {
        eprintln!("vc_complete tests: no {name} on PATH, its completion is not tried");
    }
    found
}
