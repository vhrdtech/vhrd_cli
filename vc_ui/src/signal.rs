//! SIGPIPE back to its default, so `tool | head` ends quietly.

/// Puts SIGPIPE back to its default action, so a write to a closed pipe (`tool | head`) ends the run quietly,
/// as it does for C tools, instead of an "error: Broken pipe" or a panic from `println!`. The Rust runtime
/// ignores SIGPIPE before `main`; call this first thing in `main`. Does nothing outside Unix.
///
/// Code that must survive a reader going away (a server writing to sockets) should not call it, or should set
/// SIGPIPE back to ignored on its own.
///
/// ```no_run
/// // first thing in main
/// vc_ui::restore_sigpipe();
/// for i in 0.. {
///     println!("{i}"); // `| head` ends it here, quietly
/// }
/// ```
pub fn restore_sigpipe() {
    #[cfg(unix)]
    // SAFETY: `signal` with SIG_DFL installs no handler of ours; it only changes the process's disposition of
    // SIGPIPE, which the Rust runtime set to ignored before main.
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_DFL);
    }
}
