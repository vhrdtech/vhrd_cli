//! The docs other repos rely on (REPO-1, GUIDE-1): the AGENTS.md files of the CLI repos link to
//! `docs/ux-guide.md`, and the guide links to vhrd_egui for the rules every UI shares. The repo is public, so its
//! Markdown carries no private paths.

use std::fs;
use std::path::{Path, PathBuf};

const GUIDE: &str = "docs/ux-guide.md";

/// The section headings of the guide, in order. Other pages link to them by anchor.
const SECTIONS: &[&str] = &[
    "## 1. Short version",
    "## 2. Rules for every UI",
    "## 3. Shape of the command line",
    "## 4. Help and discovery",
    "## 5. Shell completion",
    "## 6. Output",
    "## 7. Errors and exit codes",
    "## 8. Long runs: progress, deploys and transfers",
    "## 9. Prompts, confirmation and dry runs",
    "## 10. Configuration and environment",
    "## 11. Version, usage and the skill tree",
    "## 12. Testing a CLI",
    "## 13. Checklists",
    "## Sources",
    "## Where the rules point here",
];

const SHARED_RULES_LINK: &str =
    "https://github.com/vhrdtech/vhrd_egui/blob/main/docs/ux-guide.md#2-rules-for-every-ui";

/// Text that points into a private home folder or the private project tracker.
const PRIVATE: &[&str] = &["/home/", "~/git", "~/sync", "area/ops", "prj/P"];

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("repo_checks sits in the workspace root")
        .to_path_buf()
}

fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn markdown_files() -> Vec<PathBuf> {
    let root = repo_root();
    let mut files = Vec::new();
    for dir in [root.clone(), root.join("docs")] {
        for entry in fs::read_dir(&dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display())) {
            let path = entry.expect("readable dir entry").path();
            if path.extension().is_some_and(|ext| ext == "md") {
                files.push(path);
            }
        }
    }
    files.sort();
    files
}

#[test]
fn guide_is_at_the_path_other_repos_link_to() {
    assert!(repo_root().join(GUIDE).is_file(), "{GUIDE} is missing");
}

#[test]
fn guide_keeps_its_sections_in_order() {
    let guide = read(&repo_root().join(GUIDE));
    let headings: Vec<&str> = guide.lines().filter(|l| l.starts_with("## ")).collect();
    assert_eq!(headings, SECTIONS);
}

#[test]
fn guide_links_to_the_shared_ui_rules() {
    let guide = read(&repo_root().join(GUIDE));
    assert!(
        guide.contains(SHARED_RULES_LINK),
        "{GUIDE} must link to {SHARED_RULES_LINK}"
    );
}

#[test]
fn agents_md_links_to_the_guide() {
    let agents = read(&repo_root().join("AGENTS.md"));
    assert!(agents.contains(GUIDE), "AGENTS.md must link to {GUIDE}");
}

#[test]
fn markdown_has_no_private_paths() {
    let files = markdown_files();
    assert!(files.len() >= 5, "found only {files:?}");
    let mut hits = Vec::new();
    for path in &files {
        for (n, line) in read(path).lines().enumerate() {
            for needle in PRIVATE {
                if line.contains(needle) {
                    hits.push(format!("{}:{}: {needle}", path.display(), n + 1));
                }
            }
        }
    }
    assert!(
        hits.is_empty(),
        "private paths in public docs:\n{}",
        hits.join("\n")
    );
}
