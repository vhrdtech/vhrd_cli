//! The store: a directory with an append-only log and a counts file the log is folded into.
//!
//! - `log.tsv`: one line per signal of each run, `day<TAB>source<TAB>signal`, appended with one write.
//! - `counts.tsv`: one line per signal, `signal<TAB>terminal<TAB>script<TAB>agent<TAB>first<TAB>last`.
//! - `fold.tsv`: the log while it is folded; `lock`: held while folding.
//!
//! A run only appends. Once the log reaches [`Limits::log_bytes_max`], the run that finds it so folds it into the
//! counts under the lock; when another run holds the lock the record is dropped, so the log never grows past
//! the limit. The counts keep at most [`Limits::signals_max`] signals, the ones used last.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs::{self, File, OpenOptions, TryLockError};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

/// The log's size at which a run folds it into the counts; a run that cannot fold drops its record.
pub const LOG_BYTES_MAX: u64 = 64 * 1024;
/// The signals the counts file keeps; past it, the ones used longest ago go.
pub const SIGNALS_MAX: usize = 2048;
/// A signal longer than this is not recorded; it bounds a line of either file.
pub const SIGNAL_BYTES_MAX: usize = 512;

/// The environment variable that names the source for every CLI on these crates (`terminal`, `script` or
/// `agent`), for a harness that runs CLIs on someone's behalf.
pub const SOURCE_ENV: &str = "VC_USAGE_SOURCE";
/// Environment variables that mark a run inside an agent session.
pub const AGENT_ENV: &[&str] = &["CLAUDECODE"];

const LOG: &str = "log.tsv";
const FOLD: &str = "fold.tsv";
const COUNTS: &str = "counts.tsv";
const COUNTS_TMP: &str = "counts.tsv.tmp";
const LOCK: &str = "lock";
const COUNTS_HEADER: &str = "# vc_usage counts 1: signal, terminal, script, agent, first, last";

/// The bounds of a store; [`Default`] gives the named constants.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    /// The log's size at which it is folded ([`LOG_BYTES_MAX`]).
    pub log_bytes_max: u64,
    /// The signals the counts keep ([`SIGNALS_MAX`]).
    pub signals_max: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            log_bytes_max: LOG_BYTES_MAX,
            signals_max: SIGNALS_MAX,
        }
    }
}

/// Who started a run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    /// A person at a terminal: stdin is one.
    Terminal,
    /// A script, a recipe or CI: no terminal and no agent.
    Script,
    /// An agent session ([`AGENT_ENV`]).
    Agent,
}

impl Source {
    /// Every source, in the columns' order.
    pub const ALL: [Self; 3] = [Self::Terminal, Self::Script, Self::Agent];

    /// The source's word in the files and the report.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Terminal => "terminal",
            Self::Script => "script",
            Self::Agent => "agent",
        }
    }

    /// The source a word names.
    pub fn parse(word: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|s| s.name() == word)
    }

    /// This run's source: [`SOURCE_ENV`] when it names one, an agent when one of [`AGENT_ENV`] is set, a
    /// terminal when stdin is one, else a script. `env` looks a variable up (`std::env::var_os` in a binary).
    pub fn detect(env: impl Fn(&str) -> Option<OsString>, stdin_is_terminal: bool) -> Self {
        if let Some(s) = env(SOURCE_ENV).and_then(|v| v.to_str().and_then(Self::parse)) {
            return s;
        }
        if AGENT_ENV
            .iter()
            .any(|k| env(k).is_some_and(|v| !v.is_empty()))
        {
            Self::Agent
        } else if stdin_is_terminal {
            Self::Terminal
        } else {
            Self::Script
        }
    }

    const fn index(self) -> usize {
        match self {
            Self::Terminal => 0,
            Self::Script => 1,
            Self::Agent => 2,
        }
    }
}

/// A day, `YYYY-MM-DD`; its text sorts as the days do.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Day(String);

impl Day {
    /// Today, in UTC, from the system clock.
    pub fn today_utc() -> Self {
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        Self::from_unix_days(i64::try_from(secs / 86_400).unwrap_or(i64::MAX))
    }

    /// The day `days` after 1970-01-01.
    pub fn from_unix_days(days: i64) -> Self {
        let (y, m, d) = vc_build::civil_date(days);
        Self(format!("{y:04}-{m:02}-{d:02}"))
    }

    /// A `YYYY-MM-DD` day; `None` for anything else.
    ///
    /// ```
    /// assert_eq!(vc_usage::Day::parse("2026-11-15").map(|d| d.to_string()).as_deref(), Some("2026-11-15"));
    /// assert!(vc_usage::Day::parse("15.11.2026").is_none());
    /// ```
    pub fn parse(text: &str) -> Option<Self> {
        let b = text.as_bytes();
        let shape = b.len() == 10
            && b[4] == b'-'
            && b[7] == b'-'
            && b.iter()
                .enumerate()
                .all(|(i, c)| i == 4 || i == 7 || c.is_ascii_digit());
        let month: u8 = text.get(5..7)?.parse().ok()?;
        let day: u8 = text.get(8..10)?.parse().ok()?;
        (shape && (1..=12).contains(&month) && (1..=31).contains(&day))
            .then(|| Self(text.to_owned()))
    }

    /// The day as text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for Day {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// One signal's record: runs per source, the first and the last day it was used.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    /// Runs per source, in [`Source::ALL`]'s order.
    pub runs: [u64; 3],
    /// The first day.
    pub first: Day,
    /// The last day.
    pub last: Day,
}

impl Entry {
    /// The runs from `source`.
    pub fn runs_from(&self, source: Source) -> u64 {
        self.runs[source.index()]
    }

    /// The runs from every source.
    pub fn total(&self) -> u64 {
        self.runs.iter().sum()
    }

    fn add(&mut self, other: &Self) {
        for (a, b) in self.runs.iter_mut().zip(other.runs) {
            *a = a.saturating_add(b);
        }
        if other.first < self.first {
            self.first = other.first.clone();
        }
        if other.last > self.last {
            self.last = other.last.clone();
        }
    }
}

/// The counts per signal.
pub type Counts = BTreeMap<String, Entry>;

/// A store of usage records in one directory.
#[derive(Clone, Debug)]
pub struct Store {
    dir: PathBuf,
    limits: Limits,
}

impl Store {
    /// The store in `dir`, with the default [`Limits`]. Nothing is created until a record.
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self {
            dir: dir.into(),
            limits: Limits::default(),
        }
    }

    /// The same store with other limits.
    #[must_use]
    pub fn with_limits(mut self, limits: Limits) -> Self {
        self.limits = limits;
        self
    }

    /// The store's directory.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// The directory of `tool`'s store, `None` when recording is off: `<TOOL>_USAGE_DIR` (`off` turns recording
    /// off), else `$XDG_STATE_HOME/<tool>/usage`, else `$HOME/.local/state/<tool>/usage`. `env` looks a variable
    /// up (`std::env::var_os` in a binary).
    ///
    /// ```
    /// use std::ffi::OsString;
    /// use std::path::PathBuf;
    ///
    /// let env = |k: &str| (k == "HOME").then(|| OsString::from("/home/a"));
    /// assert_eq!(vc_usage::Store::locate("spl", env), Some(PathBuf::from("/home/a/.local/state/spl/usage")));
    /// let off = |k: &str| (k == "SPL_USAGE_DIR").then(|| OsString::from("off"));
    /// assert_eq!(vc_usage::Store::locate("spl", off), None);
    /// ```
    pub fn locate(tool: &str, env: impl Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
        let var = format!("{}_USAGE_DIR", tool.to_uppercase().replace('-', "_"));
        if let Some(dir) = env(&var).filter(|v| !v.is_empty()) {
            return (dir != "off").then(|| PathBuf::from(dir));
        }
        let state = env("XDG_STATE_HOME")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .or_else(|| {
                env("HOME")
                    .map(PathBuf::from)
                    .filter(|p| p.is_absolute())
                    .map(|h| h.join(".local").join("state"))
            })?;
        Some(state.join(tool).join("usage"))
    }

    /// Appends one run's signals, after folding the log when it has reached its limit. A run that finds the log
    /// full and cannot fold it (another run holds the lock) records nothing. Signals longer than
    /// [`SIGNAL_BYTES_MAX`] are skipped.
    ///
    /// # Errors
    ///
    /// The directory or a file cannot be created, read or written.
    pub fn record(&self, signals: &[String], source: Source, day: &Day) -> io::Result<()> {
        fs::create_dir_all(&self.dir)?;
        let log = self.dir.join(LOG);
        if size(&log)? >= self.limits.log_bytes_max {
            self.fold()?;
            if size(&log)? >= self.limits.log_bytes_max {
                return Ok(());
            }
        }
        let mut lines = String::new();
        for s in signals.iter().filter(|s| s.len() <= SIGNAL_BYTES_MAX) {
            let s: String = s
                .chars()
                .map(|c| if c.is_control() { ' ' } else { c })
                .collect();
            lines.push_str(&format!("{day}\t{}\t{s}\n", source.name()));
        }
        if lines.is_empty() {
            return Ok(());
        }
        // one write per run: appenders on one file do not interleave inside a line
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log)?
            .write_all(lines.as_bytes())
    }

    /// Every count: the counts file with the log (and a fold in progress) added.
    ///
    /// # Errors
    ///
    /// A file exists but cannot be read.
    pub fn read(&self) -> io::Result<Counts> {
        let mut counts = self.read_counts()?;
        for f in [FOLD, LOG] {
            add_log(
                &mut counts,
                &read_bounded(&self.dir.join(f), self.log_read_max())?,
            );
        }
        Ok(counts)
    }

    /// Folds the log into the counts under the lock; `false` when another run holds it.
    fn fold(&self) -> io::Result<bool> {
        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(self.dir.join(LOCK))?;
        match lock.try_lock() {
            Ok(()) => {}
            Err(TryLockError::WouldBlock) => return Ok(false),
            Err(TryLockError::Error(e)) => return Err(e),
        }
        let fold = self.dir.join(FOLD);
        // a fold file found under the lock is one a crashed run left: it goes first, the log in the second round
        for _ in 0..2 {
            if !fold.exists() {
                match fs::rename(self.dir.join(LOG), &fold) {
                    Ok(()) => {}
                    Err(e) if e.kind() == io::ErrorKind::NotFound => break,
                    Err(e) => return Err(e),
                }
            }
            let mut counts = self.read_counts()?;
            add_log(&mut counts, &read_bounded(&fold, self.log_read_max())?);
            let tmp = self.dir.join(COUNTS_TMP);
            fs::write(&tmp, counts_text(&counts, self.limits.signals_max))?;
            fs::rename(&tmp, self.dir.join(COUNTS))?;
            fs::remove_file(&fold)?;
        }
        Ok(true)
    }

    fn read_counts(&self) -> io::Result<Counts> {
        let max = u64::try_from(self.limits.signals_max)
            .unwrap_or(u64::MAX)
            .saturating_mul(SIGNAL_BYTES_MAX as u64 + 64);
        let text = read_bounded(&self.dir.join(COUNTS), max)?;
        Ok(text.lines().filter_map(parse_count).collect())
    }

    /// A log is read up to twice its limit: runs that appended while it was folded can take it past the limit.
    fn log_read_max(&self) -> u64 {
        self.limits.log_bytes_max.saturating_mul(2)
    }
}

fn size(path: &Path) -> io::Result<u64> {
    match fs::metadata(path) {
        Ok(m) => Ok(m.len()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(0),
        Err(e) => Err(e),
    }
}

/// The file's text up to `max` bytes, empty when it does not exist; a line cut at the end is dropped by the
/// parsers, which skip what they do not read whole.
fn read_bounded(path: &Path, max: u64) -> io::Result<String> {
    let file = match File::open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(String::new()),
        Err(e) => return Err(e),
    };
    let mut bytes = Vec::new();
    file.take(max).read_to_end(&mut bytes)?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

fn add_log(counts: &mut Counts, text: &str) {
    for line in text.lines() {
        let mut f = line.splitn(3, '\t');
        let (Some(day), Some(source), Some(signal)) = (
            f.next().and_then(Day::parse),
            f.next().and_then(Source::parse),
            f.next(),
        ) else {
            continue;
        };
        let mut runs = [0; 3];
        runs[source.index()] = 1;
        let e = Entry {
            runs,
            first: day.clone(),
            last: day,
        };
        match counts.get_mut(signal) {
            Some(c) => c.add(&e),
            None => {
                counts.insert(signal.to_owned(), e);
            }
        }
    }
}

fn parse_count(line: &str) -> Option<(String, Entry)> {
    let f: Vec<&str> = line.split('\t').collect();
    let [signal, t, s, a, first, last] = f.as_slice() else {
        return None;
    };
    Some((
        (*signal).to_owned(),
        Entry {
            runs: [t.parse().ok()?, s.parse().ok()?, a.parse().ok()?],
            first: Day::parse(first)?,
            last: Day::parse(last)?,
        },
    ))
}

/// The counts file's text: the `max` signals used last, in the signals' order.
fn counts_text(counts: &Counts, max: usize) -> String {
    let mut kept: Vec<(&String, &Entry)> = counts.iter().collect();
    if kept.len() > max {
        kept.sort_by(|a, b| b.1.last.cmp(&a.1.last).then(b.1.total().cmp(&a.1.total())));
        kept.truncate(max);
        kept.sort_by(|a, b| a.0.cmp(b.0));
    }
    let mut out = format!("{COUNTS_HEADER}\n");
    for (s, e) in kept {
        out.push_str(&format!(
            "{s}\t{}\t{}\t{}\t{}\t{}\n",
            e.runs[0], e.runs[1], e.runs[2], e.first, e.last
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A scratch directory of its own per test, removed first.
    fn scratch(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("vc_usage_{}_{name}", std::process::id()));
        fs::remove_dir_all(&d).ok();
        d
    }

    fn day(s: &str) -> Day {
        Day::parse(s).expect("a test day")
    }

    fn sigs(s: &[&str]) -> Vec<String> {
        s.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn records_add_up_per_source_and_day() {
        let store = Store::new(scratch("add-up"));
        store
            .record(
                &sigs(&["t check", "t check --fix"]),
                Source::Terminal,
                &day("2026-11-14"),
            )
            .unwrap();
        store
            .record(&sigs(&["t check"]), Source::Agent, &day("2026-11-15"))
            .unwrap();
        let c = store.read().unwrap();
        let check = &c["t check"];
        assert_eq!(check.runs, [1, 0, 1]);
        assert_eq!(
            (check.first.as_str(), check.last.as_str()),
            ("2026-11-14", "2026-11-15")
        );
        assert_eq!(c["t check --fix"].total(), 1);
        fs::remove_dir_all(store.dir()).ok();
    }

    #[test]
    fn a_full_log_is_folded_and_the_counts_stay() {
        let limits = Limits {
            log_bytes_max: 100,
            signals_max: SIGNALS_MAX,
        };
        let store = Store::new(scratch("fold")).with_limits(limits);
        for _ in 0..20 {
            store
                .record(&sigs(&["t check"]), Source::Script, &day("2026-11-15"))
                .unwrap();
        }
        let c = store.read().unwrap();
        assert_eq!(c["t check"].runs, [0, 20, 0]);
        assert!(store.dir().join(COUNTS).exists());
        // folded lazily, by the run that finds it full: one run's lines past the limit at most
        assert!(size(&store.dir().join(LOG)).unwrap() <= 100 + 26);
        assert!(!store.dir().join(FOLD).exists());
        fs::remove_dir_all(store.dir()).ok();
    }

    #[test]
    fn a_held_lock_drops_the_record_instead_of_growing_the_log() {
        let limits = Limits {
            log_bytes_max: 60,
            signals_max: SIGNALS_MAX,
        };
        let store = Store::new(scratch("locked")).with_limits(limits);
        for _ in 0..3 {
            store
                .record(&sigs(&["t check"]), Source::Script, &day("2026-11-15"))
                .unwrap();
        }
        let held = File::create(store.dir().join(LOCK)).unwrap();
        held.lock().unwrap();
        let before = size(&store.dir().join(LOG)).unwrap();
        assert!(before >= 60);
        store
            .record(&sigs(&["t check"]), Source::Script, &day("2026-11-15"))
            .unwrap();
        assert_eq!(size(&store.dir().join(LOG)).unwrap(), before);
        held.unlock().unwrap();
        fs::remove_dir_all(store.dir()).ok();
    }

    #[test]
    fn a_fold_left_by_a_crash_is_folded_once() {
        let store = Store::new(scratch("crash")).with_limits(Limits {
            log_bytes_max: 40,
            signals_max: SIGNALS_MAX,
        });
        fs::create_dir_all(store.dir()).unwrap();
        fs::write(store.dir().join(FOLD), "2026-11-10\tscript\tt old\n").unwrap();
        // read sees it while it waits
        assert_eq!(store.read().unwrap()["t old"].total(), 1);
        for _ in 0..3 {
            store
                .record(&sigs(&["t new"]), Source::Script, &day("2026-11-15"))
                .unwrap();
        }
        let c = store.read().unwrap();
        assert_eq!(c["t old"].total(), 1);
        assert_eq!(c["t new"].total(), 3);
        fs::remove_dir_all(store.dir()).ok();
    }

    #[test]
    fn the_counts_keep_the_signals_used_last() {
        let mut c = Counts::new();
        for (s, last) in [
            ("a", "2026-11-01"),
            ("b", "2026-11-03"),
            ("c", "2026-11-02"),
        ] {
            c.insert(
                s.to_owned(),
                Entry {
                    runs: [1, 0, 0],
                    first: day("2026-10-01"),
                    last: day(last),
                },
            );
        }
        let text = counts_text(&c, 2);
        let kept: Vec<&str> = text.lines().skip(1).map(|l| &l[..1]).collect();
        assert_eq!(kept, ["b", "c"]);
        assert_eq!(text.lines().filter_map(parse_count).count(), 2);
    }

    #[test]
    fn torn_and_foreign_lines_are_skipped() {
        let mut c = Counts::new();
        add_log(
            &mut c,
            "2026-11-15\tscript\tt a\ngarbage\n2026-11-15\tnobody\tt b\n2026-11-1",
        );
        assert_eq!(c.keys().collect::<Vec<_>>(), ["t a"]);
        assert!(parse_count("t a\t1\t2").is_none());
    }

    #[test]
    fn long_and_control_signals() {
        let store = Store::new(scratch("long"));
        let long = "x".repeat(SIGNAL_BYTES_MAX + 1);
        store
            .record(
                &[long, "t a\tb\nc".to_owned()],
                Source::Script,
                &day("2026-11-15"),
            )
            .unwrap();
        let c = store.read().unwrap();
        assert_eq!(c.keys().collect::<Vec<_>>(), ["t a b c"]);
        fs::remove_dir_all(store.dir()).ok();
    }

    #[test]
    fn sources() {
        let none = |_: &str| None;
        assert_eq!(Source::detect(none, true), Source::Terminal);
        assert_eq!(Source::detect(none, false), Source::Script);
        let agent = |k: &str| (k == "CLAUDECODE").then(|| OsString::from("1"));
        assert_eq!(Source::detect(agent, true), Source::Agent);
        let named = |k: &str| (k == SOURCE_ENV).then(|| OsString::from("script"));
        assert_eq!(Source::detect(named, true), Source::Script);
    }

    #[test]
    fn locations() {
        let env = |pairs: &'static [(&'static str, &'static str)]| {
            move |k: &str| {
                pairs
                    .iter()
                    .find(|(n, _)| *n == k)
                    .map(|(_, v)| OsString::from(v))
            }
        };
        assert_eq!(
            Store::locate("my-tool", env(&[("MY_TOOL_USAGE_DIR", "/s/u")])),
            Some(PathBuf::from("/s/u"))
        );
        assert_eq!(
            Store::locate("t", env(&[("XDG_STATE_HOME", "/x"), ("HOME", "/h")])),
            Some(PathBuf::from("/x/t/usage"))
        );
        assert_eq!(
            Store::locate("t", env(&[("XDG_STATE_HOME", "rel"), ("HOME", "/h")])),
            Some(PathBuf::from("/h/.local/state/t/usage"))
        );
        assert_eq!(Store::locate("t", env(&[])), None);
    }

    #[test]
    fn days() {
        assert_eq!(Day::from_unix_days(20_772).as_str(), "2026-11-15");
        assert!(Day::parse("2026-13-01").is_none());
        assert!(Day::parse("2026-1-011").is_none());
        assert!(Day::parse("2026-11-15x").is_none());
    }
}
