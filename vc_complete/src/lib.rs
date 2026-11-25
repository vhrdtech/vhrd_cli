//! Shell completion for clap CLIs: bash, zsh and fish complete live values with a help text each, never fail
//! loudly, and are registered by one line in the shell's config (the [rc] block).
//!
//! It sits on clap_complete's dynamic engine (`unstable-dynamic`): `COMPLETE=bash mycli` prints the registration,
//! then the shell calls the binary itself on every TAB, so completion always matches the installed version. The
//! shell adapters differ from clap_complete's own:
//! - **bash** quotes values with spaces and quotes, and handles bash's word breaks at `@`, `:` and `=` (the binary
//!   splits the raw line itself, see [`split_line`]). There is no `-o bashdefault`, so no candidates never turns
//!   into a list of file names.
//! - **zsh** offers candidates that replace the typed word instead of continuing it (a search by words), in a menu.
//! - **fish** takes one marked candidate ([`PICKED`]) in place of the token even when fish's own matching would drop it.
//!
//! An error or a panic in a completer gives no candidates and no output on stderr: a Tab press never breaks the
//! line being typed. Values that take long to list belong in a cache the CLI fills elsewhere.
//!
//! One call at the top of `main`:
//!
//! ```no_run
//! use clap::{Arg, Command};
//! use clap_complete::ArgValueCompleter;
//!
//! fn cli() -> Command {
//!     Command::new("mycli").arg(Arg::new("device").long("device").add(ArgValueCompleter::new(
//!         vc_complete::values(|| {
//!             Ok::<_, std::io::Error>(vec![("2100AB".to_string(), "Nucleo on the desk".to_string())])
//!         }),
//!     )))
//! }
//!
//! fn main() {
//!     vc_complete::complete(cli); // answers COMPLETE=... and exits; otherwise returns
//!     let _matches = cli().get_matches();
//! }
//! ```
//!
//! The shell's config then holds `source <(COMPLETE=bash mycli)` (zsh the same, fish `COMPLETE=fish mycli | source`),
//! which [`rc::Block`] writes and keeps current.

mod adapters;
mod line;
pub mod rc;

pub use line::{CurrentWord, comp_words, quote, split_line, word_after};

use clap::Command;
use clap_complete::env::{Elvish, Powershell, Shells};
use clap_complete::{CompleteEnv, CompletionCandidate};
use std::ffi::OsStr;
use std::sync::OnceLock;

/// The extra lines of a binary's registration for one shell (`"bash"`, `"zsh"`, `"fish"`), written after the
/// completion function. `NAME`, `BIN` and `COMPLETER` in them are replaced as in the registration itself, so a key
/// binding can call the completion function or the binary.
///
/// ```
/// fn extra(shell: &str) -> &'static str {
///     match shell {
///         "bash" => "alias BIN-help='COMPLETER --help'",
///         _ => "",
///     }
/// }
/// vc_complete::Completion::new(|| clap::Command::new("mycli")).extra(extra).complete();
/// ```
pub type Extra = fn(shell: &str) -> &'static str;

/// Answers a shell completion request (`COMPLETE=<shell>` in the environment) and exits; without one it returns
/// and `main` goes on. Call it first thing in `main`. [`Completion`] adds a pick widget or extra lines.
///
/// ```no_run
/// vc_complete::complete(|| clap::Command::new("mycli"));
/// ```
pub fn complete(factory: fn() -> Command) {
    Completion::new(factory).complete();
}

/// The completion of one CLI, with what its registration adds: a pick widget for a key binding, extra lines.
///
/// ```no_run
/// vc_complete::Completion::new(|| clap::Command::new("mycli"))
///     .pick_widget("pick --print", "open")
///     .complete();
/// ```
#[derive(Clone, Copy, Debug)]
pub struct Completion {
    factory: fn() -> Command,
    setup: Setup,
}

/// What the registration adds, kept for the one completion request a process answers.
#[derive(Clone, Copy, Debug, Default)]
struct Setup {
    extra: Option<Extra>,
    pick: Option<(&'static str, &'static str)>,
}

impl Completion {
    /// The completion of the CLI that `factory` builds.
    pub fn new(factory: fn() -> Command) -> Self {
        Self {
            factory,
            setup: Setup::default(),
        }
    }

    /// Extra lines in the registration, after the completion function.
    pub fn extra(mut self, extra: Extra) -> Self {
        self.setup.extra = Some(extra);
        self
    }

    /// A pick widget in the registration: a shell function that runs the CLI with the words `run` (a subcommand that
    /// opens a picker and prints the pick, exit status 0; anything else leaves the line alone) and puts the pick on
    /// the line. On an empty line the line becomes the CLI, the words `line` and the pick (`mycli open PICK`), else
    /// the pick goes in at the cursor, quoted for the shell. The function is `_vc_pick_<bin>` in bash and zsh (a zle
    /// widget there) and `__vc_pick_<bin>` in fish; a key is bound to it in the shell's config by
    /// [`rc::Block::pick_key`].
    ///
    /// # Panics
    ///
    /// When `run` or `line` holds anything but letters, digits, spaces and `-_.=`: they go into the shell
    /// function as words.
    pub fn pick_widget(mut self, run: &'static str, line: &'static str) -> Self {
        let plain = |s: &str| {
            s.chars()
                .all(|c| c.is_ascii_alphanumeric() || " -_.=".contains(c))
        };
        assert!(plain(run), "pick_widget: run {run:?} must be plain words");
        assert!(
            plain(line),
            "pick_widget: line {line:?} must be plain words"
        );
        self.setup.pick = Some((run, line));
        self
    }

    /// Answers a shell completion request and exits; without one it returns.
    pub fn complete(self) {
        SETUP.get_or_init(|| self.setup);
        CompleteEnv::with_factory(self.factory)
            .shells(Shells(&[
                &adapters::Bash,
                &Elvish,
                &adapters::Fish,
                &Powershell,
                &adapters::Zsh,
            ]))
            .complete();
    }
}

static SETUP: OnceLock<Setup> = OnceLock::new();

/// The extra lines of the registration for `shell`: the pick widget, then the caller's own.
fn extra(shell: &str) -> String {
    let setup = SETUP.get().copied().unwrap_or_default();
    let mut out = String::new();
    if setup.pick.is_some() {
        out.push_str(adapters::pick_widget(shell));
        out.push('\n');
    }
    if let Some(f) = setup.extra {
        out.push_str(f(shell));
    }
    out
}

/// The pick widget's words: what runs the picker, and what goes before the pick on an empty line.
fn pick_words() -> (&'static str, &'static str) {
    SETUP.get().and_then(|s| s.pick).unwrap_or_default()
}

/// A CLI's name as a shell function name takes it: letters, digits and `_`.
pub(crate) fn ident(bin: &str) -> String {
    bin.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect()
}

/// Whether Tab may open a picker in this completion: the shell asking is bash, zsh or fish, and the variable
/// [`tab_picker_var`] of `tool` is not `0`. A completer that opens one falls back to its plain candidates where
/// it cannot run (no terminal).
///
/// ```
/// assert!(!vc_complete::tab_picker("mycli"), "outside completion");
/// ```
pub fn tab_picker(tool: &str) -> bool {
    matches!(shell(), Some("bash" | "zsh" | "fish"))
        && std::env::var_os(tab_picker_var(tool)).is_none_or(|v| v != "0")
}

/// The variable that turns the picker on Tab off for `tool` when it is `0`: `TOOL_TAB_PICKER`.
///
/// ```
/// assert_eq!(vc_complete::tab_picker_var("my-cli"), "MY_CLI_TAB_PICKER");
/// ```
pub fn tab_picker_var(tool: &str) -> String {
    format!("{}_TAB_PICKER", ident(tool).to_ascii_uppercase())
}

/// The id of a candidate a picker chose: fish puts it in place of the token even when it does not match what was
/// typed (fish would drop it otherwise). Set it with [`CompletionCandidate::id`].
///
/// ```
/// let c = vc_complete::cand("ds13560-rev5.txt", "STM32G0B1 datasheet").id(Some(vc_complete::PICKED.into()));
/// assert_eq!(c.get_id().map(String::as_str), Some(vc_complete::PICKED));
/// ```
pub const PICKED: &str = "vc-picked";

/// The shell asking, while a completer runs: `"bash"`, `"zsh"` or `"fish"`, `None` for the others and outside
/// completion. clap_complete takes `COMPLETE` out of the environment before the completers run, so this is the
/// way to tell.
///
/// ```
/// assert_eq!(vc_complete::shell(), None);
/// ```
pub fn shell() -> Option<&'static str> {
    SHELL.get().copied()
}

static SHELL: OnceLock<&'static str> = OnceLock::new();

/// A candidate with its help text (none when the text is empty).
///
/// ```
/// let c = vc_complete::cand("2100AB", "Nucleo on the desk");
/// assert_eq!(c.get_value(), "2100AB");
/// assert_eq!(c.get_help().map(|h| h.to_string()).as_deref(), Some("Nucleo on the desk"));
/// assert!(vc_complete::cand("x", "").get_help().is_none());
/// ```
pub fn cand(value: impl Into<String>, help: impl Into<String>) -> CompletionCandidate {
    let help: String = help.into();
    let c = CompletionCandidate::new(value.into());
    if help.is_empty() {
        c
    } else {
        c.help(Some(help.into()))
    }
}

/// Whether `value` starts with what was typed, ignoring case.
///
/// ```
/// assert!(vc_complete::starts("Nucleo", "nuc"));
/// assert!(!vc_complete::starts("Nucleo", "ucl"));
/// ```
pub fn starts(value: &str, typed: &str) -> bool {
    value.to_lowercase().starts_with(&typed.to_lowercase())
}

/// The `(value, help)` items that start with the typed word, as candidates.
///
/// ```
/// let items = [("alpha", "first"), ("beta", "second")].map(|(v, h)| (v.to_string(), h.to_string()));
/// let got = vc_complete::filtered(items, "AL".as_ref());
/// assert_eq!(got.len(), 1);
/// assert_eq!(got[0].get_value(), "alpha");
/// ```
pub fn filtered(
    items: impl IntoIterator<Item = (String, String)>,
    typed: &OsStr,
) -> Vec<CompletionCandidate> {
    let typed = typed.to_string_lossy();
    items
        .into_iter()
        .filter(|(v, _)| starts(v, &typed))
        .map(|(v, h)| cand(v, h))
        .collect()
}

/// A live value completer for [`clap_complete::ArgValueCompleter`]: `list` gives `(value, help)` items, those that
/// start with the typed word become candidates. An error gives no candidates, never a message.
///
/// ```
/// use clap_complete::engine::ValueCompleter;
/// let devices = vc_complete::values(|| Ok::<_, std::io::Error>(vec![("2100AB".into(), "Nucleo".into())]));
/// assert_eq!(devices.complete("21".as_ref()).len(), 1);
/// let broken = vc_complete::values(|| Err::<Vec<(String, String)>, _>("no bus"));
/// assert!(broken.complete("".as_ref()).is_empty());
/// ```
pub fn values<F, I, E>(
    list: F,
) -> impl Fn(&OsStr) -> Vec<CompletionCandidate> + Send + Sync + 'static
where
    F: Fn() -> Result<I, E> + Send + Sync + 'static,
    I: IntoIterator<Item = (String, String)>,
{
    move |typed| match list() {
        Ok(items) => filtered(items, typed),
        Err(_) => Vec::new(),
    }
}
