//! What the picker looks like: the query line, the header, the rows and the bottom line, as styled lines.

use crate::layout::{cut, width};
use crate::state::{Picker, Width};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use unicode_width::UnicodeWidthChar;

/// The bottom line when no row passes the query.
const NO_ROW: &str = "no row matches: Backspace, Ctrl-W or Ctrl-U";

/// The styles, with colour or without (only bold, underline and reverse).
pub(crate) struct Theme {
    hl: Style,
    head: Style,
    dim: Style,
    sel: Style,
    prompt: Style,
}

impl Theme {
    pub(crate) fn new(color: bool) -> Self {
        if color {
            Self {
                hl: Style::new().fg(Color::Green).add_modifier(Modifier::BOLD),
                head: Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD),
                dim: Style::new().add_modifier(Modifier::DIM),
                sel: Style::new()
                    .bg(Color::Indexed(237))
                    .add_modifier(Modifier::BOLD),
                prompt: Style::new().fg(Color::Blue).add_modifier(Modifier::BOLD),
            }
        } else {
            Self {
                hl: Style::new().add_modifier(Modifier::UNDERLINED),
                head: Style::new().add_modifier(Modifier::BOLD),
                dim: Style::new(),
                sel: Style::new().add_modifier(Modifier::REVERSED),
                prompt: Style::new().add_modifier(Modifier::BOLD),
            }
        }
    }
}

impl Picker<'_> {
    /// The hint on the query line: what Enter, Ctrl-Y and Esc do.
    pub(crate) fn hint(&self) -> String {
        match self.alternate {
            Some(alt) => format!("  Enter {} · Ctrl-Y {alt} · Esc", self.enter),
            None => format!("  Enter {} · Esc", self.enter),
        }
    }

    /// The lines of the picker `area` big: the query, the header, the rows, the bottom line.
    pub(crate) fn lines(&self, area: Rect, widths: &[usize], t: &Theme) -> Vec<Line<'static>> {
        let w = usize::from(area.width);
        let list = usize::from(area.height).saturating_sub(3);
        let mut out = vec![self.query_line(w, t), self.header(widths, t)];
        for (n, s) in self.shown.iter().enumerate().skip(self.top).take(list) {
            out.push(self.row(s, n == self.selected, w, widths, t));
        }
        while out.len() < 2 + list {
            out.push(Line::raw(""));
        }
        let detail = self
            .current()
            .map_or(NO_ROW, |s| self.items[s.item].detail.as_str());
        let d: String = cut(detail, w, true).iter().map(|x| x.0).collect();
        out.push(Line::styled(d, t.dim));
        out
    }

    /// The query with the counts and, where there is room, the hint at the right.
    fn query_line(&self, w: usize, t: &Theme) -> Line<'static> {
        let count = format!("{}/{}", self.shown.len(), self.items.len());
        let hint = self.hint();
        let left = 2 + width(&self.query);
        let mut right = vec![Span::styled(count.clone(), t.dim)];
        if left + width(&count) + width(&hint) + 2 <= w {
            right.push(Span::styled(hint, t.dim));
        }
        let rw: usize = right.iter().map(|s| width(&s.content)).sum();
        let mut q = vec![Span::styled("> ", t.prompt), Span::raw(self.query.clone())];
        q.push(Span::raw(" ".repeat(w.saturating_sub(left + rw).max(1))));
        q.extend(right);
        Line::from(q)
    }

    fn header(&self, widths: &[usize], t: &Theme) -> Line<'static> {
        let mut head = vec![Span::raw("  ")];
        let mut first = true;
        for (i, c) in self.columns.iter().enumerate() {
            if widths[i] == 0 {
                continue;
            }
            if !first {
                head.push(Span::raw("  "));
            }
            first = false;
            let h: String = cut(c.head, widths[i], false).iter().map(|x| x.0).collect();
            head.push(Span::styled(format!("{h:<0$}", widths[i]), t.head));
        }
        Line::from(head)
    }

    fn row(
        &self,
        s: &crate::state::Shown,
        sel: bool,
        w: usize,
        widths: &[usize],
        t: &Theme,
    ) -> Line<'static> {
        let it = &self.items[s.item];
        let base = if sel {
            t.sel
        } else if it.dim {
            t.dim
        } else {
            Style::new()
        };
        let mut spans = vec![Span::styled(
            if sel { "▌ " } else { "  " },
            base.patch(t.prompt),
        )];
        let mut first = true;
        for (i, c) in self.columns.iter().enumerate() {
            if widths[i] == 0 {
                continue;
            }
            if !first {
                spans.push(Span::styled("  ", base));
            }
            first = false;
            let left = matches!(c.width, Width::Fill { left: true, .. });
            let hl = self.cell_hl(s, i);
            let shown = cut(it.cells.get(i).map_or("", |s| s), widths[i], left);
            let mut used = 0;
            for (ch, at) in shown {
                let st = if at.is_some_and(|a| hl.contains(&a)) {
                    base.patch(t.hl)
                } else {
                    base
                };
                used += ch.width().unwrap_or(0);
                spans.push(Span::styled(ch.to_string(), st));
            }
            spans.push(Span::styled(
                " ".repeat(widths[i].saturating_sub(used)),
                base,
            ));
        }
        if sel {
            let used: usize = spans.iter().map(|s| width(&s.content)).sum();
            spans.push(Span::styled(" ".repeat(w.saturating_sub(used)), base));
        }
        Line::from(spans)
    }

    /// Where the query's cursor is, in the picker.
    pub(crate) fn cursor(&self) -> (u16, u16) {
        (u16::try_from(2 + width(&self.query)).unwrap_or(u16::MAX), 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::layout;
    use crate::state::Fuzzy;
    use crate::state::tests::{COLS, items};

    fn text(p: &Picker, w: u16, h: u16) -> Vec<String> {
        let widths = layout(p.columns, &p.items, usize::from(w));
        p.lines(Rect::new(0, 0, w, h), &widths, &Theme::new(false))
            .iter()
            .map(|l| {
                l.spans
                    .iter()
                    .map(|s| s.content.as_ref())
                    .collect::<String>()
            })
            .collect()
    }

    #[test]
    fn lines_show_header_rows_and_the_bottom_line() {
        let mut it = items();
        it[0].detail = "/data/datasheets/ST/G0/ds13560.pdf".into();
        let p = Picker::new(&COLS, it, &Fuzzy, "").hints("open", Some("path"));
        let text = text(&p, 70, 6);
        assert_eq!(text.len(), 6);
        assert!(
            text[0].starts_with("> ") && text[0].contains("4/4"),
            "{text:?}"
        );
        assert!(
            text[0].ends_with("Enter open · Ctrl-Y path · Esc"),
            "{text:?}"
        );
        assert!(text[1].starts_with("  no "), "{text:?}");
        assert!(
            text[1].contains("title") && text[1].contains("path"),
            "{text:?}"
        );
        assert!(text[2].starts_with("▌ DS13560"), "{text:?}");
        assert!(text[3].starts_with("  ES0548"), "{text:?}");
        assert_eq!(text[5], "/data/datasheets/ST/G0/ds13560.pdf");
    }

    #[test]
    fn no_row_says_how_to_get_rows_back() {
        let p = Picker::new(&COLS, items(), &Fuzzy, "zzzz");
        let text = text(&p, 60, 6);
        assert!(text[0].contains("0/4"), "{text:?}");
        assert!(text[0].contains("Enter insert · Esc"), "{text:?}");
        assert_eq!(text[5], NO_ROW);
    }

    #[test]
    fn a_narrow_terminal_drops_the_hint_first() {
        let p = Picker::new(&COLS, items(), &Fuzzy, "");
        let text = text(&p, 24, 6);
        assert!(
            text[0].contains("4/4") && !text[0].contains("Enter"),
            "{text:?}"
        );
    }
}
