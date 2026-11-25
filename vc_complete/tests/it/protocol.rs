//! What the binary answers each shell, without the shell: bash driven with the variables readline sets, zsh and fish
//! by the arguments and variables their registration passes. These run everywhere; `shells` runs the real shells.
use super::scratch;
use vc_test::*;

const WORDBREAKS: &str = "\"'><=;|&(:@ \t\n";

/// Runs bash, sources `COMPLETE=bash vcdemo` and completes `line` (cursor at its end) as readline would: the
/// candidates bash would offer, one per line.
fn bash(line: &str) -> Vec<String> {
    let Some(bash) = which("bash") else {
        return vec!["no bash".into()];
    };
    let dir = scratch("bash");
    let script = format!(
        r#"source <(COMPLETE=bash vcdemo)
COMP_LINE={line}
COMP_POINT=${{#COMP_LINE}}
COMP_WORDBREAKS=$'"\'><=;|&(:@ \t\n'
_vc_complete_vcdemo vcdemo
printf '%s\n' "${{COMPREPLY[@]}}""#,
        line = vc_complete::quote(line)
    );
    let out = clean(&bash, &dir)
        .args(["--norc", "--noprofile", "-c", &script])
        .output()
        .expect("bash runs");
    assert!(
        out.status.success() && out.stderr.is_empty(),
        "bash: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout)
        .unwrap()
        .lines()
        .filter(|l| !l.is_empty())
        .map(String::from)
        .collect()
}

fn skip_without_bash(got: &[String]) -> bool {
    got == ["no bash"]
}

#[test]
fn bash_completes_subcommands_not_files() {
    let got = bash("vcdemo ");
    if skip_without_bash(&got) {
        return;
    }
    for sub in ["open", "find", "host", "fail", "panic", "pick", "shell"] {
        assert!(got.iter().any(|c| c == sub), "{sub} missing in {got:?}");
    }
    assert!(!got.iter().any(|c| c.contains("some-file")), "{got:?}");
}

#[test]
fn bash_quotes_values() {
    let got = bash("vcdemo open al");
    if skip_without_bash(&got) {
        return;
    }
    assert_eq!(got, ["'alpha one'", "'alpha two'"]);
    assert_eq!(bash("vcdemo open it"), [r"'it'\''s'"]);
    assert_eq!(bash("vcdemo open 'alpha t"), ["alpha two"]);
    assert_eq!(bash(r#"vcdemo open "it"#), ["it's"]);
    assert_eq!(bash("vcdemo open b"), ["beta"]);
}

#[test]
fn bash_completes_after_a_word_break() {
    let got = bash("vcdemo host user@c");
    if skip_without_bash(&got) {
        return;
    }
    assert_eq!(
        got,
        ["@core"],
        "bash keeps `user` and puts `@core` after it"
    );
    assert_eq!(bash("vcdemo host us"), ["user@build", "user@core"]);
}

#[test]
fn bash_keeps_the_word_among_candidates_that_replace_it() {
    let got = bash("vcdemo find alpha");
    if skip_without_bash(&got) {
        return;
    }
    assert_eq!(
        got,
        ["'alpha one'", "'alpha two'"],
        "both continue the word"
    );
    assert_eq!(bash("vcdemo find two"), ["'alpha two'"]);
    assert_eq!(
        bash("vcdemo find e"),
        ["'alpha one'", "beta", "e"],
        "readline would put their common start in place of `e`; the word itself keeps it"
    );
}

#[test]
fn bash_gives_nothing_for_a_failing_value_and_says_nothing() {
    let got = bash("vcdemo fail x");
    if skip_without_bash(&got) {
        return;
    }
    assert!(got.is_empty(), "{got:?}");
    assert!(bash("vcdemo panic x").is_empty());
    assert_eq!(
        bash("vcdemo fail "),
        ["--help"],
        "an empty word gets the flags, as clap gives them"
    );
}

#[test]
fn completers_know_the_shell() {
    let got = bash("vcdemo shell x");
    if skip_without_bash(&got) {
        return;
    }
    assert_eq!(got, ["bash"]);
}

/// The binary's answer to one completion request, as a shell's registration asks for it.
fn ask(shell: &str, args: &[&str], env: &[(&str, &str)]) -> String {
    let dir = scratch(&format!("ask-{shell}"));
    let out = clean(&dir.0.join("bin/vcdemo"), &dir)
        .env("COMPLETE", shell)
        .envs(env.iter().copied())
        .arg("--")
        .args(args)
        .output()
        .expect("vcdemo runs");
    assert!(out.status.success(), "{shell} {args:?}: {:?}", out.status);
    assert!(
        out.stderr.is_empty(),
        "{shell} {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).unwrap()
}

#[test]
fn zsh_and_fish_words_lose_their_quotes() {
    let index = [("_CLAP_COMPLETE_INDEX", "2"), ("_CLAP_IFS", "\n")];
    assert_eq!(
        ask("zsh", &["vcdemo", "open", "'alpha t"], &index),
        "alpha two:the second document"
    );
    assert_eq!(
        ask("fish", &["vcdemo", "open", "\"it"], &[]),
        "it's\ta name with a quote\n"
    );
    assert_eq!(
        ask("fish", &["vcdemo", "open", "alpha\\ o"], &[]),
        "alpha one\tthe first document\n"
    );
}

#[test]
fn zsh_gets_values_with_help_and_escaped_colons() {
    let index = [("_CLAP_COMPLETE_INDEX", "2"), ("_CLAP_IFS", "\n")];
    let got = ask("zsh", &["vcdemo", "open", "al"], &index);
    assert_eq!(
        got,
        "alpha one:the first document\nalpha two:the second document"
    );
    let got = ask("zsh", &["vcdemo", "host", "user@b"], &index);
    assert_eq!(got, "user@build:the build machine");
    assert_eq!(ask("zsh", &["vcdemo", "fail", "x"], &index), "");
    assert_eq!(ask("zsh", &["vcdemo", "panic", "x"], &index), "");
    assert_eq!(
        ask("zsh", &["vcdemo", "shell", "x"], &index),
        "zsh:the shell asking"
    );
    let index = [("_CLAP_COMPLETE_INDEX", "2")];
    assert_eq!(
        ask("zsh", &["vcdemo", "open"], &index),
        "alpha one:the first document\nalpha two:the second document\nbeta:a plain name\nit's:a name with a quote\n--help:Print help",
        "an empty current word zsh leaves out"
    );
}

#[test]
fn fish_gets_values_with_help_and_the_picked_mark() {
    assert_eq!(
        ask("fish", &["vcdemo", "open", "al"], &[]),
        "alpha one\tthe first document\nalpha two\tthe second document\n"
    );
    assert_eq!(
        ask("fish", &["vcdemo", "pick", "zz"], &[]),
        "\x1falpha two\tpicked\n"
    );
    assert_eq!(ask("fish", &["vcdemo", "fail", "x"], &[]), "");
    assert_eq!(ask("fish", &["vcdemo", "panic", "x"], &[]), "");
    assert_eq!(
        ask("fish", &["vcdemo", "shell", "x"], &[]),
        "fish\tthe shell asking\n"
    );
}

#[test]
fn bash_line_variables_are_read_directly() {
    let got = ask(
        "bash",
        &["al"],
        &[
            ("_VC_COMP_LINE", "vcdemo open al"),
            ("_VC_COMP_WORDBREAKS", WORDBREAKS),
            ("_CLAP_IFS", "\n"),
        ],
    );
    assert_eq!(got, "'alpha one'\n'alpha two'");
}

#[test]
fn registrations_name_the_binary_and_the_function() {
    let dir = scratch("registration");
    for (shell, wants) in [
        (
            "bash",
            &["_vc_complete_vcdemo()", "complete -o nospace", " vcdemo\n"][..],
        ),
        (
            "zsh",
            &["#compdef vcdemo", "compdef _vc_complete_vcdemo vcdemo"][..],
        ),
        (
            "fish",
            &["function __vc_complete_vcdemo", "--command vcdemo"][..],
        ),
    ] {
        let out = clean(&dir.0.join("bin/vcdemo"), &dir)
            .env("COMPLETE", shell)
            .output()
            .unwrap();
        let script = String::from_utf8(out.stdout).unwrap();
        for want in wants {
            assert!(script.contains(want), "{shell}: no {want:?} in\n{script}");
        }
        assert!(!script.contains("bashdefault"), "{shell}");
        for placeholder in ["NAME", "COMPLETER", "EXTRA", "VAR="] {
            assert!(
                !script.contains(placeholder),
                "{shell}: {placeholder} left in\n{script}"
            );
        }
    }
}

#[test]
fn without_complete_the_cli_runs() {
    let dir = scratch("run");
    let out = clean(&dir.0.join("bin/vcdemo"), &dir)
        .args(["open", "beta"])
        .output()
        .unwrap();
    assert_eq!(String::from_utf8(out.stdout).unwrap(), "open: [\"beta\"]\n");
}
