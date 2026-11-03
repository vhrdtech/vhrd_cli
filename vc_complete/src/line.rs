//! The command line being completed: its words, the word under the cursor as bash sees it, and the text bash should
//! put in its place.

/// Set by the bash registration: the line up to the cursor (`COMP_LINE`).
pub(crate) const LINE_VAR: &str = "_VC_COMP_LINE";
/// Set by the bash registration: bash's word break characters (`COMP_WORDBREAKS`).
pub(crate) const WORDBREAKS_VAR: &str = "_VC_COMP_WORDBREAKS";

/// The words of the command line being completed, the binary's name first and the word being completed last. Read
/// it from a completer to see the words before the current one; outside completion it is empty.
///
/// ```
/// assert!(vc_complete::comp_words().is_empty() || std::env::args().any(|a| a == "--"));
/// ```
pub fn comp_words() -> Vec<String> {
    if let Ok(line) = std::env::var(LINE_VAR) {
        return split_line(&line, "").0;
    }
    std::env::args().skip_while(|a| a != "--").skip(1).collect()
}

/// The word right after `word` on the command line being completed, if any: the subcommand's first argument, a
/// flag's value.
///
/// ```
/// assert_eq!(vc_complete::word_after("no-such-subcommand"), None);
/// ```
pub fn word_after(word: &str) -> Option<String> {
    let w = comp_words();
    let i = w.iter().position(|x| x == word)?;
    w.get(i + 1).cloned()
}

/// The word being completed, as bash sees it.
#[derive(Debug, PartialEq, Eq)]
pub struct CurrentWord {
    /// The word without its quotes and escapes.
    pub value: String,
    /// The quote left open, if any.
    pub open_quote: Option<char>,
    /// How much of the start of `value` bash keeps: up to the open quote or the last word break.
    pub kept: usize,
}

/// Splits a command line (up to the cursor) into shell words, the last one being the word being completed, which is
/// also returned as a [`CurrentWord`]. `wordbreaks` is bash's `COMP_WORDBREAKS`.
///
/// ```
/// let (words, current) = vc_complete::split_line(r#"mycli --label 'a b' "c\"d" e\ f us"#, "\"'@:=");
/// assert_eq!(words, ["mycli", "--label", "a b", "c\"d", "e f", "us"]);
/// assert_eq!(current.value, "us");
/// ```
pub fn split_line(line: &str, wordbreaks: &str) -> (Vec<String>, CurrentWord) {
    let mut words = vec![];
    let mut word = String::new();
    let mut in_word = false;
    let mut quote = None;
    let mut kept = 0;
    let mut chars = line.chars();
    while let Some(c) = chars.next() {
        match (quote, c) {
            (Some(q), c) if c == q => {
                quote = None;
                kept = word.len(); // bash treats the closing quote as a word break
            }
            (Some('"'), '\\') => match chars.next() {
                Some(e @ ('"' | '\\' | '$' | '`')) => word.push(e),
                Some(e) => {
                    word.push('\\');
                    word.push(e);
                }
                None => word.push('\\'),
            },
            (Some(_), c) => word.push(c),
            (None, '\'' | '"') => {
                quote = Some(c);
                in_word = true;
                kept = word.len();
            }
            (None, '\\') => {
                in_word = true;
                if let Some(e) = chars.next() {
                    word.push(e);
                }
            }
            (None, c) if c.is_whitespace() => {
                if in_word {
                    words.push(std::mem::take(&mut word));
                    in_word = false;
                }
                kept = 0;
            }
            (None, c) => {
                in_word = true;
                if wordbreaks.contains(c) {
                    // bash keeps `@` and `$` (its rl_special_prefixes) as the start of the next word
                    kept = word.len()
                        + if matches!(c, '@' | '$') {
                            0
                        } else {
                            c.len_utf8()
                        };
                }
                word.push(c);
            }
        }
    }
    words.push(word.clone());
    (
        words,
        CurrentWord {
            value: word,
            open_quote: quote,
            kept,
        },
    )
}

impl CurrentWord {
    /// The text to put in place of bash's current word so the line holds `candidate`: only the part after the last
    /// word break, escaped for a quote left open, or single-quoted when needed. `None` when the candidate does not
    /// continue what bash keeps of the word.
    ///
    /// ```
    /// let (_, w) = vc_complete::split_line("mycli --label N", "\"'@:=");
    /// assert_eq!(w.reply("Nucleo on the desk").as_deref(), Some("'Nucleo on the desk'"));
    /// let (_, w) = vc_complete::split_line("mycli --api blinky@", "\"'@:=");
    /// assert_eq!(w.reply("blinky@^0.1").as_deref(), Some("@^0.1"));
    /// ```
    pub fn reply(&self, candidate: &str) -> Option<String> {
        let rest = candidate.strip_prefix(&self.value[..self.kept])?;
        Some(match self.open_quote {
            Some('"') => rest
                .chars()
                .flat_map(|c| match c {
                    '"' | '\\' | '$' | '`' => vec!['\\', c],
                    c => vec![c],
                })
                .collect(),
            Some(_) => rest.replace('\'', r"'\''"),
            None => quote(rest),
        })
    }
}

/// Single-quotes a value that holds characters the shell would read (spaces, quotes, `$`, a leading `~` or `#`);
/// a plain value stays as it is.
///
/// ```
/// assert_eq!(vc_complete::quote("2100AB"), "2100AB");
/// assert_eq!(vc_complete::quote("a b"), "'a b'");
/// assert_eq!(vc_complete::quote("it's"), r"'it'\''s'");
/// ```
pub fn quote(v: &str) -> String {
    // shell syntax is ASCII: emoji and other non-ASCII text needs no quotes
    let plain = |c: char| !c.is_ascii() || c.is_ascii_alphanumeric() || "-_.,:/@%+=^~#".contains(c);
    if v.chars().all(plain) && !v.starts_with(['#', '~']) {
        v.to_string()
    } else {
        format!("'{}'", v.replace('\'', r"'\''"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WORDBREAKS: &str = "\"'><=;|&(:@";

    fn reply(line: &str, candidate: &str) -> Option<String> {
        split_line(line, WORDBREAKS).1.reply(candidate)
    }

    #[test]
    fn splits_words() {
        let (words, current) = split_line(r#"ww  --label 'a b' "c\"d" e\ f g"#, WORDBREAKS);
        assert_eq!(words, ["ww", "--label", "a b", "c\"d", "e f", "g"]);
        assert_eq!(current.value, "g");
        let (words, current) = split_line("ww --label ", WORDBREAKS);
        assert_eq!(words, ["ww", "--label", ""]);
        assert_eq!(current.value, "");
    }

    #[test]
    fn quotes_values_needing_it() {
        assert_eq!(
            reply("ww --label N", "Nucleo on the desk").unwrap(),
            "'Nucleo on the desk'"
        );
        assert_eq!(reply("ww --label ", "it's").unwrap(), r"'it'\''s'");
        assert_eq!(reply("ww --serial 21", "2100AB").unwrap(), "2100AB");
        assert_eq!(reply("todo set T18#3 ", "⛔").unwrap(), "⛔");
        assert_eq!(reply("todo done T1", "T18#").unwrap(), "T18#");
    }

    #[test]
    fn quotes_what_the_shell_would_expand_at_the_start() {
        assert_eq!(quote("~home"), "'~home'");
        assert_eq!(quote("#tag"), "'#tag'");
        assert_eq!(
            quote(""),
            "",
            "nothing left after a word break stays nothing"
        );
        assert_eq!(quote("a~b#c"), "a~b#c");
    }

    #[test]
    fn escapes_for_open_quote() {
        assert_eq!(
            reply("ww --label 'Nu", "Nucleo on the desk").unwrap(),
            "Nucleo on the desk"
        );
        assert_eq!(
            reply(r#"ww --label "Nu"#, r#"Nu "x" $y"#).unwrap(),
            r#"Nu \"x\" \$y"#
        );
        assert_eq!(
            reply("ww --label 'a b' --label 'it", "it's").unwrap(),
            r"it'\''s"
        );
    }

    #[test]
    fn replaces_after_word_break() {
        assert_eq!(
            reply("ww --api blinky_api@", "blinky_api@^0.1.0").unwrap(),
            "@^0.1.0"
        );
        assert_eq!(reply("ww --vid-pid c0de:", "c0de:cafe").unwrap(), "cafe");
        assert_eq!(
            reply("ww --label=Nu", "--label=Nucleo on the desk").unwrap(),
            "'Nucleo on the desk'"
        );
        assert_eq!(reply("ww --api blinky_api@", "other"), None);
    }
}
