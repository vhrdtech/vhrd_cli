//! The `--version` line of a CLI: `0.3.1 (efc600d, commit 26 Oct 2026 10:02 UTC)`.
//!
//! Two parts, one crate, std only. In `build.rs`, [`emit`] asks git for the short SHA (`-dirty` when the tree has
//! changes) and the commit's own time, and hands both to the compiler. In the binary, [`build_info!`] reads them
//! back as an [`Info`], whose [`line`](Info::line) is what `--version` shows after the tool's name.
//!
//! ```toml
//! [dependencies]
//! vc_build = "0.1"
//!
//! [build-dependencies]
//! vc_build = "0.1"
//! ```
//!
//! ```ignore
//! // build.rs
//! fn main() {
//!     vc_build::emit();
//! }
//! ```
//!
//! ```ignore
//! // main.rs
//! println!("{}", vc_build::build_info!().line_named("tool")); // tool 0.3.1 (efc600d, commit ...)
//! ```
//!
//! The commit time, not the clock, so a rebuild of an unchanged commit gives the same output. Both git values
//! are `unknown` without git or outside a repository. The build script is worktree-safe: it watches the real
//! git directory's `HEAD` and index and the shared branch ref and `packed-refs`, so cargo rebuilds when a commit
//! is made and not otherwise.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Version, git SHA (short, `-dirty` when the tree has changes) and commit time (`3 Oct 2026 18:20 UTC`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Info {
    pub version: &'static str,
    pub sha: &'static str,
    pub time: &'static str,
}

impl Info {
    /// `0.3.1 (efc600d, commit 26 Oct 2026 10:02 UTC)`: what `--version` shows after the tool's name.
    ///
    /// ```
    /// let info = vc_build::Info { version: "0.3.1", sha: "efc600d", time: "26 Oct 2026 10:02 UTC" };
    /// assert_eq!(info.line(), "0.3.1 (efc600d, commit 26 Oct 2026 10:02 UTC)");
    /// ```
    pub fn line(&self) -> String {
        format!("{} ({}, commit {})", self.version, self.sha, self.time)
    }

    /// The whole `--version` output: `tool 0.3.1 (efc600d, commit 26 Oct 2026 10:02 UTC)`.
    ///
    /// ```
    /// let info = vc_build::Info { version: "0.3.1", sha: "efc600d", time: "26 Oct 2026 10:02 UTC" };
    /// assert_eq!(info.line_named("tool"), "tool 0.3.1 (efc600d, commit 26 Oct 2026 10:02 UTC)");
    /// ```
    pub fn line_named(&self, name: &str) -> String {
        format!("{name} {}", self.line())
    }
}

/// The calling package's build as an [`Info`]: `CARGO_PKG_VERSION` and the `VC_GIT_SHA` and `VC_COMMIT_TIME` that
/// [`emit`] set in this package's `build.rs`. Expanded where it is written, so the version is the binary's
/// package, not this crate's.
#[macro_export]
macro_rules! build_info {
    () => {
        $crate::Info {
            version: env!("CARGO_PKG_VERSION"),
            sha: env!("VC_GIT_SHA"),
            time: env!("VC_COMMIT_TIME"),
        }
    };
}

/// What git said about a checkout.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GitInfo {
    /// Short SHA, with `-dirty` when the tree has changes; `unknown` without git.
    pub sha: String,
    /// `3 Oct 2026 18:20 UTC`, the commit's own time; `unknown` without git.
    pub time: String,
    /// Files whose change should rebuild: only ones that exist, since cargo calls a build script with a missing
    /// watched file stale on every build.
    pub watch: Vec<PathBuf>,
}

/// Asks git about the checkout at `dir`. Never fails: anything git cannot say is `unknown`.
pub fn collect(dir: &Path) -> GitInfo {
    let git = |args: &[&str]| -> Option<String> {
        let out = Command::new("git")
            .args(args)
            .current_dir(dir)
            .output()
            .ok()?;
        out.status
            .success()
            .then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
    };
    let path = |p: String| -> PathBuf {
        let p = PathBuf::from(p);
        if p.is_absolute() { p } else { dir.join(p) }
    };

    let mut watch = Vec::new();
    if let Some(git_dir) = git(&["rev-parse", "--git-dir"]).map(path) {
        let common_dir = git(&["rev-parse", "--git-common-dir"])
            .map(path)
            .unwrap_or_else(|| git_dir.clone());
        watch.push(git_dir.join("HEAD"));
        watch.push(git_dir.join("index"));
        // HEAD is this worktree's own, but `ref: refs/heads/X` changes only on a checkout; a commit on X
        // rewrites the ref file in the common dir (or, once git packs it, packed-refs there).
        if let Ok(head) = std::fs::read_to_string(git_dir.join("HEAD"))
            && let Some(r) = head.trim().strip_prefix("ref: ")
        {
            watch.push(common_dir.join(r));
        }
        watch.push(common_dir.join("packed-refs"));
        watch.retain(|p| p.exists());
    }

    let sha = match git(&["rev-parse", "--short", "HEAD"]) {
        None => "unknown".to_string(),
        Some(sha) => match git(&["status", "--porcelain"]) {
            Some(changes) if changes.is_empty() => sha,
            _ => format!("{sha}-dirty"),
        },
    };
    let time = git(&["log", "-1", "--format=%ct"])
        .and_then(|s| s.parse::<i64>().ok())
        .map_or_else(|| "unknown".to_string(), format_time);
    GitInfo { sha, time, watch }
}

/// For `build.rs`: sets `VC_GIT_SHA` and `VC_COMMIT_TIME` for [`build_info!`] and tells cargo which git files
/// to watch. Runs git in the current directory, which cargo sets to the package's.
pub fn emit() {
    let info = collect(Path::new("."));
    for p in &info.watch {
        println!("cargo:rerun-if-changed={}", p.display());
    }
    println!("cargo:rustc-env=VC_GIT_SHA={}", info.sha);
    println!("cargo:rustc-env=VC_COMMIT_TIME={}", info.time);
}

/// `3 Oct 2026 18:20 UTC` from Unix seconds, without a date crate.
///
/// ```
/// assert_eq!(vc_build::format_time(0), "1 Jan 1970 00:00 UTC");
/// ```
pub fn format_time(secs: i64) -> String {
    let (days, rest) = (secs.div_euclid(86400), secs.rem_euclid(86400));
    // Civil date from days since 1970-01-01 (Howard Hinnant's algorithm)
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    format!(
        "{day} {} {year} {:02}:{:02} UTC",
        MONTHS[(month - 1) as usize],
        rest / 3600,
        rest % 3600 / 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn times() {
        assert_eq!(format_time(0), "1 Jan 1970 00:00 UTC");
        assert_eq!(format_time(951_782_400), "29 Feb 2000 00:00 UTC");
        assert_eq!(format_time(1_791_021_720), "3 Oct 2026 10:02 UTC");
        assert_eq!(format_time(-1), "31 Dec 1969 23:59 UTC");
    }

    #[test]
    fn lines() {
        let info = Info {
            version: "0.4.2",
            sha: "a1b2c3d-dirty",
            time: "3 Oct 2026 18:20 UTC",
        };
        assert_eq!(
            info.line(),
            "0.4.2 (a1b2c3d-dirty, commit 3 Oct 2026 18:20 UTC)"
        );
        assert_eq!(
            info.line_named("tool"),
            "tool 0.4.2 (a1b2c3d-dirty, commit 3 Oct 2026 18:20 UTC)"
        );
    }
}
