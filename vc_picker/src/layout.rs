//! Where things go: the width of each column, cells cut to fit, and how high the picker is.

use crate::state::{Column, Item, Width};
use unicode_width::UnicodeWidthChar;

/// Display width of a text.
pub(crate) fn width(s: &str) -> usize {
    s.chars().map(|c| c.width().unwrap_or(0)).sum()
}

/// The width of each column in a terminal `total` wide (0: left out), two spaces between columns and two for the
/// pointer in front. Measured over all rows, so the columns stay put while the query changes.
pub(crate) fn layout(cols: &[Column], items: &[Item], total: usize) -> Vec<usize> {
    let natural: Vec<usize> = cols
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let w = items
                .iter()
                .map(|it| it.cells.get(i).map_or(0, |s| width(s)))
                .max()
                .unwrap_or(0)
                .max(width(c.head));
            match c.width {
                Width::Fit { max } => w.min(max),
                Width::Fill { .. } => w,
            }
        })
        .collect();
    let mut on: Vec<bool> = natural.iter().map(|&w| w > 0).collect();
    loop {
        let n = on.iter().filter(|&&o| o).count();
        let room = total.saturating_sub(2 + 2 * n.saturating_sub(1));
        let fit: usize = (0..cols.len())
            .filter(|&i| on[i] && matches!(cols[i].width, Width::Fit { .. }))
            .map(|i| natural[i])
            .sum();
        let min: usize = (0..cols.len())
            .filter(|&i| on[i])
            .map(|i| match cols[i].width {
                Width::Fill { min, .. } => min.min(natural[i]),
                Width::Fit { .. } => 0,
            })
            .sum();
        if fit + min <= room || n <= 1 {
            return share(cols, &natural, &on, room.saturating_sub(fit));
        }
        // the least needed column goes, the rightmost of them first
        let Some(drop) = (0..cols.len())
            .filter(|&i| on[i])
            .min_by_key(|&i| (cols[i].keep, std::cmp::Reverse(i)))
        else {
            return share(cols, &natural, &on, 0);
        };
        on[drop] = false;
    }
}

/// The fill columns' share of `room`: each its minimum, then the rest by weight, none wider than its widest cell.
fn share(cols: &[Column], natural: &[usize], on: &[bool], mut room: usize) -> Vec<usize> {
    let mut w: Vec<usize> = (0..cols.len())
        .map(|i| match cols[i].width {
            _ if !on[i] => 0,
            Width::Fit { .. } => natural[i],
            Width::Fill { min, .. } => min.min(natural[i]),
        })
        .collect();
    let mut open: Vec<usize> = (0..cols.len())
        .filter(|&i| on[i] && matches!(cols[i].width, Width::Fill { .. }))
        .collect();
    room = room.saturating_sub(open.iter().map(|&i| w[i]).sum());
    let weight = |i: usize| match cols[i].width {
        Width::Fill { weight, .. } => weight.max(1),
        Width::Fit { .. } => 1,
    };
    while !open.is_empty() && room > 0 {
        let total: usize = open.iter().map(|&i| weight(i)).sum();
        // a column that wants less than its share takes what it wants, the rest is shared again
        if let Some(pos) = open
            .iter()
            .position(|&i| natural[i] - w[i] <= room * weight(i) / total)
        {
            let i = open.remove(pos);
            room -= natural[i] - w[i];
            w[i] = natural[i];
            continue;
        }
        let mut given = 0;
        for &i in &open {
            let g = room * weight(i) / total;
            w[i] += g;
            given += g;
        }
        // what division left over goes to the first
        w[open[0]] += room - given;
        break;
    }
    w
}

/// A cell cut to `w` columns: at its end, or with `left` at its start, `…` where it was cut. Returns the chars
/// shown with their position in the cell (`None` for the `…`). Control characters are shown as spaces.
pub(crate) fn cut(s: &str, w: usize, left: bool) -> Vec<(char, Option<usize>)> {
    let chars: Vec<(char, Option<usize>)> = s
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .enumerate()
        .map(|(i, c)| (c, Some(i)))
        .collect();
    if width(s) <= w {
        return chars;
    }
    if w == 0 {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut used = 1;
    let it: Box<dyn Iterator<Item = &(char, Option<usize>)>> = if left {
        Box::new(chars.iter().rev())
    } else {
        Box::new(chars.iter())
    };
    for &(c, i) in it {
        let cw = c.width().unwrap_or(0);
        if used + cw > w {
            break;
        }
        used += cw;
        out.push((c, i));
    }
    out.push(('…', None));
    if left {
        out.reverse();
    }
    out
}

/// How many lines the picker takes in a terminal `rows` high: about three fifths of it, at most what the rows need.
pub(crate) fn height(rows: u16, items: usize) -> u16 {
    let rows = usize::from(rows.max(4));
    let cap = (rows * 3 / 5).max(10).min(rows - 1).max(4);
    u16::try_from((items + 3).clamp(4, cap)).unwrap_or(u16::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::tests::{COLS, items};

    #[test]
    fn columns_fit_the_terminal() {
        let it = items();
        assert_eq!(
            layout(&COLS, &it, 200),
            [7, 26, 23],
            "wide: every column as its widest cell"
        );
        for total in [40, 60, 80, 120] {
            let w = layout(&COLS, &it, total);
            let n = w.iter().filter(|&&w| w > 0).count();
            let used: usize = w.iter().sum::<usize>() + 2 + 2 * (n - 1);
            assert!(used <= total, "{total}: {w:?}");
            assert_eq!(
                w[0], 7,
                "{total}: a fit column keeps its width while there is room"
            );
        }
        assert_eq!(
            layout(&COLS, &it, 50),
            [7, 21, 16],
            "the title three parts of the rest, the path two"
        );
        assert_eq!(
            layout(&COLS, &it, 28),
            [0, 13, 11],
            "too narrow: the least needed one goes"
        );
        let mut it = it;
        for i in &mut it {
            i.cells[0].clear();
        }
        assert_eq!(
            layout(&COLS, &it, 200)[0],
            2,
            "a header wider than the cells widens its column"
        );
    }

    #[test]
    fn cells_are_cut_at_their_end_or_start() {
        let s = |v: Vec<(char, Option<usize>)>| v.iter().map(|x| x.0).collect::<String>();
        assert_eq!(s(cut("ST/G0/es0548-errata.pdf", 12, true)), "…-errata.pdf");
        assert_eq!(s(cut("Datasheet STM32G0B1", 10, false)), "Datasheet…");
        assert_eq!(s(cut("short", 10, false)), "short");
        assert_eq!(
            s(cut("a\tb", 10, false)),
            "a b",
            "a control character is a space"
        );
        let c = cut("abcdef", 4, true);
        assert_eq!(
            c,
            [('…', None), ('d', Some(3)), ('e', Some(4)), ('f', Some(5))],
            "positions say which char of the cell each one is, for the highlight"
        );
        assert_eq!(
            s(cut("日本語の資料", 7, false)),
            "日本語…",
            "wide chars count double"
        );
    }

    #[test]
    fn the_height_is_most_of_the_terminal_or_what_the_rows_need() {
        assert_eq!(height(50, 1000), 30);
        assert_eq!(height(50, 4), 7);
        assert_eq!(height(12, 1000), 10);
        assert_eq!(height(5, 1000), 4);
        assert_eq!(height(0, 0), 4);
    }
}
