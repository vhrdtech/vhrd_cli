//! What every CLI prints the same way (UI-1): colour that turns off without a terminal, the error line with the
//! tool's name in front, exit codes as a documented contract, and outside strings escaped before they reach the
//! terminal. The rules are in the CLI UX guide, `docs/ux-guide.md` sections 6 and 7.
//!
//! A `main` that uses all of it:
//!
//! ```no_run
//! use clap::Parser;
//! use vc_ui::{ColorFlag, Exit};
//!
//! #[derive(Parser)]
//! struct Cli {
//!     #[command(flatten)]
//!     color: ColorFlag,
//! }
//!
//! fn run(_cli: Cli) -> Result<(), Box<dyn std::error::Error>> {
//!     println!("{} {}", vc_ui::id("P1"), vc_ui::good("ok"));
//!     Ok(())
//! }
//!
//! fn main() -> std::process::ExitCode {
//!     vc_ui::restore_sigpipe();
//!     let cli = Cli::parse();
//!     cli.color.apply();
//!     vc_ui::finish("tool", run(cli), |_| None).into()
//! }
//! ```
//!
//! The crate is light on purpose: anstyle for the styles, clap for the `--color` flag, unicode-width for column
//! widths. Progress bars and tables live in `vc_steps`.

mod color;
mod error;
mod exit;
mod signal;
mod untrusted;
mod width;

pub use color::{
    BUSY, ColorChoice, ColorFlag, DATE, DIM, ERR, HEADING, ID, NEXT, OK, Paint, Stream, WARN,
    color_choice, date, dim, err, good, id, next_hint, paint, set_color, status, status_style,
    warn,
};
pub use error::{ExitError, Warning, error_line, finish, is_broken_pipe};
pub use exit::Exit;
pub use signal::restore_sigpipe;
pub use untrusted::{Untrusted, escape};
pub use width::{strip_ansi, visible_width};
