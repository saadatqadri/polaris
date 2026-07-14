//! Smart-punctuation transforms, applied at input time so the file itself
//! carries the real characters (DESIGN.md "Smart typography as you type").
//!
//! These are pure functions: given the text before the cursor and the char
//! just typed, they say what to substitute. **Context is the caller's
//! responsibility** — do not call this inside inline code spans or fenced
//! code blocks (wired in M4), and offer backspace-right-after to revert.

/// A substitution to apply instead of inserting the typed character.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Substitution {
    /// Chars to delete immediately before the cursor first.
    pub delete_before: usize,
    /// Text to insert in place of the typed character.
    pub insert: &'static str,
}

/// Like [`substitute`], but returns `None` inside a markdown code context
/// (fenced block or inline span), so `--verbose` and `"flags"` stay literal.
/// This is the guard every front-end should use.
pub fn substitute_in_context(before: &str, typed: char) -> Option<Substitution> {
    if in_code_context(before) {
        return None;
    }
    substitute(before, typed)
}

/// Inside a fenced code block (odd number of ``` fence lines so far) or an
/// inline code span (odd number of backticks on the current line)?
pub fn in_code_context(before: &str) -> bool {
    let fences = before
        .lines()
        .filter(|l| l.trim_start().starts_with("```"))
        .count();
    if fences % 2 == 1 {
        return true;
    }
    let line = before.rsplit('\n').next().unwrap_or(before);
    line.chars().filter(|&c| c == '`').count() % 2 == 1
}

/// Decide the smart-punctuation substitution for `typed`, given the document
/// text before the cursor. Returns `None` when the character should be
/// inserted literally. Does **not** check code context — use
/// [`substitute_in_context`] unless you have already guarded.
pub fn substitute(before: &str, typed: char) -> Option<Substitution> {
    match typed {
        '"' => Some(Substitution {
            delete_before: 0,
            insert: if opens_quote(before) {
                "\u{201C}"
            } else {
                "\u{201D}"
            },
        }),
        '\'' => Some(Substitution {
            delete_before: 0,
            insert: if opens_quote(before) {
                "\u{2018}"
            } else {
                "\u{2019}"
            },
        }),
        // "--" becomes an em dash — except on hyphen-only lines, so markdown
        // horizontal rules ("---") and front-matter fences stay typeable.
        '-' if before.ends_with('-') && !current_line_is_hyphens(before) => Some(Substitution {
            delete_before: 1,
            insert: "\u{2014}",
        }),
        // "..." becomes an ellipsis.
        '.' if before.ends_with("..") && !before.ends_with("...") => Some(Substitution {
            delete_before: 2,
            insert: "\u{2026}",
        }),
        _ => None,
    }
}

/// A quote typed here should be an opening quote: at the start of the text,
/// after whitespace, or after an opening bracket/quote/dash.
fn opens_quote(before: &str) -> bool {
    match before.chars().last() {
        None => true,
        Some(c) => {
            c.is_whitespace()
                || matches!(
                    c,
                    '(' | '[' | '{' | '\u{201C}' | '\u{2018}' | '\u{2014}' | '-' | '/'
                )
        }
    }
}

fn current_line_is_hyphens(before: &str) -> bool {
    let line = before.rsplit('\n').next().unwrap_or(before);
    !line.is_empty() && line.chars().all(|c| c == '-')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn apply(text: &str, typed: char) -> String {
        match substitute(text, typed) {
            Some(sub) => {
                let keep: String = {
                    let chars: Vec<char> = text.chars().collect();
                    chars[..chars.len() - sub.delete_before].iter().collect()
                };
                format!("{}{}", keep, sub.insert)
            }
            None => format!("{}{}", text, typed),
        }
    }

    #[test]
    fn double_quotes_curl_by_context() {
        assert_eq!(apply("", '"'), "\u{201C}");
        assert_eq!(apply("he said ", '"'), "he said \u{201C}");
        assert_eq!(apply("(", '"'), "(\u{201C}");
        assert_eq!(
            apply("he said \u{201C}hi", '"'),
            "he said \u{201C}hi\u{201D}"
        );
        assert_eq!(apply("word", '"'), "word\u{201D}");
    }

    #[test]
    fn single_quotes_and_apostrophes() {
        assert_eq!(apply("", '\''), "\u{2018}");
        assert_eq!(apply("say ", '\''), "say \u{2018}");
        // apostrophe inside a word closes
        assert_eq!(apply("it", '\''), "it\u{2019}");
        assert_eq!(apply("writers", '\''), "writers\u{2019}");
    }

    #[test]
    fn double_hyphen_becomes_em_dash() {
        assert_eq!(apply("word -", '-'), "word \u{2014}");
        assert_eq!(apply("word-", '-'), "word\u{2014}");
    }

    #[test]
    fn hyphen_only_lines_stay_literal_for_markdown_rules() {
        // "---" horizontal rule / front-matter fence must remain typeable
        assert_eq!(apply("-", '-'), "--");
        assert_eq!(apply("--", '-'), "---");
        assert_eq!(apply("text\n-", '-'), "text\n--");
        // but an em dash after a word mid-line still works
        assert_eq!(apply("text\nword -", '-'), "text\nword \u{2014}");
    }

    #[test]
    fn single_hyphen_is_literal() {
        assert_eq!(apply("word ", '-'), "word -");
    }

    #[test]
    fn triple_dot_becomes_ellipsis() {
        assert_eq!(apply("wait..", '.'), "wait\u{2026}");
        // a lone or double dot stays literal
        assert_eq!(apply("wait", '.'), "wait.");
        assert_eq!(apply("wait.", '.'), "wait..");
        // and we don't chain onto an existing ellipsis or four dots
        assert_eq!(apply("wait...", '.'), "wait....");
    }

    #[test]
    fn other_chars_pass_through() {
        assert_eq!(substitute("anything", 'a'), None);
        assert_eq!(substitute("anything", ' '), None);
    }

    #[test]
    fn context_guard_skips_code() {
        // Inline span (odd backticks on the line): no substitution.
        assert_eq!(substitute_in_context("run `--verbose", '-'), None);
        assert!(in_code_context("run `--verbose"));
        // Closed span: back to normal.
        assert!(!in_code_context("run `x` then --"));
        assert!(substitute_in_context("run `x` then --", '-').is_some());
        // Fenced block (odd fences).
        assert!(in_code_context("```\ncode --"));
        assert_eq!(substitute_in_context("```\ncode --", '-'), None);
        assert!(!in_code_context("```\ncode\n```\nprose --"));
    }
}
