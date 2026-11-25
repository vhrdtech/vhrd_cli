//! The picker on Tab: a completer that has several hits opens it, and what is picked is the one candidate.

use crate::state::{Action, Outcome, Picker};
use clap_complete::CompletionCandidate;

/// The candidates a completer gives when Tab opens the picker: the picked row's value as the one candidate, marked
/// [`vc_complete::PICKED`] so every shell puts it in place of the typed word; no candidate when it was cancelled.
/// `None` where the picker does not run, so the completer gives its plain candidates: a shell other than bash, zsh
/// or fish, no terminal, or `TOOL_TAB_PICKER=0` ([`vc_complete::tab_picker`]).
///
/// `picker` is only called when the picker runs; `value` turns the picked row and how it was taken into the text
/// that goes on the line.
///
/// ```no_run
/// use clap_complete::CompletionCandidate;
/// use vc_picker::{Action, Column, Fuzzy, Item, Picker, Width};
///
/// const COLS: [Column; 2] = [
///     Column { head: "serial", width: Width::Fit { max: 24 }, keep: 9 },
///     Column { head: "product", width: Width::Fill { min: 10, weight: 1, left: false }, keep: 5 },
/// ];
///
/// fn devices(typed: &std::ffi::OsStr) -> Vec<CompletionCandidate> {
///     let found = [("2100AB", "Nucleo-G0B1"), ("3300CD", "Nucleo-H743")];
///     let typed = typed.to_string_lossy();
///     let hits: Vec<_> = found.iter().filter(|(s, _)| s.starts_with(&*typed)).collect();
///     if hits.len() > 1 {
///         let rows = found.iter().map(|(s, p)| Item { cells: vec![s.to_string(), p.to_string()], ..Item::default() });
///         let picked = vc_picker::tab(
///             "mycli",
///             || Picker::new(&COLS, rows.collect(), &Fuzzy, &typed),
///             |row, _how: Action| found[row].0.to_owned(),
///         );
///         if let Some(candidates) = picked {
///             return candidates;
///         }
///     }
///     hits.iter().map(|(s, p)| vc_complete::cand(*s, *p)).collect()
/// }
/// ```
pub fn tab<'a>(
    tool: &str,
    picker: impl FnOnce() -> Picker<'a>,
    value: impl FnOnce(usize, Action) -> String,
) -> Option<Vec<CompletionCandidate>> {
    if !vc_complete::tab_picker(tool) {
        return None;
    }
    let mut p = picker();
    match crate::run(&mut p, crate::color()) {
        Ok(Outcome::Picked(row, how)) => Some(vec![
            vc_complete::cand(value(row, how), "").id(Some(vc_complete::PICKED.into())),
        ]),
        Ok(Outcome::Cancelled) => Some(Vec::new()),
        Err(_) => None,
    }
}
