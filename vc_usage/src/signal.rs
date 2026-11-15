//! Signals: the words a record counts, read from clap's own command tree and its parsed matches.

use clap::parser::ValueSource;
use clap::{Arg, ArgMatches, Command};

/// The signals of one invocation: the command path, then one per flag given on the command line or through its
/// environment variable.
///
/// `cmd` is the tree the matches came from, as `CommandFactory::command()` gives it (not built). A path is the
/// command's words, `spl tools needed`; a flag adds its long name, `spl check --fix`; a flag with a fixed set of
/// values adds the value too, `spl check --format=json`. Positional arguments and free-form values (paths, names)
/// are never recorded: what a person typed stays out of the file, and the number of signals stays that of the tree.
///
/// ```
/// use clap::{Arg, ArgAction, Command};
///
/// let cmd = Command::new("tool").subcommand(
///     Command::new("check")
///         .arg(Arg::new("fix").long("fix").action(ArgAction::SetTrue))
///         .arg(Arg::new("format").long("format").value_parser(["text", "json"]).default_value("text")),
/// );
/// let m = cmd.clone().get_matches_from(["tool", "check", "--format", "json"]);
/// assert_eq!(vc_usage::signals(&cmd, &m), ["tool check", "tool check --format=json"]);
/// ```
pub fn signals(cmd: &Command, m: &ArgMatches) -> Vec<String> {
    let mut out = Vec::new();
    let mut path = cmd.get_name().to_owned();
    let mut flags = Vec::new();
    let (mut cmd, mut m) = (cmd, m);
    loop {
        for arg in cmd.get_arguments().filter(|a| !a.is_positional()) {
            flags.extend(given(&path, arg, m));
        }
        let Some((name, sub_m)) = m.subcommand() else {
            break;
        };
        let Some(sub) = cmd.find_subcommand(name) else {
            break;
        };
        path = format!("{path} {}", sub.get_name());
        (cmd, m) = (sub, sub_m);
    }
    out.push(path);
    out.append(&mut flags);
    out
}

/// Every signal the tree can give: each command path, each flag, each value of a flag with a fixed set, in the
/// tree's order. The report lists them all, so a part never used shows with a count of 0.
///
/// ```
/// use clap::{Arg, ArgAction, Command};
///
/// let cmd = Command::new("tool")
///     .subcommand(Command::new("check").arg(Arg::new("fix").long("fix").action(ArgAction::SetTrue)));
/// assert_eq!(vc_usage::known(&cmd), ["tool", "tool check", "tool check --fix"]);
/// ```
pub fn known(cmd: &Command) -> Vec<String> {
    let mut out = Vec::new();
    walk(cmd, cmd.get_name(), &mut out);
    out
}

fn walk(cmd: &Command, path: &str, out: &mut Vec<String>) {
    out.push(path.to_owned());
    for arg in cmd.get_arguments().filter(|a| !a.is_positional()) {
        let flag = format!("{path} {}", label(arg));
        let values = fixed_values(arg);
        if values.is_empty() {
            out.push(flag);
        } else {
            out.extend(values.iter().map(|v| format!("{flag}={v}")));
        }
    }
    for sub in cmd.get_subcommands() {
        walk(sub, &format!("{path} {}", sub.get_name()), out);
    }
}

/// The signals of `arg` when it was given, on the command line or by its environment variable; a default is not
/// a use.
fn given(path: &str, arg: &Arg, m: &ArgMatches) -> Vec<String> {
    let id = arg.get_id().as_str();
    if !matches!(
        m.value_source(id),
        Some(ValueSource::CommandLine | ValueSource::EnvVariable)
    ) {
        return Vec::new();
    }
    let flag = format!("{path} {}", label(arg));
    let values = fixed_values(arg);
    if values.is_empty() {
        return vec![flag];
    }
    let possible = arg.get_possible_values();
    let mut out: Vec<String> = Vec::new();
    for raw in m.try_get_raw(id).ok().flatten().into_iter().flatten() {
        let raw = raw.to_string_lossy();
        if let Some(v) = possible
            .iter()
            .find(|p| p.matches(&raw, arg.is_ignore_case_set()))
        {
            let s = format!("{flag}={}", v.get_name());
            if !out.contains(&s) {
                out.push(s);
            }
        }
    }
    if out.is_empty() {
        out.push(flag);
    }
    out
}

/// The values worth a signal of their own: those of a flag that takes a value from a fixed set (a `ValueEnum`).
/// A switch's `true`/`false` is not one.
fn fixed_values(arg: &Arg) -> Vec<String> {
    if !arg.get_action().takes_values() {
        return Vec::new();
    }
    arg.get_possible_values()
        .iter()
        .map(|p| p.get_name().to_owned())
        .collect()
}

/// `--long`, else `-s`, else the argument's id.
fn label(arg: &Arg) -> String {
    match (arg.get_long(), arg.get_short()) {
        (Some(l), _) => format!("--{l}"),
        (None, Some(s)) => format!("-{s}"),
        (None, None) => arg.get_id().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::ArgAction;

    fn tool() -> Command {
        Command::new("tool")
            .arg(
                Arg::new("color")
                    .long("color")
                    .value_parser(["auto", "always", "never"])
                    .default_value("auto")
                    .global(true),
            )
            .arg(
                Arg::new("verbose")
                    .short('v')
                    .action(ArgAction::Count)
                    .global(true),
            )
            .subcommand(
                Command::new("check")
                    .arg(Arg::new("path"))
                    .arg(Arg::new("fix").long("fix").action(ArgAction::SetTrue))
                    .arg(Arg::new("only").long("only").action(ArgAction::Append))
                    .arg(
                        Arg::new("format")
                            .long("format")
                            .value_parser(["text", "json"])
                            .ignore_case(true)
                            .action(ArgAction::Append),
                    )
                    .arg(Arg::new("today").long("today").env("TOOL_TEST_TODAY")),
            )
            .subcommand(Command::new("tools").subcommand(Command::new("needed")))
    }

    fn run(args: &[&str]) -> Vec<String> {
        let cmd = tool();
        let m = cmd
            .clone()
            .try_get_matches_from(args)
            .expect("the test's arguments parse");
        signals(&cmd, &m)
    }

    #[test]
    fn path_and_flags() {
        assert_eq!(run(&["tool"]), ["tool"]);
        assert_eq!(run(&["tool", "tools", "needed"]), ["tool tools needed"]);
        assert_eq!(
            run(&["tool", "check", "src", "--fix", "--only", "secret-name"]),
            ["tool check", "tool check --fix", "tool check --only"]
        );
    }

    #[test]
    fn values_from_a_fixed_set_only_and_once() {
        assert_eq!(
            run(&[
                "tool",
                "check",
                "--format",
                "JSON",
                "--format=json",
                "--format",
                "text"
            ]),
            [
                "tool check",
                "tool check --format=json",
                "tool check --format=text"
            ]
        );
    }

    #[test]
    fn globals_count_once_where_they_are_declared() {
        assert_eq!(
            run(&["tool", "check", "--color", "never", "-vv"]),
            ["tool check", "tool --color=never", "tool -v"]
        );
        assert_eq!(run(&["tool", "-v", "check"]), ["tool check", "tool -v"]);
    }

    #[test]
    fn known_lists_the_whole_tree() {
        assert_eq!(
            known(&tool()),
            [
                "tool",
                "tool --color=auto",
                "tool --color=always",
                "tool --color=never",
                "tool -v",
                "tool check",
                "tool check --fix",
                "tool check --only",
                "tool check --format=text",
                "tool check --format=json",
                "tool check --today",
                "tool tools",
                "tool tools needed",
            ]
        );
    }
}
