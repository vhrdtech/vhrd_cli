//! Colour from one set of styles, off when the stream is not a terminal, with `NO_COLOR` set and not empty, or
//! with `--color never` (guide section 6).

use anstyle::{AnsiColor, Style};
use std::ffi::OsStr;
use std::io::IsTerminal;
use std::sync::atomic::{AtomicU8, Ordering};

/// `--color auto|always|never`, as clap and cargo have it.
///
/// ```
/// use vc_ui::ColorChoice;
///
/// // a pipe: no colour, unless asked for
/// assert!(!ColorChoice::Auto.resolve(false, None));
/// assert!(ColorChoice::Always.resolve(false, None));
/// // NO_COLOR set and not empty turns auto off at a terminal too
/// assert!(!ColorChoice::Auto.resolve(true, Some("1".as_ref())));
/// assert!(ColorChoice::Auto.resolve(true, Some("".as_ref())));
/// ```
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, clap::ValueEnum)]
pub enum ColorChoice {
    /// Colour when the output is a terminal and NO_COLOR is not set
    #[default]
    Auto,
    /// Colour even into a pipe or a file
    Always,
    /// No colour
    Never,
}

impl ColorChoice {
    /// Whether colour is on for a stream that is a terminal or not, with `no_color` the value of `NO_COLOR`.
    pub fn resolve(self, is_terminal: bool, no_color: Option<&OsStr>) -> bool {
        match self {
            ColorChoice::Always => true,
            ColorChoice::Never => false,
            ColorChoice::Auto => is_terminal && no_color.is_none_or(OsStr::is_empty),
        }
    }

    fn to_u8(self) -> u8 {
        match self {
            ColorChoice::Auto => 0,
            ColorChoice::Always => 1,
            ColorChoice::Never => 2,
        }
    }

    fn from_u8(n: u8) -> ColorChoice {
        match n {
            1 => ColorChoice::Always,
            2 => ColorChoice::Never,
            _ => ColorChoice::Auto,
        }
    }
}

/// The `--color` flag, flattened into a CLI's root parser and global to every subcommand. `main` calls
/// [`ColorFlag::apply`] right after parsing, before the first line is printed.
///
/// ```
/// use clap::Parser;
/// use vc_ui::{ColorChoice, ColorFlag};
///
/// #[derive(Parser)]
/// struct Cli {
///     #[command(flatten)]
///     color: ColorFlag,
/// }
///
/// let cli = Cli::parse_from(["tool", "--color", "never"]);
/// assert_eq!(cli.color.color, ColorChoice::Never);
/// assert_eq!(Cli::parse_from(["tool"]).color.color, ColorChoice::Auto);
/// ```
#[derive(Clone, Copy, Debug, Default, clap::Args)]
pub struct ColorFlag {
    /// Colour: auto (when the output is a terminal and NO_COLOR is not set), always or never
    #[arg(long, value_enum, value_name = "WHEN", default_value_t, global = true)]
    pub color: ColorChoice,
}

impl ColorFlag {
    /// Decides colour for the whole run: [`set_color`] with the flag's value.
    pub fn apply(&self) {
        set_color(self.color);
    }
}

static CHOICE: AtomicU8 = AtomicU8::new(0);
/// Per stream: 0 not worked out yet, 1 off, 2 on. [`set_color`] clears them.
static RESOLVED: [AtomicU8; 2] = [AtomicU8::new(0), AtomicU8::new(0)];

/// Sets the colour choice for the whole run. Without a call it is [`ColorChoice::Auto`].
pub fn set_color(choice: ColorChoice) {
    CHOICE.store(choice.to_u8(), Ordering::Relaxed);
    for r in &RESOLVED {
        r.store(0, Ordering::Relaxed);
    }
}

/// The colour choice of this run, as [`set_color`] left it.
pub fn color_choice() -> ColorChoice {
    ColorChoice::from_u8(CHOICE.load(Ordering::Relaxed))
}

/// The output a piece of text is painted for: colour is decided per stream, since `tool | less` keeps stderr
/// at the terminal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stream {
    Stdout,
    Stderr,
}

impl Stream {
    fn is_terminal(self) -> bool {
        match self {
            Stream::Stdout => std::io::stdout().is_terminal(),
            Stream::Stderr => std::io::stderr().is_terminal(),
        }
    }
}

/// Successes (`ok`, `done`, `ready`): green.
pub const OK: Style = AnsiColor::Green.on_default().bold();
/// Something to look at but not an error (`behind main`, `skipped`): yellow.
pub const WARN: Style = AnsiColor::Yellow.on_default().bold();
/// Working or waiting: yellow, quieter than [`WARN`].
pub const BUSY: Style = AnsiColor::Yellow.on_default();
/// Errors and failed states: red, and only those.
pub const ERR: Style = AnsiColor::Red.on_default().bold();
/// Ids, slugs and names: one colour everywhere.
pub const ID: Style = AnsiColor::Cyan.on_default();
/// Dates and times.
pub const DATE: Style = Style::new().dimmed();
/// An aside.
pub const DIM: Style = Style::new().dimmed();
/// The word `next:` of a hint.
pub const NEXT: Style = AnsiColor::Magenta.on_default().bold();
/// A heading in a report: bold, no colour.
pub const HEADING: Style = Style::new().bold();

/// Paints text in a style when colour is on, and leaves it plain otherwise. [`Paint::stdout`] and
/// [`Paint::stderr`] follow the run's colour choice; [`Paint::ON`] and [`Paint::OFF`] are fixed, for tests and
/// for output that decides colour itself.
///
/// ```
/// use vc_ui::{Paint, strip_ansi};
///
/// let line = |p: Paint| format!("{} {}", p.id("P1"), p.status("failed"));
/// assert_eq!(line(Paint::OFF), "P1 failed");
/// assert_ne!(line(Paint::ON), "P1 failed");
/// assert_eq!(strip_ansi(&line(Paint::ON)), line(Paint::OFF));
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Paint {
    on: bool,
}

impl Paint {
    /// Always colour.
    pub const ON: Paint = Paint { on: true };
    /// Never colour.
    pub const OFF: Paint = Paint { on: false };

    /// Colour on or off.
    pub const fn new(on: bool) -> Paint {
        Paint { on }
    }

    /// For text that goes to `stream`, following [`set_color`], the stream's terminal and `NO_COLOR`.
    pub fn for_stream(stream: Stream) -> Paint {
        let slot = &RESOLVED[stream as usize];
        let on = match slot.load(Ordering::Relaxed) {
            1 => false,
            2 => true,
            _ => {
                let no_color = std::env::var_os("NO_COLOR");
                let on = color_choice().resolve(stream.is_terminal(), no_color.as_deref());
                slot.store(if on { 2 } else { 1 }, Ordering::Relaxed);
                on
            }
        };
        Paint { on }
    }

    /// For text that goes to stdout.
    pub fn stdout() -> Paint {
        Paint::for_stream(Stream::Stdout)
    }

    /// For text that goes to stderr: diagnostics, progress, the error line.
    pub fn stderr() -> Paint {
        Paint::for_stream(Stream::Stderr)
    }

    /// Whether this paints at all.
    pub const fn is_on(self) -> bool {
        self.on
    }

    /// `text` in `style`, or `text` as it is with colour off. Escape a string from outside first
    /// ([`crate::escape`]), then paint it.
    pub fn paint(self, style: Style, text: &str) -> String {
        if self.on && !text.is_empty() {
            format!("{style}{text}{style:#}")
        } else {
            text.to_string()
        }
    }

    /// Green, for success.
    pub fn good(self, text: &str) -> String {
        self.paint(OK, text)
    }

    /// Yellow, for something to look at that is not an error.
    pub fn warn(self, text: &str) -> String {
        self.paint(WARN, text)
    }

    /// Red, for an error.
    pub fn err(self, text: &str) -> String {
        self.paint(ERR, text)
    }

    /// Dim, for an aside.
    pub fn dim(self, text: &str) -> String {
        self.paint(DIM, text)
    }

    /// An id, slug or name.
    pub fn id(self, text: &str) -> String {
        self.paint(ID, text)
    }

    /// A date or time.
    pub fn date(self, text: &str) -> String {
        self.paint(DATE, text)
    }

    /// A `next:` hint line: the word `next:` highlighted, the rest as it is. A line without it stays as it is.
    pub fn next_hint(self, line: &str) -> String {
        match line.strip_prefix("next:") {
            Some(rest) => format!("{}{rest}", self.paint(NEXT, "next:")),
            None => line.to_string(),
        }
    }

    /// A status word in its colour ([`status_style`]); a word with no colour of its own stays as it is.
    pub fn status(self, word: &str) -> String {
        match status_style(word) {
            Some(style) => self.paint(style, word),
            None => word.to_string(),
        }
    }
}

/// The colour of a status word: `ok`, `done`, `ready` and the like green; `working`, `waiting`, `pending`
/// yellow; `failed`, `error`, `blocked`, `missing` red; `None` for a word with no convention. Case, a trailing
/// `!` and anything after the first space, `:` or `(` are ignored.
///
/// ```
/// assert_eq!(vc_ui::status_style("Failed: 2 files"), Some(vc_ui::ERR));
/// assert_eq!(vc_ui::status_style("waiting"), Some(vc_ui::BUSY));
/// assert_eq!(vc_ui::status_style("maybe"), None);
/// ```
pub fn status_style(word: &str) -> Option<Style> {
    let w = word.trim().to_lowercase();
    let w = w
        .split([' ', ':', '('])
        .next()
        .unwrap_or("")
        .trim_end_matches('!');
    match w {
        "ok" | "done" | "ready" | "landed" | "active" | "synced" | "clean" | "idle" | "yes"
        | "up" | "passed" => Some(OK),
        "working" | "waiting" | "paused" | "pending" | "behind" | "ahead" | "running"
        | "stopped" | "held" | "skipped" => Some(BUSY),
        "failed" | "error" | "diverged" | "dirty" | "blocked" | "full" | "stuck" | "conflict"
        | "missing" | "down" => Some(ERR),
        _ => None,
    }
}

/// `text` in `style` on stdout ([`Paint::stdout`]).
pub fn paint(style: Style, text: &str) -> String {
    Paint::stdout().paint(style, text)
}

/// [`Paint::good`] on stdout.
pub fn good(text: &str) -> String {
    Paint::stdout().good(text)
}

/// [`Paint::warn`] on stdout.
pub fn warn(text: &str) -> String {
    Paint::stdout().warn(text)
}

/// [`Paint::err`] on stdout.
pub fn err(text: &str) -> String {
    Paint::stdout().err(text)
}

/// [`Paint::dim`] on stdout.
pub fn dim(text: &str) -> String {
    Paint::stdout().dim(text)
}

/// [`Paint::id`] on stdout.
pub fn id(text: &str) -> String {
    Paint::stdout().id(text)
}

/// [`Paint::date`] on stdout.
pub fn date(text: &str) -> String {
    Paint::stdout().date(text)
}

/// [`Paint::next_hint`] on stdout.
pub fn next_hint(line: &str) -> String {
    Paint::stdout().next_hint(line)
}

/// [`Paint::status`] on stdout.
pub fn status(word: &str) -> String {
    Paint::stdout().status(word)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::strip_ansi;

    #[test]
    fn auto_is_off_in_a_pipe_and_with_no_color() {
        let auto = ColorChoice::Auto;
        assert!(auto.resolve(true, None));
        assert!(!auto.resolve(false, None));
        assert!(!auto.resolve(true, Some(OsStr::new("1"))));
        // NO_COLOR is honoured only when not empty (no-color.org)
        assert!(auto.resolve(true, Some(OsStr::new(""))));
    }

    #[test]
    fn always_and_never_ignore_the_terminal_and_no_color() {
        for tty in [false, true] {
            for nc in [None, Some(OsStr::new("1"))] {
                assert!(ColorChoice::Always.resolve(tty, nc));
                assert!(!ColorChoice::Never.resolve(tty, nc));
            }
        }
    }

    #[test]
    fn choice_round_trips_through_the_atomic() {
        for c in [ColorChoice::Auto, ColorChoice::Always, ColorChoice::Never] {
            assert_eq!(ColorChoice::from_u8(c.to_u8()), c);
        }
    }

    /// Guide section 12: the plain output has no escape codes, and the coloured output minus its escapes is the
    /// plain output.
    #[test]
    fn coloured_minus_escapes_is_plain() {
        let line = |p: Paint| {
            [
                p.good("ok"),
                p.warn("behind"),
                p.err("failed"),
                p.dim("(aside)"),
                p.id("P1#3"),
                p.date("3 Nov 2026"),
                p.status("waiting"),
                p.status("unknown"),
                p.next_hint("next: tool run"),
            ]
            .join(" ")
        };
        let plain = line(Paint::OFF);
        assert!(!plain.contains('\x1b'), "{plain:?}");
        let coloured = line(Paint::ON);
        assert!(coloured.contains('\x1b'));
        assert_eq!(strip_ansi(&coloured), plain);
    }

    #[test]
    fn empty_text_gets_no_codes() {
        assert_eq!(Paint::ON.good(""), "");
    }

    #[test]
    fn next_hint_paints_only_the_word() {
        let s = Paint::ON.next_hint("next: tool land");
        assert!(s.ends_with(" tool land"));
        assert!(s.starts_with('\x1b'));
        assert_eq!(Paint::ON.next_hint("no hint here"), "no hint here");
    }

    #[test]
    fn status_words() {
        assert_eq!(status_style("OK"), Some(OK));
        assert_eq!(status_style("done!"), Some(OK));
        assert_eq!(status_style("blocked (by P1)"), Some(ERR));
        assert_eq!(status_style("skipped"), Some(BUSY));
        assert_eq!(status_style(""), None);
    }
}
