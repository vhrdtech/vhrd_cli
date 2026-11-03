//! Exit codes as a documented contract (guide section 7): 0 success, 1 a failure with no more specific category,
//! 2 bad arguments (clap's usage errors), a tool's own categories from 3 up, and 75 and 78 with their BSD
//! sysexits meaning.

use crate::error::ExitError;
use std::error::Error;

/// How a run ended, as a number the process exits with and a stable name that JSON errors carry
/// (`"exit": "no_device"`). The numbers are a public contract: a tool documents its categories in one table,
/// never renumbers one, and gives a new one the next free number.
///
/// ```
/// use vc_ui::Exit;
///
/// // a tool's own categories, in one place
/// pub const NO_DEVICE: Exit = Exit::category(3, "no_device");
/// pub const TIMEOUT: Exit = Exit::category(4, "timeout");
///
/// assert_eq!((NO_DEVICE.code(), NO_DEVICE.name()), (3, "no_device"));
/// assert_eq!(Exit::CONFIG.code(), 78);
/// let code: std::process::ExitCode = TIMEOUT.into();
/// # let _ = code;
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Exit {
    code: u8,
    name: &'static str,
}

impl Exit {
    /// 0: the run did what was asked.
    pub const SUCCESS: Exit = Exit {
        code: 0,
        name: "success",
    };
    /// 1: a failure with no more specific category.
    pub const FAILURE: Exit = Exit {
        code: 1,
        name: "failure",
    };
    /// 2: wrong or missing arguments; clap exits with it on a usage error.
    pub const BAD_ARGS: Exit = Exit {
        code: 2,
        name: "bad_args",
    };
    /// 75 (`EX_TEMPFAIL`): a temporary failure, worth trying again later (a server draining for a deploy, a
    /// lock held by another run).
    pub const TEMP_FAIL: Exit = Exit {
        code: 75,
        name: "temp_fail",
    };
    /// 78 (`EX_CONFIG`): can't work until someone acts (a bad setting, a database newer than the binary, a
    /// missing secret). A service unit does not restart on it.
    pub const CONFIG: Exit = Exit {
        code: 78,
        name: "config",
    };

    /// The lowest and highest number a tool's own category may take: from 3 up to below the sysexits range
    /// (64 to 78), far from the shell's 126 and up.
    pub const CATEGORIES: std::ops::RangeInclusive<u8> = 3..=63;

    /// A tool's own category: `code` in [`Exit::CATEGORIES`], `name` in `snake_case` (lowercase ASCII letters,
    /// digits and `_`). Checked when the constant is built, so a wrong one fails the build:
    ///
    /// ```compile_fail
    /// const BAD: vc_ui::Exit = vc_ui::Exit::category(78, "config_again");
    /// ```
    pub const fn category(code: u8, name: &'static str) -> Exit {
        assert!(
            code >= *Self::CATEGORIES.start() && code <= *Self::CATEGORIES.end(),
            "a tool's exit category is a number from 3 to 63"
        );
        let b = name.as_bytes();
        assert!(!b.is_empty(), "an exit category needs a name");
        let mut i = 0;
        while i < b.len() {
            assert!(
                b[i].is_ascii_lowercase() || b[i].is_ascii_digit() || b[i] == b'_',
                "an exit category's name is snake_case: lowercase letters, digits and _"
            );
            i += 1;
        }
        Exit { code, name }
    }

    /// The number the process exits with.
    pub const fn code(self) -> u8 {
        self.code
    }

    /// The stable name, as JSON errors carry it.
    pub const fn name(self) -> &'static str {
        self.name
    }

    /// Whether this is [`Exit::SUCCESS`].
    pub const fn is_success(self) -> bool {
        self.code == 0
    }

    /// An error that ends the run with this exit code. The message is the error line's text.
    ///
    /// ```
    /// use vc_ui::Exit;
    ///
    /// let e = Exit::CONFIG.error("the database is newer than this binary (update the tool)");
    /// assert_eq!(Exit::of(&e, |_| None), Exit::CONFIG);
    /// ```
    pub fn error(self, message: impl Into<String>) -> ExitError {
        ExitError::new(self, message)
    }

    /// The exit code for `err`: the first link of its cause chain that is an [`ExitError`] or that `classify`
    /// knows decides it; [`Exit::FAILURE`] when none does. `classify` maps a tool's own error types to its
    /// categories, usually by `downcast_ref`.
    ///
    /// ```
    /// use vc_ui::Exit;
    ///
    /// const TIMEOUT: Exit = Exit::category(5, "timeout");
    /// let io = std::io::Error::from(std::io::ErrorKind::TimedOut);
    /// let classify = |e: &(dyn std::error::Error + 'static)| {
    ///     let io = e.downcast_ref::<std::io::Error>()?;
    ///     (io.kind() == std::io::ErrorKind::TimedOut).then_some(TIMEOUT)
    /// };
    /// assert_eq!(Exit::of(&io, classify), TIMEOUT);
    /// assert_eq!(Exit::of(&std::fmt::Error, classify), Exit::FAILURE);
    /// ```
    pub fn of(
        err: &(dyn Error + 'static),
        classify: impl Fn(&(dyn Error + 'static)) -> Option<Exit>,
    ) -> Exit {
        let mut link = Some(err);
        while let Some(e) = link {
            if let Some(x) = e.downcast_ref::<ExitError>() {
                return x.exit();
            }
            if let Some(x) = classify(e) {
                return x;
            }
            link = e.source();
        }
        Exit::FAILURE
    }
}

impl From<()> for Exit {
    fn from((): ()) -> Exit {
        Exit::SUCCESS
    }
}

impl From<Exit> for std::process::ExitCode {
    fn from(exit: Exit) -> std::process::ExitCode {
        std::process::ExitCode::from(exit.code)
    }
}

impl std::process::Termination for Exit {
    fn report(self) -> std::process::ExitCode {
        self.into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The numbers and names are what scripts branch on: this test is the contract, change it only with a
    /// CHANGELOG entry that says the break.
    #[test]
    fn codes_and_names_are_stable() {
        let all = [
            (Exit::SUCCESS, 0, "success"),
            (Exit::FAILURE, 1, "failure"),
            (Exit::BAD_ARGS, 2, "bad_args"),
            (Exit::TEMP_FAIL, 75, "temp_fail"),
            (Exit::CONFIG, 78, "config"),
        ];
        for (exit, code, name) in all {
            assert_eq!((exit.code(), exit.name()), (code, name));
        }
    }

    #[test]
    fn categories_take_3_to_63() {
        assert_eq!(Exit::category(3, "no_device").code(), 3);
        assert_eq!(Exit::category(63, "last_one2").code(), 63);
        for bad in [0, 1, 2, 64, 75, 78, 126, 255] {
            assert!(
                std::panic::catch_unwind(|| Exit::category(bad, "x")).is_err(),
                "{bad}"
            );
        }
    }

    #[test]
    fn category_names_are_snake_case() {
        for bad in ["", "NoDevice", "no-device", "no device"] {
            assert!(
                std::panic::catch_unwind(|| Exit::category(3, bad)).is_err(),
                "{bad:?}"
            );
        }
    }

    #[test]
    fn exit_error_deep_in_the_chain_decides() {
        #[derive(Debug)]
        struct Outer(ExitError);
        impl std::fmt::Display for Outer {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("loading the settings")
            }
        }
        impl Error for Outer {
            fn source(&self) -> Option<&(dyn Error + 'static)> {
                Some(&self.0)
            }
        }
        let e = Outer(Exit::CONFIG.error("no secret"));
        assert_eq!(Exit::of(&e, |_| None), Exit::CONFIG);
        // the outermost link a classifier knows wins over a deeper one
        let outer = Exit::category(9, "settings");
        assert_eq!(Exit::of(&e, |l| l.is::<Outer>().then_some(outer)), outer);
    }

    #[test]
    fn unit_is_success() {
        assert_eq!(Exit::from(()), Exit::SUCCESS);
        assert!(Exit::SUCCESS.is_success() && !Exit::FAILURE.is_success());
    }
}
