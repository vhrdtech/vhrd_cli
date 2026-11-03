//! The error line (guide section 7): one line on stderr that starts with the tool's name and gives the whole
//! cause chain, and the end of `main` that prints it and picks the exit code.

use crate::color::{ERR, Paint, WARN};
use crate::exit::Exit;
use crate::untrusted::escape;
use std::error::Error;
use std::fmt;
use std::io::Write as _;

/// An error that ends the run with a given [`Exit`] code, made with [`Exit::error`]. It may carry the error
/// that caused it.
///
/// ```
/// use vc_ui::{Exit, Paint, error_line};
///
/// let io = std::io::Error::other("permission denied");
/// let e = Exit::CONFIG.error("can't read /etc/tool.toml").with_source(io);
/// assert_eq!(error_line("tool", &e, Paint::OFF), "tool: can't read /etc/tool.toml: permission denied");
/// ```
#[derive(Debug)]
pub struct ExitError {
    exit: Exit,
    message: String,
    source: Option<Box<dyn Error + Send + Sync>>,
}

impl ExitError {
    /// An error that exits with `exit` and says `message`.
    pub fn new(exit: Exit, message: impl Into<String>) -> ExitError {
        ExitError {
            exit,
            message: message.into(),
            source: None,
        }
    }

    /// The same error, caused by `source`: the error line names it after the message.
    pub fn with_source(mut self, source: impl Into<Box<dyn Error + Send + Sync>>) -> ExitError {
        self.source = Some(source.into());
        self
    }

    /// The exit code the run ends with.
    pub fn exit(&self) -> Exit {
        self.exit
    }
}

impl fmt::Display for ExitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl Error for ExitError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.source.as_deref().map(|e| e as &(dyn Error + 'static))
    }
}

/// An error that is something to do first rather than a failure (commits to land, uncommitted files): the error
/// line shows it yellow instead of red. The exit code is still 1, or what [`Exit::of`] finds.
///
/// ```
/// let w = vc_ui::Warning("2 commits to land first (tool land)".into());
/// assert_eq!(vc_ui::error_line("tool", &w, vc_ui::Paint::OFF), "tool: 2 commits to land first (tool land)");
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Warning(pub String);

impl fmt::Display for Warning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for Warning {}

/// The error line for `err`: `tool: message: cause: cause`, the whole cause chain on one line. Each message is
/// escaped ([`escape`]), since error texts often carry a device's or another process's words, and a newline in
/// one stays on the line. A cause that its parent already printed at its end (an error that formats its source
/// into its own message) is not said twice. Red with colour on, yellow for a [`Warning`]; the tool's name stays
/// plain.
///
/// ```
/// use vc_ui::{Paint, error_line};
///
/// let e = std::io::Error::other("no task P1#99 (tool ls P1 lists its tasks)");
/// assert_eq!(error_line("tool", &e, Paint::OFF), "tool: no task P1#99 (tool ls P1 lists its tasks)");
/// ```
pub fn error_line(tool: &str, err: &(dyn Error + 'static), paint: Paint) -> String {
    let mut parts: Vec<String> = Vec::new();
    let mut link = Some(err);
    while let Some(e) = link {
        let text = escape(&e.to_string()).into_owned();
        let said = parts.last().is_some_and(|p| p.ends_with(&text));
        if !text.is_empty() && !said {
            parts.push(text);
        }
        link = e.source();
    }
    let style = if err.is::<Warning>() { WARN } else { ERR };
    format!(
        "{}: {}",
        escape(tool),
        paint.paint(style, &parts.join(": "))
    )
}

/// Whether `err` is, somewhere in its cause chain, a write to a closed pipe (`tool | head`): not an error worth a
/// line. With [`crate::restore_sigpipe`] on Unix the run ends quietly before it gets here; this covers the other
/// platforms and code that ignores SIGPIPE on purpose.
///
/// ```
/// let pipe = std::io::Error::from(std::io::ErrorKind::BrokenPipe);
/// assert!(vc_ui::is_broken_pipe(&pipe));
/// assert!(!vc_ui::is_broken_pipe(&std::fmt::Error));
/// ```
pub fn is_broken_pipe(err: &(dyn Error + 'static)) -> bool {
    let mut link = Some(err);
    while let Some(e) = link {
        if e.downcast_ref::<std::io::Error>()
            .is_some_and(|io| io.kind() == std::io::ErrorKind::BrokenPipe)
        {
            return true;
        }
        link = e.source();
    }
    false
}

/// The end of `main`: on success the exit code the run returned (`()` is [`Exit::SUCCESS`]); on an error its
/// [`error_line`] on stderr, coloured as [`Paint::stderr`] decides, and the code [`Exit::of`] finds with
/// `classify`. A closed pipe ([`is_broken_pipe`]) ends quietly with [`Exit::SUCCESS`].
///
/// Works with any error that turns into `Box<dyn Error>`: a typed error, `Box<dyn Error>` itself, or
/// `anyhow::Error` (its context chain comes along).
///
/// ```no_run
/// fn run() -> Result<(), std::io::Error> {
///     Err(std::io::Error::other("no device found (plug it in, or pass --port)"))
/// }
///
/// fn main() -> vc_ui::Exit {
///     vc_ui::restore_sigpipe();
///     vc_ui::finish("tool", run(), |_| None)
/// }
/// ```
pub fn finish<T, E>(
    tool: &str,
    result: Result<T, E>,
    classify: impl Fn(&(dyn Error + 'static)) -> Option<Exit>,
) -> Exit
where
    T: Into<Exit>,
    E: Into<Box<dyn Error>>,
{
    match result {
        Ok(v) => v.into(),
        Err(e) => {
            let e: Box<dyn Error> = e.into();
            if is_broken_pipe(e.as_ref()) {
                return Exit::SUCCESS;
            }
            let line = error_line(tool, e.as_ref(), Paint::stderr());
            // nothing more to do when stderr is gone too
            let _ = writeln!(std::io::stderr().lock(), "{line}");
            Exit::of(e.as_ref(), classify)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::strip_ansi;

    #[test]
    fn whole_chain_on_one_line() {
        let e = anyhow::anyhow!("permission denied")
            .context("reading tool.toml")
            .context("loading the settings");
        assert_eq!(
            error_line("tool", e.as_ref(), Paint::OFF),
            "tool: loading the settings: reading tool.toml: permission denied"
        );
    }

    #[test]
    fn a_cause_already_in_its_parent_is_said_once() {
        #[derive(Debug)]
        struct Wraps(std::io::Error);
        impl fmt::Display for Wraps {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "opening the port: {}", self.0)
            }
        }
        impl Error for Wraps {
            fn source(&self) -> Option<&(dyn Error + 'static)> {
                Some(&self.0)
            }
        }
        let e = Wraps(std::io::Error::other("busy"));
        assert_eq!(
            error_line("tool", &e, Paint::OFF),
            "tool: opening the port: busy"
        );
    }

    #[test]
    fn outside_text_in_an_error_is_escaped_and_stays_on_one_line() {
        let e =
            anyhow::anyhow!("device said: \x1b[2Jgone\nsecond line").context("reading the name");
        let line = error_line("tool", e.as_ref(), Paint::OFF);
        assert_eq!(
            line,
            "tool: reading the name: device said: \\u{1b}[2Jgone\\nsecond line"
        );
        // with colour, the only escapes are the tool's own
        let coloured = error_line("tool", e.as_ref(), Paint::ON);
        assert_eq!(strip_ansi(&coloured), line);
    }

    #[test]
    fn warning_is_yellow_and_errors_red() {
        let w = error_line("tool", &Warning("land first".into()), Paint::ON);
        assert!(w.contains(&WARN.render().to_string()));
        let e = error_line("tool", &std::fmt::Error, Paint::ON);
        assert!(e.contains(&ERR.render().to_string()));
        assert!(e.starts_with("tool: "));
    }

    #[test]
    fn broken_pipe_is_found_under_context() {
        let io = std::io::Error::from(std::io::ErrorKind::BrokenPipe);
        let e = anyhow::Error::new(io).context("writing the table");
        assert!(is_broken_pipe(e.as_ref()));
        let boxed: Box<dyn Error> = e.into();
        assert!(is_broken_pipe(boxed.as_ref()));
        assert!(!is_broken_pipe(anyhow::anyhow!("other").as_ref()));
    }

    #[test]
    fn finish_picks_the_exit_code() {
        const NO_DEVICE: Exit = Exit::category(3, "no_device");
        let classify = |e: &(dyn Error + 'static)| {
            e.downcast_ref::<std::io::Error>()
                .is_some_and(|io| io.kind() == std::io::ErrorKind::NotFound)
                .then_some(NO_DEVICE)
        };
        assert_eq!(
            finish("tool", Ok::<(), std::io::Error>(()), classify),
            Exit::SUCCESS
        );
        assert_eq!(
            finish(
                "tool",
                Ok::<Exit, std::io::Error>(Exit::TEMP_FAIL),
                classify
            ),
            Exit::TEMP_FAIL
        );
        let not_found = anyhow::Error::new(std::io::Error::from(std::io::ErrorKind::NotFound))
            .context("opening");
        assert_eq!(finish("tool", Err::<(), _>(not_found), classify), NO_DEVICE);
        let config = Exit::CONFIG.error("bad setting");
        assert_eq!(finish("tool", Err::<(), _>(config), classify), Exit::CONFIG);
        let pipe = std::io::Error::from(std::io::ErrorKind::BrokenPipe);
        assert_eq!(finish("tool", Err::<(), _>(pipe), classify), Exit::SUCCESS);
    }
}
