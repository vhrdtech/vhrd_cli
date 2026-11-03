//! The completion lines in a user's shell config: one marked block per file that registers the completion of a
//! list of CLIs, each only where it is installed. A tool's setup step writes it, instead of a person by hand.
//!
//! The block is rewritten only when it differs, so it is added once and never piles up, and the hand-written lines
//! it replaces (`source <(COMPLETE=bash mycli)`) are taken out. Everything else in the file stays as it was.
//!
//! ```
//! use vc_complete::rc::{Block, Shell};
//!
//! let block = Block::new("mytools", &["mycli", "othercli"]);
//! let rc = block.update("alias ll='ls -l'\n", Shell::Bash).expect("the block is added");
//! assert!(rc.contains("command -v mycli >/dev/null 2>&1 && source <(COMPLETE=bash mycli)"));
//! assert_eq!(block.update(&rc, Shell::Bash), None, "and only once");
//! ```

use std::path::{Path, PathBuf};

/// A shell whose config file gets the block.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shell {
    /// `~/.bashrc`
    Bash,
    /// `~/.zshrc`
    Zsh,
    /// `~/.config/fish/config.fish`
    Fish,
}

impl Shell {
    /// Every shell with a block.
    pub const ALL: [Shell; 3] = [Shell::Bash, Shell::Zsh, Shell::Fish];

    /// The shell's name, as `COMPLETE=` takes it.
    ///
    /// ```
    /// assert_eq!(vc_complete::rc::Shell::Fish.name(), "fish");
    /// ```
    pub fn name(self) -> &'static str {
        match self {
            Shell::Bash => "bash",
            Shell::Zsh => "zsh",
            Shell::Fish => "fish",
        }
    }

    /// The config file, relative to the home directory.
    ///
    /// ```
    /// assert_eq!(vc_complete::rc::Shell::Zsh.rc(), ".zshrc");
    /// ```
    pub fn rc(self) -> &'static str {
        match self {
            Shell::Bash => ".bashrc",
            Shell::Zsh => ".zshrc",
            Shell::Fish => ".config/fish/config.fish",
        }
    }

    /// Whether the user of `home` uses this shell: its config file or directory exists. bash always counts.
    ///
    /// ```
    /// use vc_complete::rc::Shell;
    /// assert!(Shell::Bash.in_use("/nonexistent".as_ref()));
    /// assert!(!Shell::Fish.in_use("/nonexistent".as_ref()));
    /// ```
    pub fn in_use(self, home: &Path) -> bool {
        match self {
            Shell::Bash => true,
            Shell::Zsh => home.join(".zshrc").exists(),
            Shell::Fish => home.join(".config/fish").is_dir(),
        }
    }

    /// The line that registers one CLI's completion, silent where the CLI is not installed.
    fn line(self, cli: &str) -> String {
        match self {
            Shell::Bash | Shell::Zsh => format!(
                "command -v {cli} >/dev/null 2>&1 && source <(COMPLETE={} {cli})",
                self.name()
            ),
            Shell::Fish => format!("type -q {cli}; and COMPLETE=fish {cli} | source"),
        }
    }

    /// The hand-written line of one CLI that the block replaces.
    fn old_line(self, cli: &str) -> String {
        match self {
            Shell::Fish => format!("COMPLETE=fish {cli} | source"),
            _ => format!("source <(COMPLETE={} {cli})", self.name()),
        }
    }
}

/// The marked block: its name (in the marker lines), the CLIs it registers, and who manages it.
#[derive(Clone, Debug)]
pub struct Block<'a> {
    name: &'a str,
    clis: &'a [&'a str],
    managed_by: &'a str,
}

impl<'a> Block<'a> {
    /// A block named `name` that registers the completion of `clis`, in this order. The name tells blocks of
    /// different tools apart, so keep it the same from one version to the next.
    ///
    /// ```
    /// let text = vc_complete::rc::Block::new("mytools", &["mycli"]).text(vc_complete::rc::Shell::Fish);
    /// assert_eq!(text.lines().nth(1), Some("type -q mycli; and COMPLETE=fish mycli | source"));
    /// ```
    pub fn new(name: &'a str, clis: &'a [&'a str]) -> Self {
        Block {
            name,
            clis,
            managed_by: name,
        }
    }

    /// The program named in the first marker line as the one that writes the block (the name by default), so a
    /// person reading the config knows where a change belongs.
    ///
    /// ```
    /// let b = vc_complete::rc::Block::new("mytools", &["mycli"]).managed_by("mytools setup");
    /// assert!(b.text(vc_complete::rc::Shell::Bash).starts_with("# >>> mytools completion (managed by mytools setup,"));
    /// ```
    pub fn managed_by(mut self, program: &'a str) -> Self {
        self.managed_by = program;
        self
    }

    fn begin(&self) -> String {
        format!(
            "# >>> {} completion (managed by {}, rewritten when it changes) >>>",
            self.name, self.managed_by
        )
    }

    fn begin_prefix(&self) -> String {
        format!("# >>> {} completion", self.name)
    }

    fn end_prefix(&self) -> String {
        format!("# <<< {} completion", self.name)
    }

    /// The block for a shell, marker lines included.
    ///
    /// ```
    /// use vc_complete::rc::{Block, Shell};
    /// let text = Block::new("mytools", &["mycli"]).text(Shell::Bash);
    /// assert!(text.ends_with("# <<< mytools completion <<<\n"));
    /// ```
    pub fn text(&self, shell: Shell) -> String {
        let mut s = format!("{}\n", self.begin());
        for cli in self.clis {
            s.push_str(&shell.line(cli));
            s.push('\n');
        }
        s.push_str(&format!("{} <<<\n", self.end_prefix()));
        s
    }

    /// The config text with the block in place: added at the end when missing, replaced where it is when it
    /// differs, and the hand-written lines it replaces taken out. `None` when nothing changes. A block whose end
    /// marker was lost has only its first line replaced, so no line after it is lost.
    ///
    /// ```
    /// use vc_complete::rc::{Block, Shell};
    /// let b = Block::new("mytools", &["mycli"]);
    /// let new = b.update("source <(COMPLETE=zsh mycli)\n", Shell::Zsh).unwrap();
    /// assert!(!new.starts_with("source"));
    /// ```
    pub fn update(&self, text: &str, shell: Shell) -> Option<String> {
        let want = self.text(shell);
        let old: Vec<String> = self.clis.iter().map(|c| shell.old_line(c)).collect();
        let (begin, end) = (self.begin_prefix(), self.end_prefix());
        let lines: Vec<&str> = text.split_inclusive('\n').collect();
        let bare = |l: &str| l.trim_end_matches(['\n', '\r']).to_string();
        let mut out = String::new();
        let mut placed = false;
        let mut i = 0;
        while i < lines.len() {
            let line = bare(lines[i]);
            if line.starts_with(&begin) {
                if !placed {
                    out.push_str(&want);
                    placed = true;
                }
                // up to and with the end marker; without one only this line goes
                i = match lines[i + 1..]
                    .iter()
                    .position(|l| bare(l).starts_with(&end))
                {
                    Some(n)
                        if !lines[i + 1..i + 1 + n]
                            .iter()
                            .any(|l| bare(l).starts_with(&begin)) =>
                    {
                        i + n + 2
                    }
                    _ => i + 1,
                };
                continue;
            }
            if !old.iter().any(|o| *o == line.trim()) {
                out.push_str(lines[i]);
            }
            i += 1;
        }
        if !placed {
            if !out.is_empty() && !out.ends_with('\n') {
                out.push('\n');
            }
            if !out.is_empty() {
                out.push('\n');
            }
            out.push_str(&want);
        }
        (out != text).then_some(out)
    }

    /// The shells the user of `home` uses whose config lacks the current block.
    ///
    /// ```
    /// use vc_complete::rc::{Block, Shell};
    /// let home = std::path::Path::new("/nonexistent");
    /// assert_eq!(Block::new("mytools", &["mycli"]).missing(home), [Shell::Bash]);
    /// ```
    pub fn missing(&self, home: &Path) -> Vec<Shell> {
        Shell::ALL
            .into_iter()
            .filter(|s| s.in_use(home))
            .filter(|s| {
                let text = std::fs::read_to_string(home.join(s.rc())).unwrap_or_default();
                self.update(&text, *s).is_some()
            })
            .collect()
    }

    /// Puts the block into the configs of `shells` under `home` and returns the files that changed. A file that
    /// changes keeps its old text beside it as `<file>.<name>-bak`. The file is written in place, so a config that
    /// is a link into a dotfiles repo stays a link.
    ///
    /// ```
    /// use vc_complete::rc::{Block, Shell};
    /// let home = std::env::temp_dir().join(format!("vc-complete-doc-{}", std::process::id()));
    /// std::fs::create_dir_all(&home)?;
    /// let block = Block::new("mytools", &["mycli"]);
    /// assert_eq!(block.apply(&home, &[Shell::Bash])?, [home.join(".bashrc")]);
    /// assert!(block.apply(&home, &[Shell::Bash])?.is_empty());
    /// std::fs::remove_dir_all(&home)?;
    /// # Ok::<_, Box<dyn std::error::Error>>(())
    /// ```
    pub fn apply(&self, home: &Path, shells: &[Shell]) -> Result<Vec<PathBuf>, RcError> {
        let mut changed = vec![];
        for s in shells {
            let path = home.join(s.rc());
            let at = |e| RcError {
                path: path.clone(),
                source: e,
            };
            let text = match std::fs::read_to_string(&path) {
                Ok(t) => t,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
                Err(e) => return Err(at(e)),
            };
            let Some(new) = self.update(&text, *s) else {
                continue;
            };
            if let Some(dir) = path.parent() {
                std::fs::create_dir_all(dir).map_err(at)?;
            }
            if !text.is_empty() {
                let mut bak = path.clone().into_os_string();
                bak.push(format!(".{}-bak", self.name));
                std::fs::write(&bak, &text).map_err(|e| RcError {
                    path: bak.into(),
                    source: e,
                })?;
            }
            std::fs::write(&path, new).map_err(at)?;
            changed.push(path);
        }
        Ok(changed)
    }
}

/// A shell config that could not be read or written.
#[derive(Debug)]
pub struct RcError {
    /// The file.
    pub path: PathBuf,
    /// What went wrong.
    pub source: std::io::Error,
}

impl std::fmt::Display for RcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.path.display(), self.source)
    }
}

impl std::error::Error for RcError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.source)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CLIS: &[&str] = &["todo", "ww"];

    fn block() -> Block<'static> {
        Block::new("todo", CLIS).managed_by("demo setup")
    }

    #[test]
    fn adds_the_block_once() {
        let first = block().update("alias x=y\n", Shell::Bash).unwrap();
        assert!(first.starts_with(
            "alias x=y\n\n# >>> todo completion (managed by demo setup, rewritten when it changes) >>>\n"
        ));
        assert!(
            first.contains("command -v todo >/dev/null 2>&1 && source <(COMPLETE=bash todo)\n")
        );
        assert!(first.contains("command -v ww >/dev/null 2>&1 && source <(COMPLETE=bash ww)\n"));
        assert_eq!(
            block().update(&first, Shell::Bash),
            None,
            "second run changes nothing"
        );
        let empty = block().update("", Shell::Bash).unwrap();
        assert!(empty.starts_with("# >>> todo completion"));
        let unterminated = block().update("x", Shell::Bash).unwrap();
        assert!(unterminated.starts_with("x\n\n# >>> todo completion"));
    }

    #[test]
    fn replaces_an_old_block_in_place() {
        let text =
            "a\n# >>> todo completion (older text) >>>\nold line\n# <<< todo completion <<<\nb\n";
        let new = block().update(text, Shell::Zsh).unwrap();
        assert!(new.starts_with("a\n# >>> todo completion (managed by"));
        assert!(new.ends_with("# <<< todo completion <<<\nb\n"));
        assert!(!new.contains("old line"));
        assert_eq!(new.matches("# >>> todo completion").count(), 1);
        assert!(new.contains("source <(COMPLETE=zsh ww)"));
    }

    #[test]
    fn a_second_block_is_dropped() {
        let once = block().text(Shell::Bash);
        let text = format!("a\n{once}b\n{once}c\n");
        assert_eq!(
            block().update(&text, Shell::Bash).unwrap(),
            format!("a\n{once}b\nc\n")
        );
    }

    #[test]
    fn a_lost_end_marker_loses_no_lines() {
        let text =
            "a\n# >>> todo completion (managed by demo setup) >>>\nalias keep=me\nexport X=1\n";
        let new = block().update(text, Shell::Bash).unwrap();
        assert!(new.contains("alias keep=me\nexport X=1\n"), "{new}");
        assert!(new.starts_with("a\n# >>> todo completion"));
    }

    #[test]
    fn other_blocks_are_left_alone() {
        let other = Block::new("mytools", &["mycli"]).text(Shell::Bash);
        let new = block().update(&other, Shell::Bash).unwrap();
        assert!(new.starts_with(&other));
        assert_eq!(new.matches("# >>> ").count(), 2);
    }

    #[test]
    fn removes_hand_written_lines() {
        let text = "x\nsource <(COMPLETE=bash todo)\n  source <(COMPLETE=bash todo)\nsource <(COMPLETE=bash ww)\nsource <(COMPLETE=bash other)\ny\n";
        let new = block().update(text, Shell::Bash).unwrap();
        assert!(new.starts_with("x\nsource <(COMPLETE=bash other)\ny\n\n# >>> todo"));
        let fish = block()
            .update("COMPLETE=fish ww | source\n", Shell::Fish)
            .unwrap();
        assert!(!fish.starts_with("COMPLETE"));
    }

    #[test]
    fn fish_uses_its_own_syntax() {
        let new = block().update("", Shell::Fish).unwrap();
        assert!(new.contains("type -q todo; and COMPLETE=fish todo | source\n"));
    }

    #[test]
    fn apply_backs_up_and_is_idempotent() {
        let home = std::env::temp_dir().join(format!("vc-complete-rc-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&home);
        std::fs::create_dir_all(home.join(".config/fish")).unwrap();
        std::fs::write(home.join(".bashrc"), "alias a=b\n").unwrap();
        assert_eq!(block().missing(&home), [Shell::Bash, Shell::Fish]);
        let changed = block().apply(&home, &[Shell::Bash, Shell::Fish]).unwrap();
        assert_eq!(
            changed,
            [home.join(".bashrc"), home.join(".config/fish/config.fish")]
        );
        assert_eq!(
            std::fs::read_to_string(home.join(".bashrc.todo-bak")).unwrap(),
            "alias a=b\n"
        );
        assert!(
            !home.join(".config/fish/config.fish.todo-bak").exists(),
            "no backup of a new file"
        );
        assert!(block().missing(&home).is_empty());
        assert!(
            block().apply(&home, &Shell::ALL).unwrap().len() == 1,
            "zsh is new"
        );
        assert!(block().apply(&home, &Shell::ALL).unwrap().is_empty());
        std::fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn apply_names_the_file_it_could_not_read() {
        let home = std::env::temp_dir().join(format!("vc-complete-rc-err-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&home);
        std::fs::create_dir_all(home.join(".bashrc")).unwrap(); // a directory, not a file
        let err = block().apply(&home, &[Shell::Bash]).unwrap_err();
        assert_eq!(err.path, home.join(".bashrc"));
        assert!(
            err.to_string()
                .starts_with(&home.join(".bashrc").display().to_string())
        );
        std::fs::remove_dir_all(&home).unwrap();
    }
}
