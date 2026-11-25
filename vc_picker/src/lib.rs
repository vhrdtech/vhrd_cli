//! A fuzzy picker drawn below the shell prompt, for Tab completion and key bindings.
//!
//! It is drawn inline on `/dev/tty` (like fzf's `--height`, about three fifths of the terminal, not the alternate
//! screen), so it also runs from inside a completion function or a key binding whose stdout the shell reads. Rows
//! sit in [`Column`]s with a header, typing filters them, the matched letters are lit, and a bottom line describes
//! the selected row. The rules it follows are in the CLI UX guide, `docs/ux-guide.md` section 5.1.
//!
//! - **Rows** are [`Item`]s made by the caller from its own typed rows: a cell per column, more words to match, the
//!   bottom line.
//! - **Matching:** the caller's own rule first ([`Source`], best first), then every other row that fuzzy matching
//!   takes, best score first, with fzf's syntax: `'exact`, `^start`, `end$`, `!not`.
//! - **Keys:** type to filter; Up/Down, Ctrl-P/N, Tab/Shift-Tab, PgUp/PgDn, Home/End move; Backspace, Ctrl-W,
//!   Ctrl-U edit the query; Enter takes the row ([`Action::Accept`]); Ctrl-Y or Alt-Enter takes its other form
//!   ([`Action::Alternate`], when the picker has a hint for it); Esc, Ctrl-C, Ctrl-G cancel.
//!
//! Three ways in: [`run`] draws the picker, [`tab`] runs it inside a completer and gives the candidates, and a key
//! binding (`vc_complete::Completion::pick_widget`, `vc_complete::rc::Block::pick_key`) runs a subcommand of the
//! CLI that calls [`run`] and prints the pick.
//!
//! ```
//! use vc_picker::{Column, Fuzzy, Item, Picker, Width};
//!
//! const COLS: [Column; 2] = [
//!     Column { head: "serial", width: Width::Fit { max: 24 }, keep: 9 },
//!     Column { head: "product", width: Width::Fill { min: 10, weight: 1, left: false }, keep: 5 },
//! ];
//! let rows = [("2100AB", "Nucleo-G0B1"), ("3300CD", "Nucleo-H743")]
//!     .map(|(s, p)| Item { cells: vec![s.into(), p.into()], ..Item::default() })
//!     .to_vec();
//! let p = Picker::new(&COLS, rows, &Fuzzy, "h7");
//! assert_eq!(p.rows(), [1]);
//! ```
//!
//! The heavy parts (ratatui with crossterm, nucleo's matcher) live in this crate only, so a CLI without a picker
//! does not build them.

mod draw;
mod layout;
mod state;
mod tab;
mod term;

pub use state::{Action, Column, Fuzzy, Item, Outcome, Picker, Source, Width};
pub use tab::tab;
pub use term::{PickError, color, run};
