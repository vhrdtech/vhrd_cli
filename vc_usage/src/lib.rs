//! Usage records for clap CLIs (USAGE-1): each invocation counted per subcommand and per flag, in a small local
//! file, and a report that lists every part of the command tree with its counts, the never-used ones included.
//! The rule and what is kept where are in the CLI UX guide, `docs/ux-guide.md` section 11.
//!
//! - The signals come from clap's own tree and parsed matches ([`signals`], [`known`]): the command path, each
//!   flag given, and the value of a flag with a fixed set of values. Positional arguments and free-form values
//!   are never recorded.
//! - Opt-in, off by default ([`Store::locate`]): nothing is recorded until `<TOOL>_USAGE_DIR` names a
//!   directory, or `<TOOL>_USAGE=on` turns it on at the default directory under the XDG state dir. An
//!   append-only log is folded into a counts file when it reaches [`LOG_BYTES_MAX`]. Nothing leaves the machine.
//! - Recording never fails a run and prints nothing: [`record`] drops any error.
//!
//! A `main` that records, and a `usage` command that shows the counts:
//!
//! ```no_run
//! use clap::{CommandFactory, FromArgMatches, Parser, Subcommand};
//!
//! #[derive(Parser)]
//! #[command(name = "tool")]
//! struct Cli {
//!     #[command(subcommand)]
//!     cmd: Cmd,
//! }
//!
//! #[derive(Subcommand)]
//! enum Cmd {
//!     /// Do the work
//!     Run,
//!     /// How often each command and flag was used
//!     Usage,
//! }
//!
//! fn main() {
//!     let matches = Cli::command().get_matches();
//!     vc_usage::record(&Cli::command(), &matches, None);
//!     let cli = Cli::from_arg_matches(&matches).unwrap_or_else(|e| e.exit());
//!     match cli.cmd {
//!         Cmd::Run => {}
//!         Cmd::Usage => {
//!             let env = |k: &str| std::env::var_os(k);
//!             let Some(dir) = vc_usage::Store::locate("tool", env) else { return };
//!             let counts = vc_usage::Store::new(dir).read().unwrap_or_default();
//!             let rows = vc_usage::rows(&vc_usage::known(&Cli::command()), &counts);
//!             print!("{}", vc_usage::text(&rows, vc_ui::Paint::stdout()));
//!         }
//!     }
//! }
//! ```

mod report;
mod signal;
mod store;

pub use report::{Row, rows, text};
pub use signal::{known, signals};
pub use store::{
    AGENT_ENV, Counts, Day, Entry, LOG_BYTES_MAX, Limits, SIGNAL_BYTES_MAX, SIGNALS_MAX,
    SOURCE_ENV, Source, Store,
};

use clap::{ArgMatches, Command};
use std::io::IsTerminal;

/// Records this run in the tool's store: the signals of `matches` against `cmd` (named after `cmd`'s name),
/// the source from the environment and stdin, on `today` (default: the system clock's day in UTC).
///
/// Opt-in, off by default ([`Store::locate`]): this is a no-op until the person enables recording.
/// Never fails and never prints: a store that cannot be found or written loses this record, nothing else.
pub fn record(cmd: &Command, matches: &ArgMatches, today: Option<Day>) {
    let env = |k: &str| std::env::var_os(k);
    let Some(dir) = Store::locate(cmd.get_name(), env) else {
        return;
    };
    let source = Source::detect(env, std::io::stdin().is_terminal());
    let day = today.unwrap_or_else(Day::today_utc);
    // the record is best effort by design: a failure must not reach the run
    Store::new(dir)
        .record(&signals(cmd, matches), source, &day)
        .ok();
}
