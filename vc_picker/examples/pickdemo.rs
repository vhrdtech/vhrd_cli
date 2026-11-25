//! A small CLI with the picker on Tab and on a key, the way a real one uses vc_picker; the tests drive it in the
//! shells.
//!
//! Try it in bash: `cargo build --example pickdemo`, put `target/debug/examples` on `PATH`, then
//! `source <(COMPLETE=bash pickdemo)`, `bind -x '"\ed": _vc_pick_pickdemo'`, and `pickdemo open widget<TAB>` or
//! Alt+D on an empty line.

use clap::{Arg, ArgAction, Command};
use clap_complete::{ArgValueCompleter, CompletionCandidate};
use std::ffi::OsStr;
use std::process::ExitCode;
use vc_picker::{Action, Column, Fuzzy, Item, Outcome, Picker, Width};

/// A document: its number, title and file name.
struct Doc {
    no: &'static str,
    title: &'static str,
    file: &'static str,
}

const DOCS: [Doc; 4] = [
    Doc {
        no: "DS-100",
        title: "Widget controller datasheet",
        file: "widget-ds100.txt",
    },
    Doc {
        no: "DS-100",
        title: "Widget controller datasheet, revision 2",
        file: "widget-ds100-rev2.txt",
    },
    Doc {
        no: "RM-200",
        title: "Widget reference manual",
        file: "widget-rm200.txt",
    },
    Doc {
        no: "AN-7",
        title: "Getting started note",
        file: "an7-start.txt",
    },
];

const COLS: [Column; 3] = [
    Column {
        head: "no",
        width: Width::Fit { max: 10 },
        keep: 5,
    },
    Column {
        head: "title",
        width: Width::Fill {
            min: 12,
            weight: 3,
            left: false,
        },
        keep: 9,
    },
    Column {
        head: "file",
        width: Width::Fill {
            min: 10,
            weight: 2,
            left: true,
        },
        keep: 7,
    },
];

/// Where the documents are, for Ctrl-Y.
const DIR: &str = "/docs";

fn rows() -> Vec<Item> {
    DOCS.iter()
        .map(|d| Item {
            cells: vec![d.no.into(), d.title.into(), d.file.into()],
            extra: String::new(),
            detail: format!("{DIR}/{}", d.file),
            dim: false,
        })
        .collect()
}

/// The text a picked document puts on the line: its file name, or with Ctrl-Y its full path.
fn value(row: usize, how: Action) -> String {
    match how {
        Action::Accept => DOCS[row].file.to_owned(),
        Action::Alternate => format!("{DIR}/{}", DOCS[row].file),
    }
}

/// The documents every word finds in their number, title or file name.
fn find(words: &[String]) -> Vec<usize> {
    (0..DOCS.len())
        .filter(|&i| {
            let d = &DOCS[i];
            let hay = format!("{} {} {}", d.no, d.title, d.file).to_lowercase();
            words.iter().all(|w| hay.contains(&w.to_lowercase()))
        })
        .collect()
}

/// `open WORDS<TAB>`: one document completes at once; several or none open the picker with the words as the query.
fn open(typed: &OsStr) -> Vec<CompletionCandidate> {
    let words = vc_complete::comp_words();
    let start = words.iter().position(|w| w == "open").map_or(0, |i| i + 1);
    let mut query: Vec<String> =
        words[start.min(words.len())..words.len().saturating_sub(1)].to_vec();
    query.push(typed.to_string_lossy().into_owned());
    query.retain(|w| !w.is_empty());
    let hits = find(&query);
    if hits.len() != 1 {
        let q = query.join(" ");
        let picked = vc_picker::tab(
            "pickdemo",
            || Picker::new(&COLS, rows(), &Fuzzy, &q).hints("insert", Some("path")),
            value,
        );
        if let Some(candidates) = picked {
            return candidates;
        }
    }
    hits.iter()
        .map(|&i| vc_complete::cand(DOCS[i].file, DOCS[i].title))
        .collect()
}

fn cli() -> Command {
    Command::new("pickdemo")
        .about("Shows vc_picker on Tab and on a key")
        .subcommand(
            Command::new("open").about("Open documents by words").arg(
                Arg::new("words")
                    .action(ArgAction::Append)
                    .num_args(1..)
                    .add(ArgValueCompleter::new(open)),
            ),
        )
        .subcommand(
            Command::new("pick")
                .about("Pick a document and print its file name (Ctrl-Y: its path)"),
        )
}

fn main() -> ExitCode {
    vc_complete::Completion::new(cli)
        .pick_widget("pick", "open")
        .complete();
    match cli().get_matches().subcommand() {
        Some(("open", args)) => {
            let words: Vec<&String> = args.get_many("words").into_iter().flatten().collect();
            println!("open: {words:?}");
            ExitCode::SUCCESS
        }
        Some(("pick", _)) => {
            let mut p = Picker::new(&COLS, rows(), &Fuzzy, "").hints("insert", Some("path"));
            match vc_picker::run(&mut p, vc_picker::color()) {
                Ok(Outcome::Picked(row, how)) => {
                    println!("{}", value(row, how));
                    ExitCode::SUCCESS
                }
                Ok(Outcome::Cancelled) => ExitCode::FAILURE,
                Err(e) => {
                    eprintln!("pickdemo: {e}");
                    ExitCode::FAILURE
                }
            }
        }
        _ => ExitCode::SUCCESS,
    }
}
