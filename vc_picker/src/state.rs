//! The picker's state: the rows, the query, what passes it and which row is selected, and what a key does to it.

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher, Utf32String};

/// How a column takes its width.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Width {
    /// As wide as its widest cell (or header), at most `max`.
    Fit {
        /// The widest it gets.
        max: usize,
    },
    /// A share of what is left, by weight, at least `min`. A long cell is cut at its end, or with `left` at its
    /// start (a path keeps its file name).
    Fill {
        /// The narrowest it gets while it is shown.
        min: usize,
        /// Its share of the room against the other fill columns.
        weight: usize,
        /// Cut a long cell at its start instead of its end.
        left: bool,
    },
}

/// A column of the picker: its header, how it takes its width, and how much it is needed.
///
/// ```
/// use vc_picker::{Column, Width};
/// const NAME: Column = Column { head: "name", width: Width::Fit { max: 20 }, keep: 9 };
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Column {
    /// The header.
    pub head: &'static str,
    /// How it takes its width.
    pub width: Width,
    /// When the terminal is too narrow, the columns with the lowest `keep` are left out first.
    pub keep: u8,
}

/// One row as the picker shows it: a cell per column, more words to match, and the bottom line.
///
/// ```
/// let row = vc_picker::Item {
///     cells: vec!["2100AB".into(), "Nucleo-G0B1".into()],
///     extra: "st-link".into(),
///     detail: "/dev/ttyACM0".into(),
///     dim: false,
/// };
/// ```
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Item {
    /// One cell per [`Column`], in their order.
    pub cells: Vec<String>,
    /// Matched too, never shown: other words for the row.
    pub extra: String,
    /// The bottom line while the row is selected.
    pub detail: String,
    /// Drawn dim, such as an older revision of a document.
    pub dim: bool,
}

/// The caller's own rule of which rows a query finds, best first. Its rows come before the fuzzy matches.
///
/// ```
/// struct ByPrefix(Vec<String>);
/// impl vc_picker::Source for ByPrefix {
///     fn find(&self, query: &str) -> Vec<usize> {
///         (0..self.0.len()).filter(|&i| self.0[i].starts_with(query)).collect()
///     }
/// }
/// ```
pub trait Source {
    /// The rows (indices into the items) that `query` finds, best first.
    fn find(&self, query: &str) -> Vec<usize>;
}

/// No rule of its own: fuzzy matching alone.
#[derive(Clone, Copy, Debug, Default)]
pub struct Fuzzy;

impl Source for Fuzzy {
    fn find(&self, _query: &str) -> Vec<usize> {
        Vec::new()
    }
}

/// How a row was taken.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    /// Enter.
    Accept,
    /// Ctrl-Y or Alt-Enter: the other form of the value, such as the full path instead of a name. Only when the
    /// picker has a hint for it ([`Picker::hints`]).
    Alternate,
}

/// How the picker ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// The row (index into the items) and how it was taken.
    Picked(usize, Action),
    /// Esc, Ctrl-C, Ctrl-G or Ctrl-Q.
    Cancelled,
}

/// A row that passes the query, with the positions in its haystack that matched.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Shown {
    pub(crate) item: usize,
    pub(crate) hl: Vec<u32>,
}

/// The picker: rows in columns, a query that filters them, a selected row.
///
/// ```
/// use vc_picker::{Column, Fuzzy, Item, Picker, Width};
/// const COLS: [Column; 1] = [Column { head: "name", width: Width::Fit { max: 20 }, keep: 1 }];
/// let rows = ["alpha", "beta", "gamma"].map(|n| Item { cells: vec![n.into()], ..Item::default() }).to_vec();
/// let p = Picker::new(&COLS, rows, &Fuzzy, "gm");
/// assert_eq!(p.rows(), [2]);
/// assert_eq!(p.selected(), Some(2));
/// ```
pub struct Picker<'a> {
    pub(crate) columns: &'a [Column],
    pub(crate) items: Vec<Item>,
    source: &'a dyn Source,
    hay: Vec<Utf32String>,
    /// Where each cell starts in its row's haystack, in chars.
    starts: Vec<Vec<u32>>,
    pub(crate) query: String,
    pub(crate) shown: Vec<Shown>,
    pub(crate) selected: usize,
    /// The first shown row on screen.
    pub(crate) top: usize,
    matcher: Matcher,
    /// What Enter does, for the hint line.
    pub(crate) enter: &'static str,
    /// What Ctrl-Y does; `None`: nothing.
    pub(crate) alternate: Option<&'static str>,
}

impl<'a> Picker<'a> {
    /// A picker over `items` in `columns`, with `query` typed already. Rows the `source` finds come first, then the
    /// fuzzy matches by score. Enter's hint says `insert` until [`Picker::hints`] says otherwise.
    pub fn new(
        columns: &'a [Column],
        items: Vec<Item>,
        source: &'a dyn Source,
        query: &str,
    ) -> Self {
        let mut hay = Vec::with_capacity(items.len());
        let mut starts = Vec::with_capacity(items.len());
        for it in &items {
            let mut s = String::new();
            let mut st = Vec::with_capacity(it.cells.len());
            for c in &it.cells {
                st.push(s.chars().count() as u32);
                s.push_str(c);
                s.push(' ');
            }
            s.push_str(&it.extra);
            hay.push(Utf32String::from(s.as_str()));
            starts.push(st);
        }
        let mut p = Picker {
            columns,
            items,
            source,
            hay,
            starts,
            query: query.to_owned(),
            shown: Vec::new(),
            selected: 0,
            top: 0,
            matcher: Matcher::new(Config::DEFAULT),
            enter: "insert",
            alternate: None,
        };
        p.filter();
        p
    }

    /// The words of the hint line for Enter (`open`, `insert`) and for Ctrl-Y / Alt-Enter (`path`); without the
    /// latter, Ctrl-Y does nothing.
    ///
    /// ```
    /// # use vc_picker::{Fuzzy, Picker};
    /// let p = Picker::new(&[], vec![], &Fuzzy, "").hints("open", Some("path"));
    /// ```
    pub fn hints(mut self, enter: &'static str, alternate: Option<&'static str>) -> Self {
        self.enter = enter;
        self.alternate = alternate;
        self
    }

    /// The rows the query passes, best first, as indices into the items.
    pub fn rows(&self) -> Vec<usize> {
        self.shown.iter().map(|s| s.item).collect()
    }

    /// The selected row, as an index into the items; `None` when no row passes the query.
    pub fn selected(&self) -> Option<usize> {
        self.current().map(|s| s.item)
    }

    /// The query as typed so far.
    pub fn query(&self) -> &str {
        &self.query
    }

    /// The items, in the order they came.
    pub fn items(&self) -> &[Item] {
        &self.items
    }

    /// The rows the query passes: the source's first, then nucleo's by score.
    fn filter(&mut self) {
        self.selected = 0;
        self.top = 0;
        let q = self.query.trim();
        if q.is_empty() {
            self.shown = (0..self.items.len())
                .map(|item| Shown {
                    item,
                    hl: Vec::new(),
                })
                .collect();
            return;
        }
        let pat = Pattern::parse(q, CaseMatching::Ignore, Normalization::Smart);
        let mut own = self.source.find(q);
        let mut seen = vec![false; self.items.len()];
        own.retain(|&i| i < seen.len() && !std::mem::replace(&mut seen[i], true));
        let mut fuzzy: Vec<(u32, usize)> = (0..self.items.len())
            .filter(|&i| !seen[i])
            .filter_map(|i| {
                let mut total = 0;
                for atom in &pat.atoms {
                    let s = u32::from(atom.score(self.hay[i].slice(..), &mut self.matcher)?);
                    // letters strewn over a long row are no match: the bonuses for word starts and letters next
                    // to each other must make up for the gaps (16 is nucleo's score of one letter)
                    if !atom.negative && s < 16 * atom.needle_text().len() as u32 {
                        return None;
                    }
                    total += s;
                }
                Some((total, i))
            })
            .collect();
        fuzzy.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
        let rows: Vec<usize> = own
            .into_iter()
            .chain(fuzzy.into_iter().map(|f| f.1))
            .collect();
        self.shown = rows
            .into_iter()
            .map(|item| {
                // every word that matches is lit, also on a row the source found by a rule of its own
                let mut hl = Vec::new();
                for atom in &pat.atoms {
                    let mut one = Vec::new();
                    if !atom.negative
                        && atom
                            .indices(self.hay[item].slice(..), &mut self.matcher, &mut one)
                            .is_some()
                    {
                        hl.extend(one);
                    }
                }
                hl.sort_unstable();
                hl.dedup();
                Shown { item, hl }
            })
            .collect();
    }

    /// The highlighted char positions of one cell of a shown row.
    pub(crate) fn cell_hl(&self, s: &Shown, col: usize) -> Vec<usize> {
        let Some(&start) = self.starts[s.item].get(col) else {
            return Vec::new();
        };
        let len = self.items[s.item].cells[col].chars().count() as u32;
        s.hl.iter()
            .filter(|&&i| i >= start && i < start + len)
            .map(|&i| (i - start) as usize)
            .collect()
    }

    pub(crate) fn current(&self) -> Option<&Shown> {
        self.shown.get(self.selected)
    }

    fn step(&mut self, by: isize) {
        if self.shown.is_empty() {
            return;
        }
        let last = self.shown.len() - 1;
        self.selected = self.selected.saturating_add_signed(by).min(last);
    }

    /// A key, with `list` rows on screen: the outcome once there is one.
    pub(crate) fn key(&mut self, k: KeyEvent, list: usize) -> Option<Outcome> {
        if k.kind == KeyEventKind::Release {
            return None;
        }
        if let Some(end) = ending(k) {
            return match end {
                None => Some(Outcome::Cancelled),
                Some(Action::Alternate) if self.alternate.is_none() => None,
                Some(a) => self.current().map(|s| Outcome::Picked(s.item, a)),
            };
        }
        let page = isize::try_from(list.max(1)).unwrap_or(isize::MAX);
        if let Some(by) = motion(k, page) {
            self.step(by);
        } else if let Some(e) = editing(k) {
            self.edit(e);
        }
        // the selected row stays on screen
        if self.selected < self.top {
            self.top = self.selected;
        } else if list > 0 && self.selected >= self.top + list {
            self.top = self.selected + 1 - list;
        }
        None
    }

    /// Changes the query and filters again when it changed.
    fn edit(&mut self, e: QueryEdit) {
        let q = &mut self.query;
        let changed = match e {
            QueryEdit::Push(c) => {
                q.push(c);
                true
            }
            QueryEdit::Pop => q.pop().is_some(),
            QueryEdit::Clear => !std::mem::take(q).is_empty(),
            QueryEdit::Word => {
                let before = q.len();
                let t = q.trim_end().len();
                q.truncate(t);
                let cut = q.rfind(' ').map_or(0, |i| i + 1);
                q.truncate(cut);
                q.len() != before
            }
        };
        if changed {
            self.filter();
        }
    }
}

/// A key that ends the picker: `Some(None)` cancels, `Some(Some(action))` takes the selected row.
fn ending(k: KeyEvent) -> Option<Option<Action>> {
    let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
    let alt = k.modifiers.contains(KeyModifiers::ALT);
    match k.code {
        KeyCode::Esc => Some(None),
        KeyCode::Char('c' | 'g' | 'q') if ctrl => Some(None),
        KeyCode::Enter if alt => Some(Some(Action::Alternate)),
        KeyCode::Char('y') if ctrl => Some(Some(Action::Alternate)),
        KeyCode::Enter => Some(Some(Action::Accept)),
        KeyCode::Char('m' | 'j') if ctrl => Some(Some(Action::Accept)),
        _ => None,
    }
}

/// A key that moves the selection, by how many rows; `page` rows for a page.
fn motion(k: KeyEvent, page: isize) -> Option<isize> {
    let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
    match k.code {
        KeyCode::Up | KeyCode::BackTab => Some(-1),
        KeyCode::Down | KeyCode::Tab => Some(1),
        KeyCode::PageUp => Some(-page),
        KeyCode::PageDown => Some(page),
        KeyCode::Home => Some(isize::MIN),
        KeyCode::End => Some(isize::MAX),
        KeyCode::Char(c) if ctrl => match c {
            'p' | 'k' => Some(-1),
            'n' => Some(1),
            'b' => Some(-page),
            'f' => Some(page),
            _ => None,
        },
        _ => None,
    }
}

/// What a key does to the query.
enum QueryEdit {
    /// A character typed.
    Push(char),
    /// Backspace, Ctrl-H: the last character goes.
    Pop,
    /// Ctrl-U: all of it goes.
    Clear,
    /// Ctrl-W: the last word goes.
    Word,
}

fn editing(k: KeyEvent) -> Option<QueryEdit> {
    let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
    let alt = k.modifiers.contains(KeyModifiers::ALT);
    match k.code {
        KeyCode::Backspace => Some(QueryEdit::Pop),
        KeyCode::Char('h') if ctrl => Some(QueryEdit::Pop),
        KeyCode::Char('u') if ctrl => Some(QueryEdit::Clear),
        KeyCode::Char('w') if ctrl => Some(QueryEdit::Word),
        KeyCode::Char(c) if !ctrl && !alt => Some(QueryEdit::Push(c)),
        _ => None,
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) const COLS: [Column; 3] = [
        Column {
            head: "no",
            width: Width::Fit { max: 8 },
            keep: 3,
        },
        Column {
            head: "title",
            width: Width::Fill {
                min: 10,
                weight: 3,
                left: false,
            },
            keep: 9,
        },
        Column {
            head: "path",
            width: Width::Fill {
                min: 10,
                weight: 2,
                left: true,
            },
            keep: 8,
        },
    ];

    pub(crate) fn item(cells: &[&str]) -> Item {
        Item {
            cells: cells.iter().map(|s| (*s).to_owned()).collect(),
            ..Item::default()
        }
    }

    pub(crate) fn items() -> Vec<Item> {
        vec![
            item(&["DS13560", "Datasheet STM32G0B1", "ST/G0/ds13560.pdf"]),
            item(&[
                "ES0548",
                "STM32G0B1 device errata",
                "ST/G0/es0548-errata.pdf",
            ]),
            item(&["RM0444", "STM32G0x1 reference manual", "ST/G0/rm0444.pdf"]),
            item(&["", "", "TI/notes.pdf"]),
        ]
    }

    /// A source that finds the rows whose number starts with the query.
    struct ByNo(Vec<&'static str>);
    impl Source for ByNo {
        fn find(&self, q: &str) -> Vec<usize> {
            let q = q.to_lowercase();
            (0..self.0.len())
                .filter(|&i| !self.0[i].is_empty() && self.0[i].to_lowercase().starts_with(&q))
                .collect()
        }
    }

    fn key(c: KeyCode) -> KeyEvent {
        KeyEvent::new(c, KeyModifiers::NONE)
    }
    fn ctrl(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)
    }
    fn typed(p: &mut Picker, s: &str) {
        for c in s.chars() {
            assert_eq!(p.key(key(KeyCode::Char(c)), 5), None);
        }
    }

    #[test]
    fn the_sources_rows_come_first_then_the_fuzzy_ones() {
        let src = ByNo(vec!["DS13560", "ES0548", "RM0444", ""]);
        let mut p = Picker::new(&COLS, items(), &src, "");
        assert_eq!(p.rows(), [0, 1, 2, 3], "no query: every row as it came");
        typed(&mut p, "rm");
        assert_eq!(p.rows()[0], 2, "RM0444 by the source's rule, first");
        p.key(ctrl('u'), 5);
        typed(&mut p, "errata");
        assert_eq!(p.rows(), [1], "fuzzy alone");
        p.key(ctrl('u'), 5);
        typed(&mut p, "dtsht");
        assert_eq!(p.rows(), [0], "letters of words");
        p.key(ctrl('u'), 5);
        typed(&mut p, "errata");
        let s = p.shown[0].clone();
        assert_eq!(
            p.cell_hl(&s, 1),
            (17..23).collect::<Vec<_>>(),
            "the letters that matched are lit, in the cell they are in"
        );
        p.key(ctrl('w'), 5);
        assert_eq!(p.query(), "");
        typed(&mut p, "g0 pdf");
        assert_eq!(p.rows(), [0, 1, 2], "every word must match");
        typed(&mut p, " !errata");
        assert_eq!(p.rows(), [0, 2], "fzf's syntax: !word leaves out");
        assert_eq!(p.key(key(KeyCode::Backspace), 5), None);
        assert_eq!(p.query(), "g0 pdf !errat");
    }

    #[test]
    fn fzf_syntax_exact_start_and_end() {
        let mut p = Picker::new(&COLS, items(), &Fuzzy, "'device");
        assert_eq!(p.rows(), [1], "'exact");
        p.key(ctrl('u'), 5);
        typed(&mut p, "^RM");
        assert_eq!(p.rows(), [2], "^start");
        p.key(ctrl('u'), 5);
        typed(&mut p, "notes.pdf$");
        assert_eq!(p.rows(), [3], "end$");
    }

    #[test]
    fn letters_strewn_over_a_row_are_no_match() {
        let it = vec![item(&[
            "",
            "Design of small hand launched solar powered UAV oettershagen2017.pdf",
            "",
        ])];
        assert!(
            Picker::new(&COLS, it.clone(), &Fuzzy, "errata")
                .rows()
                .is_empty()
        );
        assert_eq!(Picker::new(&COLS, it, &Fuzzy, "solpow").rows().len(), 1);
    }

    #[test]
    fn keys_move_pick_and_cancel() {
        let mut p = Picker::new(&COLS, items(), &Fuzzy, "").hints("open", Some("path"));
        assert_eq!(p.key(key(KeyCode::Down), 2), None);
        assert_eq!(p.key(ctrl('n'), 2), None);
        assert_eq!(
            (p.selected, p.top),
            (2, 1),
            "the selected row stays on screen"
        );
        p.key(key(KeyCode::End), 2);
        assert_eq!(p.selected(), Some(3));
        p.key(key(KeyCode::Home), 2);
        assert_eq!(p.selected(), Some(0));
        p.key(key(KeyCode::PageDown), 2);
        p.key(ctrl('p'), 2);
        p.key(key(KeyCode::Down), 2);
        assert_eq!(p.selected(), Some(2));
        p.key(key(KeyCode::Up), 2);
        assert_eq!(
            p.key(key(KeyCode::Enter), 2),
            Some(Outcome::Picked(1, Action::Accept))
        );
        assert_eq!(
            p.key(ctrl('y'), 2),
            Some(Outcome::Picked(1, Action::Alternate))
        );
        let alt_enter = KeyEvent::new(KeyCode::Enter, KeyModifiers::ALT);
        assert_eq!(
            p.key(alt_enter, 2),
            Some(Outcome::Picked(1, Action::Alternate))
        );
        assert_eq!(p.key(key(KeyCode::Esc), 2), Some(Outcome::Cancelled));
        assert_eq!(p.key(ctrl('c'), 2), Some(Outcome::Cancelled));
        assert_eq!(p.key(ctrl('g'), 2), Some(Outcome::Cancelled));
        typed(&mut p, "nothing like it");
        assert!(p.rows().is_empty());
        assert_eq!(p.selected(), None);
        assert_eq!(
            p.key(key(KeyCode::Enter), 2),
            None,
            "Enter on no row does nothing"
        );
    }

    #[test]
    fn without_an_alternate_ctrl_y_does_nothing() {
        let mut p = Picker::new(&COLS, items(), &Fuzzy, "");
        assert_eq!(p.key(ctrl('y'), 2), None);
        let alt_enter = KeyEvent::new(KeyCode::Enter, KeyModifiers::ALT);
        assert_eq!(p.key(alt_enter, 2), None);
        assert_eq!(
            p.key(key(KeyCode::Enter), 2),
            Some(Outcome::Picked(0, Action::Accept))
        );
    }

    #[test]
    fn the_query_given_is_the_start() {
        let p = Picker::new(&COLS, items(), &Fuzzy, "notes");
        assert_eq!(p.rows(), [3]);
        assert_eq!(p.query(), "notes");
    }

    #[test]
    fn a_source_out_of_range_or_twice_is_ignored() {
        struct Bad;
        impl Source for Bad {
            fn find(&self, _: &str) -> Vec<usize> {
                vec![9, 1, 1]
            }
        }
        let p = Picker::new(&COLS, items(), &Bad, "errata");
        assert_eq!(p.rows(), [1]);
    }
}
