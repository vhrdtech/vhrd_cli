//! `collect` against real scratch repositories: a clean tree, a dirty one, a linked worktree, no repository.

use std::path::{Path, PathBuf};
use std::process::Command;
use vc_build::collect;

fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("vc_build-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn git(dir: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args(args)
        .current_dir(dir)
        .env_clear()
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .env("HOME", dir)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_AUTHOR_NAME", "t")
        .env("GIT_AUTHOR_EMAIL", "t@example.com")
        .env("GIT_COMMITTER_NAME", "t")
        .env("GIT_COMMITTER_EMAIL", "t@example.com")
        .env("GIT_AUTHOR_DATE", "2026-10-03T10:02:00+00:00")
        .env("GIT_COMMITTER_DATE", "2026-10-03T10:02:00+00:00")
        .status()
        .unwrap();
    assert!(status.success(), "git {args:?}");
}

fn repo(name: &str) -> PathBuf {
    let dir = scratch(name);
    git(&dir, &["init", "-q", "-b", "main"]);
    std::fs::write(dir.join("a.txt"), "a").unwrap();
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "first"]);
    dir
}

#[test]
fn clean_tree() {
    let dir = repo("clean");
    let info = collect(&dir);
    assert!(!info.sha.contains("dirty"), "{}", info.sha);
    assert!(info.sha.len() >= 7);
    assert_eq!(info.time, "3 Oct 2026 10:02 UTC");
    assert!(info.watch.iter().any(|p| p.ends_with(".git/HEAD")));
    assert!(info.watch.iter().any(|p| p.ends_with(".git/index")));
    assert!(
        info.watch
            .iter()
            .any(|p| p.ends_with(".git/refs/heads/main"))
    );
    assert!(info.watch.iter().all(|p| p.exists()));
}

#[test]
fn dirty_tree() {
    let dir = repo("dirty");
    std::fs::write(dir.join("a.txt"), "changed").unwrap();
    assert!(collect(&dir).sha.ends_with("-dirty"));
}

#[test]
fn worktree_watches_the_real_git_dirs() {
    let main = repo("wt-main");
    let wt = scratch("wt-linked").join("tree");
    git(
        &main,
        &["worktree", "add", "-q", "-b", "side", wt.to_str().unwrap()],
    );
    let info = collect(&wt);
    assert!(!info.sha.contains("dirty"));
    assert_eq!(info.time, "3 Oct 2026 10:02 UTC");
    let watched: Vec<String> = info.watch.iter().map(|p| p.display().to_string()).collect();
    // HEAD and the index are the worktree's own; the branch ref is shared
    assert!(
        watched.iter().any(|p| p.contains("worktrees/tree/HEAD")),
        "{watched:?}"
    );
    assert!(
        watched.iter().any(|p| p.contains("worktrees/tree/index")),
        "{watched:?}"
    );
    assert!(
        watched.iter().any(|p| p.ends_with("refs/heads/side")),
        "{watched:?}"
    );
    assert!(info.watch.iter().all(|p| p.exists()));
}

#[test]
fn no_repository() {
    // under the target dir there may be a repo above; GIT_CEILING_DIRECTORIES would stop it, but collect runs the
    // caller's environment, so use a directory outside any checkout
    let dir = std::env::temp_dir().join(format!("vc_build-norepo-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let info = collect(&dir);
    std::fs::remove_dir_all(&dir).unwrap();
    assert_eq!(info.sha, "unknown");
    assert_eq!(info.time, "unknown");
    assert!(info.watch.is_empty());
}
