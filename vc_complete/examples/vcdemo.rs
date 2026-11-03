//! A small CLI with live completion values, the way a real one uses vc_complete; the tests drive it in the shells.
//!
//! Try it in bash: `cargo build --example vcdemo`, put `target/debug/examples` on `PATH`, then
//! `source <(COMPLETE=bash vcdemo)` and `vcdemo open <TAB>`.

use clap::{Arg, ArgAction, Command};
use clap_complete::{ArgValueCompleter, CompletionCandidate};
use std::ffi::OsStr;

/// The documents `open` and `find` complete: names with spaces and quotes, each with a help text.
fn docs() -> Vec<(String, String)> {
    [
        ("alpha one", "the first document"),
        ("alpha two", "the second document"),
        ("beta", "a plain name"),
        ("it's", "a name with a quote"),
    ]
    .map(|(v, h)| (v.to_string(), h.to_string()))
    .to_vec()
}

/// Hosts, with `@` that bash breaks words at.
fn hosts() -> Result<Vec<(String, String)>, std::io::Error> {
    Ok(vec![
        ("user@build".into(), "the build machine".into()),
        ("user@core".into(), "the main machine".into()),
    ])
}

/// A search by words: every document that holds the typed word anywhere, so the candidates replace the word.
fn find(typed: &OsStr) -> Vec<CompletionCandidate> {
    let typed = typed.to_string_lossy().to_lowercase();
    docs()
        .into_iter()
        .filter(|(v, _)| v.to_lowercase().contains(&typed))
        .map(|(v, h)| vc_complete::cand(v, h))
        .collect()
}

/// What a picker would answer: one document, whatever was typed.
fn pick(_typed: &OsStr) -> Vec<CompletionCandidate> {
    vec![vc_complete::cand("alpha two", "picked").id(Some(vc_complete::PICKED.into()))]
}

/// The shell that asks, as the one candidate.
fn which_shell(_typed: &OsStr) -> Vec<CompletionCandidate> {
    vec![vc_complete::cand(
        vc_complete::shell().unwrap_or("none"),
        "the shell asking",
    )]
}

fn value(name: &'static str, completer: ArgValueCompleter) -> Arg {
    Arg::new(name).action(ArgAction::Append).add(completer)
}

fn cli() -> Command {
    Command::new("vcdemo")
        .about("Shows vc_complete's live values")
        .subcommand(Command::new("open").about("Open a document").arg(value(
            "doc",
            ArgValueCompleter::new(vc_complete::values(|| Ok::<_, std::io::Error>(docs()))),
        )))
        .subcommand(
            Command::new("find")
                .about("Find documents by a word")
                .arg(value("words", ArgValueCompleter::new(find))),
        )
        .subcommand(Command::new("host").about("Connect to a host").arg(value(
            "host",
            ArgValueCompleter::new(vc_complete::values(hosts)),
        )))
        .subcommand(
            Command::new("fail")
                .about("A value whose list fails")
                .arg(value(
                    "x",
                    ArgValueCompleter::new(vc_complete::values(|| {
                        Err::<Vec<(String, String)>, _>(std::io::Error::other("no bus"))
                    })),
                )),
        )
        .subcommand(
            Command::new("panic")
                .about("A value whose completer panics")
                .arg(value(
                    "x",
                    ArgValueCompleter::new(|_: &OsStr| -> Vec<CompletionCandidate> {
                        panic!("completer panicked")
                    }),
                )),
        )
        .subcommand(
            Command::new("pick")
                .about("A value a picker chose")
                .arg(value("doc", ArgValueCompleter::new(pick))),
        )
        .subcommand(
            Command::new("shell")
                .about("Completes the name of the shell asking")
                .arg(value("name", ArgValueCompleter::new(which_shell))),
        )
}

fn main() {
    vc_complete::complete(cli);
    let m = cli().get_matches();
    if let Some((sub, args)) = m.subcommand() {
        let ids: Vec<_> = args.ids().map(|id| id.as_str().to_string()).collect();
        let values: Vec<String> = ids
            .iter()
            .flat_map(|id| args.get_many::<String>(id).into_iter().flatten().cloned())
            .collect();
        println!("{sub}: {values:?}");
    }
}
