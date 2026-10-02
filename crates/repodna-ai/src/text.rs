//! Text hygiene for everything that crosses the model boundary.

/// Removes control characters (keeping newlines and tabs), including terminal escape
/// sequences, collapses runs of blank lines, trims, and limits the length to `max_chars`
/// characters.
pub fn sanitize(text: &str, max_chars: usize) -> String {
    let mut out = String::with_capacity(text.len().min(max_chars));
    let mut count = 0;
    let mut newlines = 0;
    for c in text.chars() {
        if c.is_control() && c != '\n' && c != '\t' {
            continue;
        }
        if c == '\n' {
            newlines += 1;
            if newlines > 2 {
                continue;
            }
        } else {
            newlines = 0;
        }
        if count == max_chars {
            out.push('…');
            break;
        }
        out.push(c);
        count += 1;
    }
    out.trim().to_owned()
}

/// Like [`sanitize`], but also folds the text onto one line.
pub fn single_line(text: &str, max_chars: usize) -> String {
    sanitize(text, max_chars)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Rough token estimate: about four characters per token for English text and code.
pub fn estimate_tokens(text: &str) -> u32 {
    u32::try_from(text.chars().count().div_ceil(4)).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_control_characters_and_limits_length() {
        assert_eq!(sanitize("a\u{1b}[31mb\r\nc", 100), "a[31mb\nc");
        assert_eq!(sanitize("one\n\n\n\n\ntwo", 100), "one\n\ntwo");
        assert_eq!(sanitize("abcdef", 3), "abc…");
        assert_eq!(single_line(" a \n b\tc ", 100), "a b c");
        assert_eq!(estimate_tokens("abcdefgh"), 2);
        assert_eq!(estimate_tokens("abcdefghi"), 3);
    }
}
