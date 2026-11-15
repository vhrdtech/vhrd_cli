//! The report: every signal of the tree with its counts, the unused ones included.

use crate::store::{Counts, Entry, Source};
use vc_ui::Paint;

/// One line of the report.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    /// The signal, `spl check --fix`.
    pub signal: String,
    /// Its counts; `None` when it was never used.
    pub entry: Option<Entry>,
    /// `false` for a signal in the records that this version's tree no longer has.
    pub known: bool,
}

/// The rows: every signal of `known` in its order, then the recorded signals the tree no longer has.
pub fn rows(known: &[String], counts: &Counts) -> Vec<Row> {
    let mut out: Vec<Row> = known
        .iter()
        .map(|s| Row {
            signal: s.clone(),
            entry: counts.get(s).cloned(),
            known: true,
        })
        .collect();
    out.extend(
        counts
            .iter()
            .filter(|(s, _)| !known.contains(s))
            .map(|(s, e)| Row {
                signal: s.clone(),
                entry: Some(e.clone()),
                known: false,
            }),
    );
    out
}

/// The rows as a table: runs in all and per source, the last day, the signal. A never-used signal is dimmed, one
/// the tree no longer has is marked.
///
/// ```
/// let rows = vc_usage::rows(&["tool check".to_owned()], &Default::default());
/// assert_eq!(
///     vc_usage::text(&rows, vc_ui::Paint::OFF),
///     "runs  terminal  script  agent  last        signal\n   0         -       -      -  never       tool check\n"
/// );
/// ```
pub fn text(rows: &[Row], paint: Paint) -> String {
    let mut out = paint.paint(
        vc_ui::HEADING,
        "runs  terminal  script  agent  last        signal",
    );
    out.push('\n');
    for r in rows {
        let line = match &r.entry {
            Some(e) => format!(
                "{:>4}  {:>8}  {:>6}  {:>5}  {}  {}",
                e.total(),
                e.runs_from(Source::Terminal),
                e.runs_from(Source::Script),
                e.runs_from(Source::Agent),
                paint.date(e.last.as_str()),
                r.signal
            ),
            None => paint.dim(&format!(
                "{:>4}  {:>8}  {:>6}  {:>5}  {:10}  {}",
                0, "-", "-", "-", "never", r.signal
            )),
        };
        out.push_str(&line);
        if !r.known {
            out.push_str(&paint.dim("  (not in this version)"));
        }
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::Day;

    fn entry(t: u64, last: &str) -> Entry {
        Entry {
            runs: [t, 1, 0],
            first: Day::parse("2026-11-01").expect("a day"),
            last: Day::parse(last).expect("a day"),
        }
    }

    #[test]
    fn known_first_then_gone() {
        let mut c = Counts::new();
        c.insert("t check".to_owned(), entry(2, "2026-11-15"));
        c.insert("t old".to_owned(), entry(0, "2026-11-02"));
        let rows = rows(&["t check".to_owned(), "t init".to_owned()], &c);
        assert_eq!(
            text(&rows, Paint::OFF),
            "runs  terminal  script  agent  last        signal\n   \
             3         2       1      0  2026-11-15  t check\n   \
             0         -       -      -  never       t init\n   \
             1         0       1      0  2026-11-02  t old  (not in this version)\n"
        );
    }
}
