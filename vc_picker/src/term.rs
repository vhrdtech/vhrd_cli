//! The picker on the terminal: `/dev/tty` in raw mode, room made below the prompt, the cursor put back at the end.

use crate::draw::Theme;
use crate::layout::{height, layout};
use crate::state::{Outcome, Picker};
use crossterm::event::Event;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::Rect;
use ratatui::{Terminal, TerminalOptions, Viewport};
use std::io::{Read, Write};
use std::time::{Duration, Instant};

/// How long the terminal may take to say where its cursor is.
const CURSOR_ANSWER_MAX: Duration = Duration::from_secs(2);

/// Bytes read while waiting for the cursor's position, before giving up on an answer.
const CURSOR_ANSWER_BYTES_MAX: usize = 256;

/// Why the picker could not run. The caller then does without it, such as by offering plain candidates.
#[derive(Debug)]
pub enum PickError {
    /// There is no terminal to draw on (`/dev/tty` cannot be opened).
    NoTerminal(std::io::Error),
    /// The terminal did not say where its cursor is.
    NoCursor,
    /// Reading keys from or drawing on the terminal failed.
    Io(std::io::Error),
}

impl std::fmt::Display for PickError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoTerminal(_) => {
                f.write_str("the picker draws on the terminal, and there is none (/dev/tty)")
            }
            Self::NoCursor => f.write_str("the terminal did not say where its cursor is"),
            Self::Io(_) => f.write_str("the picker lost its terminal"),
        }
    }
}

impl std::error::Error for PickError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::NoTerminal(e) | Self::Io(e) => Some(e),
            Self::NoCursor => None,
        }
    }
}

impl From<std::io::Error> for PickError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

/// Raw mode on the terminal and the cursor back on the prompt line, whatever happens.
struct Raw {
    tty: std::fs::File,
    /// The shell's prompt line, after the room was made.
    row: u16,
    col: u16,
}

impl Drop for Raw {
    fn drop(&mut self) {
        // nothing to report to: the caller gets the terminal back either way
        write!(
            self.tty,
            "\x1b[{};1H\x1b[J\x1b[{};{}H\x1b[?25h",
            u32::from(self.row) + 2,
            u32::from(self.row) + 1,
            u32::from(self.col) + 1
        )
        .and_then(|()| self.tty.flush())
        .ok();
        crossterm::terminal::disable_raw_mode().ok();
    }
}

/// The cursor's position on the terminal (column, row), asked of it directly: crossterm's own question goes
/// through stdout, which a completion function reads.
fn cursor_position(tty: &mut std::fs::File) -> Result<(u16, u16), PickError> {
    use rustix::event::{PollFd, PollFlags, Timespec, poll};
    tty.write_all(b"\x1b[6n")?;
    tty.flush()?;
    let mut got = Vec::new();
    let t0 = Instant::now();
    let tick = Timespec {
        tv_sec: 0,
        tv_nsec: 100_000_000,
    };
    while t0.elapsed() < CURSOR_ANSWER_MAX && got.len() < CURSOR_ANSWER_BYTES_MAX {
        let ready = {
            let mut fds = [PollFd::new(&*tty, PollFlags::IN)];
            poll(&mut fds, Some(&tick)).map_err(std::io::Error::from)?
        };
        if ready == 0 {
            continue;
        }
        let mut b = [0u8; 32];
        let n = tty.read(&mut b)?;
        got.extend_from_slice(&b[..n]);
        if let Some(end) = got.iter().position(|&c| c == b'R') {
            let s = String::from_utf8_lossy(&got[..end]);
            if let Some(at) = s.rfind("\x1b[")
                && let Some((r, c)) = s[at + 2..].split_once(';')
                && let (Ok(r), Ok(c)) = (r.parse::<u16>(), c.parse::<u16>())
            {
                return Ok((c.saturating_sub(1), r.saturating_sub(1)));
            }
        }
    }
    Err(PickError::NoCursor)
}

/// Runs the picker on the terminal (`/dev/tty`), drawn below the prompt line, until a row is picked or it is
/// cancelled; then the rows it drew are cleared and the cursor is back where it was. It works from inside a
/// completion function or a key binding, whose stdout the shell reads. `color` comes from [`color`].
///
/// ```no_run
/// use vc_picker::{Column, Fuzzy, Item, Outcome, Picker, Width};
/// const COLS: [Column; 1] = [Column { head: "name", width: Width::Fit { max: 30 }, keep: 1 }];
/// let rows = vec![Item { cells: vec!["alpha".into()], ..Item::default() }];
/// let mut p = Picker::new(&COLS, rows, &Fuzzy, "");
/// if let Ok(Outcome::Picked(row, _)) = vc_picker::run(&mut p, vc_picker::color()) {
///     println!("{}", p.items()[row].cells[0]);
/// }
/// ```
pub fn run(p: &mut Picker, color: bool) -> Result<Outcome, PickError> {
    let mut tty = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/tty")
        .map_err(PickError::NoTerminal)?;
    crossterm::terminal::enable_raw_mode()?;
    let mut raw = Raw {
        tty: tty.try_clone()?,
        row: 0,
        col: 0,
    };
    let (col, row) = cursor_position(&mut tty)?;
    let (cols, rows) = crossterm::terminal::size()?;
    let mut h = height(rows, p.items.len());
    // room below the prompt line: the terminal scrolls up as far as it takes
    write!(
        tty,
        "{}\x1b[{h}A\x1b[{}G",
        "\n".repeat(usize::from(h)),
        u32::from(col) + 1
    )?;
    tty.flush()?;
    raw.row = row.min(rows.saturating_sub(h + 1));
    raw.col = col;
    let area = |w: u16, h: u16, row: u16| Rect::new(0, row + 1, w, h);
    let mut term = Terminal::with_options(
        CrosstermBackend::new(tty),
        TerminalOptions {
            viewport: Viewport::Fixed(area(cols, h, raw.row)),
        },
    )?;
    let theme = Theme::new(color);
    let mut widths = layout(p.columns, &p.items, usize::from(cols));
    loop {
        let list = usize::from(h.saturating_sub(3));
        term.draw(|f| frame(f, p, &widths, &theme))?;
        match crossterm::event::read()? {
            Event::Key(k) => {
                if let Some(o) = p.key(k, list) {
                    return Ok(o);
                }
            }
            Event::Resize(c, r) => {
                h = height(r, p.items.len());
                raw.row = raw.row.min(r.saturating_sub(h + 1));
                widths = layout(p.columns, &p.items, usize::from(c));
                write!(raw.tty, "\x1b[{};1H\x1b[J", u32::from(raw.row) + 2)?;
                term.resize(area(c, h, raw.row))?;
            }
            _ => {}
        }
    }
}

/// One frame of the picker: its lines in the viewport, the cursor at the end of the query.
fn frame(f: &mut ratatui::Frame, p: &Picker, widths: &[usize], theme: &Theme) {
    let a = f.area();
    let lines = p.lines(a, widths, theme);
    for (i, l) in (0..a.height).zip(lines) {
        f.render_widget(l, Rect::new(a.x, a.y + i, a.width, 1));
    }
    let (x, y) = p.cursor();
    f.set_cursor_position((a.x + x.min(a.width.saturating_sub(1)), a.y + y));
}

/// Whether the picker draws in colour: the CLI's `--color` choice ([`vc_ui::color_choice`]) for a terminal, so
/// `auto` turns it off when `NO_COLOR` is set and not empty.
///
/// ```
/// let _on: bool = vc_picker::color();
/// ```
pub fn color() -> bool {
    vc_ui::color_choice().resolve(true, std::env::var_os("NO_COLOR").as_deref())
}
