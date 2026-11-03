//! Strings from outside escaped before they reach the terminal (guide section 6): a device name, a label, an
//! error text from another process or a page. Left as they are, they could carry C0 or C1 control characters or
//! Unicode bidi overrides that move the cursor, change colours, rewrite the terminal title or make a line read
//! out of order.
//!
//! A character that needs escaping is shown the way `{:?}` shows it in a Rust string literal (`\u{1b}`,
//! `\u{80}`, `\u{202e}`), except `\n`, `\r` and `\t`, which get their two-character form, so one outside string
//! can never wrap a printed line, overwrite the line above or split into two. Everything else, ASCII or not,
//! prints as it is. JSON output skips this: JSON's own string escaping covers control characters.
//!
//! The tool's own colour codes must keep working: escape the outside piece on its own, then paint around the
//! escaped text. Never escape a line that already carries colour, or the codes themselves (which start with the
//! same ESC) get escaped too.

use std::borrow::Cow;
use std::fmt;
use std::fmt::Write as _;

/// A string from a device, the network or another process, for printing with `{}`: it shows as [`escape`]
/// makes it.
///
/// ```
/// use vc_ui::{Paint, Untrusted};
///
/// let product = "evil\x1b]0;title\x07";
/// assert_eq!(format!("device {}", Untrusted(product)), "device evil\\u{1b}]0;title\\u{7}");
/// // escape first, then colour around it
/// let line = Paint::ON.id(&Untrusted(product).to_string());
/// assert!(line.starts_with('\x1b') && line.contains("\\u{1b}"));
/// ```
#[derive(Clone, Copy, Debug)]
pub struct Untrusted<'a>(pub &'a str);

impl fmt::Display for Untrusted<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&escape(self.0))
    }
}

/// `s`, or a copy with every control character and bidi override replaced by a visible escape. Borrows `s`
/// unchanged, without allocating, when nothing needs escaping.
///
/// ```
/// assert_eq!(vc_ui::escape("a\nb\u{202e}c"), "a\\nb\\u{202e}c");
/// assert_eq!(vc_ui::escape("café · 日本"), "café · 日本");
/// ```
pub fn escape(s: &str) -> Cow<'_, str> {
    if !s.chars().any(needs_escape) {
        return Cow::Borrowed(s);
    }
    let mut out = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        match c {
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if needs_escape(c) => {
                let _ = write!(out, "\\u{{{:x}}}", c as u32);
            }
            c => out.push(c),
        }
    }
    Cow::Owned(out)
}

/// C0 controls (ESC among them) and DEL, C1 controls, and the bidi embedding, override and isolate characters.
pub(crate) fn needs_escape(c: char) -> bool {
    matches!(c,
        '\u{00}'..='\u{1f}'
        | '\u{7f}'
        | '\u{80}'..='\u{9f}'
        | '\u{202a}'..='\u{202e}'
        | '\u{2066}'..='\u{2069}'
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_text_is_borrowed() {
        assert!(matches!(escape("hello, device 123"), Cow::Borrowed(_)));
        assert!(matches!(escape("b200 · café · 日本語"), Cow::Borrowed(_)));
    }

    #[test]
    fn escapes_esc_and_keeps_the_printable_tail() {
        assert_eq!(escape("\x1b[31mred\x1b[0m"), "\\u{1b}[31mred\\u{1b}[0m");
    }

    #[test]
    fn newline_cr_and_tab_get_short_forms() {
        assert_eq!(escape("a\nb\rc\td"), "a\\nb\\rc\\td");
    }

    #[test]
    fn escapes_c1_del_and_bidi() {
        assert_eq!(escape("\u{80}\u{9f}\x7f"), "\\u{80}\\u{9f}\\u{7f}");
        assert_eq!(escape("\u{202e}evil\u{2069}"), "\\u{202e}evil\\u{2069}");
        assert_eq!(escape("\u{202a}\u{2066}"), "\\u{202a}\\u{2066}");
    }

    #[test]
    fn escapes_an_osc_title_sequence() {
        assert_eq!(escape("\x1b]0;evil\x07"), "\\u{1b}]0;evil\\u{7}");
    }

    #[test]
    fn display_matches_escape() {
        assert_eq!(Untrusted("\x1b[31m").to_string(), escape("\x1b[31m"));
    }

    /// `{:?}` on a string already escapes everything this module does, so a value printed with `{:?}` needs no
    /// [`Untrusted`] of its own.
    #[test]
    fn debug_format_escapes_the_same_characters() {
        let s = "evil\x1b]0;x\x07\u{202e}bidi\u{80}\x7f";
        let debug = format!("{s:?}");
        for c in s.chars().filter(|c| needs_escape(*c)) {
            assert!(!debug.contains(c), "{c:?} not escaped by {{:?}}: {debug}");
        }
    }
}
