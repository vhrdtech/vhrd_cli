//! The shell adapters: what each shell's registration script looks like, and how the binary answers it.

use crate::line::{LINE_VAR, WORDBREAKS_VAR, quote, split_line};
use crate::{PICKED, SHELL, extra};
use clap::Command;
use clap_complete::CompletionCandidate;
use clap_complete::env::EnvCompleter;
use std::ffi::OsString;
use std::io::Write;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::Path;

/// The candidates from the completers, or none: an error or a panic in a completer must not reach the line being
/// typed, so the panic message is silenced too (the process only answers this one request).
fn candidates(
    cmd: &mut Command,
    args: Vec<OsString>,
    index: usize,
    current_dir: Option<&Path>,
) -> Vec<CompletionCandidate> {
    std::panic::set_hook(Box::new(|_| {}));
    catch_unwind(AssertUnwindSafe(|| {
        clap_complete::engine::complete(cmd, args, index, current_dir)
    }))
    .ok()
    .and_then(Result::ok)
    .unwrap_or_default()
}

/// The registration script with its placeholders filled in, in one pass, so a path holding `BIN` or `VAR` stays
/// as it is. `EXTRA` goes in first, so the extra lines get their placeholders filled too.
fn fill(script: &str, shell: &str, var: &str, name: &str, bin: &str, completer: &str) -> String {
    let script = script.replace("EXTRA", extra(shell));
    let name = name.replace('-', "_");
    let tokens = [
        ("NAME", name.as_str()),
        ("COMPLETER", completer),
        ("BIN", bin),
        ("VAR", var),
    ];
    let mut out = String::with_capacity(script.len());
    let mut rest = script.as_str();
    while !rest.is_empty() {
        if let Some((token, value)) = tokens.iter().find(|(t, _)| rest.starts_with(t)) {
            out.push_str(value);
            rest = &rest[token.len()..];
        } else {
            let c = rest.chars().next().unwrap_or_default();
            out.push(c);
            rest = &rest[c.len_utf8()..];
        }
    }
    out
}

/// A word as zsh and fish pass it, quotes and escapes still on (`'alpha t`), as the value it stands for
/// (`alpha t`): the completers match values, not shell syntax.
fn unquote(word: OsString) -> OsString {
    match word.to_str() {
        Some(w) if w.contains(['\'', '"', '\\']) => split_line(w, "").1.value.into(),
        _ => word,
    }
}

/// The first line of a help text: shells show one line per candidate.
fn first_line(c: &CompletionCandidate) -> Option<String> {
    c.get_help()
        .map(|h| h.to_string().lines().next().unwrap_or_default().to_string())
}

// ---------------------------------------------------------------- bash

/// bash, with quoting. bash puts a candidate in verbatim and splits the word being completed at `COMP_WORDBREAKS`
/// (quotes, `@`, `:`, `=`, ...), and clap_complete's own adapter handles neither, so values with spaces broke into
/// several words. Here the binary splits the raw line itself and turns each candidate into the text bash should put
/// in place of its current word ([`crate::CurrentWord::reply`]). No `-o bashdefault`: it makes bash list file
/// names whenever the function gives no candidates (an error, an older binary); file arguments complete their paths
/// through clap's `ValueHint`s instead. A candidate ending in `=`, `/`, `:` or `#` gets no space after it.
pub(crate) struct Bash;

impl EnvCompleter for Bash {
    fn name(&self) -> &'static str {
        "bash"
    }

    fn is(&self, name: &str) -> bool {
        name == "bash"
    }

    fn write_registration(
        &self,
        var: &str,
        name: &str,
        bin: &str,
        completer: &str,
        buf: &mut dyn Write,
    ) -> std::io::Result<()> {
        let script = r#"
_vc_complete_NAME() {
    local IFS=$'\013'
    if compopt +o nospace 2> /dev/null; then
        local space=false
    else
        local space=true
    fi
    COMPREPLY=( $( \
        LINE_VAR="${COMP_LINE:0:COMP_POINT}" \
        WORDBREAKS_VAR="$COMP_WORDBREAKS" \
        _CLAP_IFS="$IFS" \
        VAR="bash" \
        'COMPLETER' -- "$1" 2> /dev/null \
    ) )
    if [[ $? != 0 ]]; then
        unset COMPREPLY
    elif [[ $space == false ]] && [[ "${COMPREPLY-}" =~ [=/:#]\'?$ ]]; then
        compopt -o nospace
    fi
}
if [[ "${BASH_VERSINFO[0]}" -eq 4 && "${BASH_VERSINFO[1]}" -ge 4 || "${BASH_VERSINFO[0]}" -gt 4 ]]; then
    complete -o nospace -o nosort -F _vc_complete_NAME BIN
else
    complete -o nospace -F _vc_complete_NAME BIN
fi
EXTRA"#
            .replace("WORDBREAKS_VAR", WORDBREAKS_VAR)
            .replace("LINE_VAR", LINE_VAR);
        let completer = completer.replace('\'', r"'\''");
        writeln!(
            buf,
            "{}",
            fill(&script, "bash", var, name, &quote(bin), &completer)
        )
    }

    fn write_complete(
        &self,
        cmd: &mut Command,
        _args: Vec<OsString>,
        current_dir: Option<&Path>,
        buf: &mut dyn Write,
    ) -> std::io::Result<()> {
        let _ = SHELL.set("bash");
        let line = std::env::var(LINE_VAR).unwrap_or_default();
        let wordbreaks = std::env::var(WORDBREAKS_VAR).unwrap_or_default();
        let ifs = std::env::var("_CLAP_IFS").unwrap_or_else(|_| "\n".into());
        let (words, current) = split_line(&line, &wordbreaks);
        let index = words.len() - 1;
        let args = words.into_iter().map(OsString::from).collect();
        let completions = candidates(cmd, args, index, current_dir);
        let mut replies: Vec<_> = completions
            .iter()
            .filter_map(|c| current.reply(&c.get_value().to_string_lossy()))
            .collect();
        // Candidates that replace the word rather than continue it (a search by words): readline would put their
        // common start in place of what was typed; the word itself among them keeps it.
        if replies.len() > 1
            && !completions
                .iter()
                .all(|c| c.get_value().to_string_lossy().starts_with(&current.value))
            && let Some(own) = current.reply(&current.value)
        {
            replies.push(own);
        }
        write!(buf, "{}", replies.join(&ifs))
    }
}

// ---------------------------------------------------------------- zsh

/// zsh: clap_complete's adapter with one change. `_describe` offers only candidates that start with the word being
/// completed; those that replace the word instead (a search by words) are added with `-U` in a menu, so TAB puts
/// the first in place of the word and cycles through them.
pub(crate) struct Zsh;

impl EnvCompleter for Zsh {
    fn name(&self) -> &'static str {
        "zsh"
    }

    fn is(&self, name: &str) -> bool {
        name == "zsh"
    }

    fn write_registration(
        &self,
        var: &str,
        name: &str,
        bin: &str,
        completer: &str,
        buf: &mut dyn Write,
    ) -> std::io::Result<()> {
        let script = r#"#compdef BIN
function _vc_complete_NAME() {
    local _CLAP_COMPLETE_INDEX=$(expr $CURRENT - 1)
    local _CLAP_IFS=$'\n'

    local completions=("${(@f)$( \
        _CLAP_IFS="$_CLAP_IFS" \
        _CLAP_COMPLETE_INDEX="$_CLAP_COMPLETE_INDEX" \
        VAR="zsh" \
        COMPLETER -- "${words[@]}" 2>/dev/null \
    )}")

    if [[ -n $completions ]]; then
        local -a dirs=()
        local -a other=()
        local -a loose=()
        local completion
        for completion in $completions; do
            local value="${completion%%:*}"
            if [[ "$value" == */ ]]; then
                local dir_no_slash="${value%/}"
                if [[ "$completion" == *:* ]]; then
                    local desc="${completion#*:}"
                    dirs+=("$dir_no_slash:$desc")
                else
                    dirs+=("$dir_no_slash")
                fi
            elif [[ "${(L)value//\\:/:}" == "${(Lb)PREFIX}"* ]]; then
                other+=("$completion")
            else
                loose+=("$completion")
            fi
        done
        [[ -n $dirs ]] && _describe -V 'values' dirs -S '/' -r '/'
        [[ -n $other ]] && _describe -V 'values' other
        if [[ -n $loose ]]; then
            _describe -V 'values' loose -U
            compstate[insert]=menu
        fi
    fi
}

compdef _vc_complete_NAME BIN
EXTRA"#;
        writeln!(
            buf,
            "{}",
            fill(script, "zsh", var, name, &quote(bin), &quote(completer))
        )
    }

    fn write_complete(
        &self,
        cmd: &mut Command,
        args: Vec<OsString>,
        current_dir: Option<&Path>,
        buf: &mut dyn Write,
    ) -> std::io::Result<()> {
        let _ = SHELL.set("zsh");
        let index: usize = std::env::var("_CLAP_COMPLETE_INDEX")
            .ok()
            .and_then(|i| i.parse().ok())
            .unwrap_or_default();
        let ifs = std::env::var("_CLAP_IFS").unwrap_or_else(|_| "\n".into());
        let mut args: Vec<OsString> = args.into_iter().map(unquote).collect();
        if args.len() == index {
            args.push("".into());
        }
        let lines: Vec<String> = candidates(cmd, args, index, current_dir)
            .iter()
            .map(|c| {
                let value = c
                    .get_value()
                    .to_string_lossy()
                    .replace('\\', "\\\\")
                    .replace(':', "\\:");
                match first_line(c) {
                    Some(help) => format!("{value}:{}", help.replace('\\', "\\\\")),
                    None => value,
                }
            })
            .collect();
        write!(buf, "{}", lines.join(&ifs))
    }
}

// ---------------------------------------------------------------- fish

/// fish: clap_complete's adapter with one change. fish drops a candidate its own matching finds unlike the token,
/// which a picker's choice may well be: such a candidate comes marked (`\x1f` before it, from [`PICKED`]) and the
/// function puts it in place of the token itself.
pub(crate) struct Fish;

impl EnvCompleter for Fish {
    fn name(&self) -> &'static str {
        "fish"
    }

    fn is(&self, name: &str) -> bool {
        name == "fish"
    }

    fn write_registration(
        &self,
        var: &str,
        name: &str,
        bin: &str,
        completer: &str,
        buf: &mut dyn Write,
    ) -> std::io::Result<()> {
        let script = r#"function __vc_complete_NAME
    set -l out (VAR=fish COMPLETER -- (commandline --current-process --tokenize --cut-at-cursor) (commandline --current-token) 2>/dev/null)
    if test (count $out) -eq 1; and string match -q -- \x1f'*' $out[1]
        commandline --current-token --replace -- (string escape -- (string sub --start 2 -- $out[1]))
        commandline -f repaint
        return
    end
    string join \n -- $out
end
complete --keep-order --exclusive --command BIN --arguments "(__vc_complete_NAME)"
EXTRA"#;
        writeln!(
            buf,
            "{}",
            fill(script, "fish", var, name, &quote(bin), &quote(completer))
        )
    }

    fn write_complete(
        &self,
        cmd: &mut Command,
        args: Vec<OsString>,
        current_dir: Option<&Path>,
        buf: &mut dyn Write,
    ) -> std::io::Result<()> {
        let _ = SHELL.set("fish");
        let args: Vec<OsString> = args.into_iter().map(unquote).collect();
        let index = args.len().saturating_sub(1);
        for c in candidates(cmd, args, index, current_dir) {
            if c.get_id().is_some_and(|id| id == PICKED) {
                write!(buf, "\x1f")?;
            }
            write!(buf, "{}", c.get_value().to_string_lossy())?;
            if let Some(help) = first_line(&c) {
                write!(buf, "\t{help}")?;
            }
            writeln!(buf)?;
        }
        Ok(())
    }
}
