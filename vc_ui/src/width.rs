//! Widths measured on the visible text (guide section 6): escape codes count as nothing, a wide character or
//! emoji as two columns.

use unicode_width::UnicodeWidthStr;

/// `s` without ANSI escape sequences: CSI (`ESC [ ... final byte`, colours and cursor moves), OSC
/// (`ESC ] ... BEL` or `ESC ] ... ESC \`, titles and hyperlinks) and the two-byte `ESC x` forms. An escape cut
/// off at the end of `s` is dropped.
///
/// ```
/// assert_eq!(vc_ui::strip_ansi("\x1b[1;32mok\x1b[0m"), "ok");
/// assert_eq!(vc_ui::strip_ansi("\x1b]8;;https://example.com\x1b\\link\x1b]8;;\x1b\\"), "link");
/// ```
pub fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut it = s.chars().peekable();
    while let Some(c) = it.next() {
        if c != '\x1b' {
            out.push(c);
            continue;
        }
        match it.next() {
            // CSI: parameter and intermediate bytes, then one final byte in @..=~
            Some('[') => {
                for d in it.by_ref() {
                    if ('@'..='~').contains(&d) {
                        break;
                    }
                }
            }
            // OSC: up to BEL or the string terminator ESC \
            Some(']') => {
                while let Some(d) = it.next() {
                    if d == '\x07' {
                        break;
                    }
                    if d == '\x1b' && it.peek() == Some(&'\\') {
                        it.next();
                        break;
                    }
                }
            }
            // ESC and one more character (ESC 7, ESC c, ...), or ESC at the very end
            _ => {}
        }
    }
    out
}

/// The number of terminal columns `s` takes: escape sequences count as nothing, a wide character (CJK, most
/// emoji) as two. Use it to pad table columns, never `len()` or `chars().count()`.
///
/// ```
/// use vc_ui::{Paint, visible_width};
///
/// assert_eq!(visible_width(&Paint::ON.good("ok")), 2);
/// assert_eq!(visible_width("✅ done"), 7);
/// assert_eq!(visible_width("日本"), 4);
/// ```
pub fn visible_width(s: &str) -> usize {
    strip_ansi(s).width()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_text_is_untouched() {
        assert_eq!(strip_ansi("a [b] ] c"), "a [b] ] c");
    }

    #[test]
    fn strips_sgr_and_cursor_moves() {
        assert_eq!(
            strip_ansi("\x1b[2K\x1b[1Gdone \x1b[38;5;208mx\x1b[m"),
            "done x"
        );
    }

    #[test]
    fn strips_osc_with_either_terminator() {
        assert_eq!(strip_ansi("\x1b]0;title\x07rest"), "rest");
        assert_eq!(strip_ansi("\x1b]0;title\x1b\\rest"), "rest");
    }

    #[test]
    fn strips_two_byte_escapes_and_a_cut_off_tail() {
        assert_eq!(strip_ansi("a\x1b7b\x1b8c"), "abc");
        assert_eq!(strip_ansi("a\x1b"), "a");
        assert_eq!(strip_ansi("a\x1b[31"), "a");
        assert_eq!(strip_ansi("a\x1b]0;x"), "a");
    }

    #[test]
    fn wide_characters_take_two_columns() {
        assert_eq!(visible_width("🚧"), 2);
        assert_eq!(visible_width("\x1b[31m🐛 bug\x1b[0m"), 6);
        assert_eq!(visible_width("café"), 4);
    }
}
