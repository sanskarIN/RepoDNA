//! The generic lexical scanner.
//!
//! A single pass over the file's bytes tracks whether the cursor is in code, a comment, or
//! a string literal, using the language's [`Syntax`]. For every line it produces:
//!
//! * `code` — the line with comments removed and string contents intact (for imports),
//! * `masked` — the line with comments removed and string contents replaced by spaces
//!   (for brace matching, keywords, and symbol patterns, which must ignore string text),
//! * `comment` — the text of comments on the line (for TODO/FIXME markers).
//!
//! All delimiters are ASCII, so the scanner works on bytes without ever splitting a
//! multi-byte UTF-8 sequence.

use repodna_core::model::structure::LineCounts;

use crate::spec::Syntax;

/// Classification of one line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineKind {
    /// Only whitespace.
    Blank,
    /// Contains code (possibly with a trailing comment).
    Code,
    /// Contains only comments.
    Comment,
}

/// One scanned line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScannedLine {
    /// Classification.
    pub kind: LineKind,
    /// Comments removed; string contents intact.
    pub code: String,
    /// Comments removed; string contents replaced by spaces.
    pub masked: String,
    /// Comment text on the line.
    pub comment: String,
}

/// The result of scanning a file.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ScannedFile {
    /// Lines in order; line `n` (1-based) is at index `n - 1`.
    pub lines: Vec<ScannedLine>,
}

impl ScannedFile {
    /// Counts code, comment, and blank lines.
    pub fn line_counts(&self) -> LineCounts {
        let mut counts = LineCounts {
            total: self.lines.len() as u64,
            ..LineCounts::default()
        };
        for line in &self.lines {
            match line.kind {
                LineKind::Blank => counts.blank += 1,
                LineKind::Code => counts.code += 1,
                LineKind::Comment => counts.comment += 1,
            }
        }
        counts
    }
}

#[derive(Default)]
struct LineBuilder {
    code: Vec<u8>,
    masked: Vec<u8>,
    comment: Vec<u8>,
    has_code: bool,
    has_comment: bool,
}

impl LineBuilder {
    fn finish(&mut self) -> ScannedLine {
        let kind = if self.has_code {
            LineKind::Code
        } else if self.has_comment {
            LineKind::Comment
        } else {
            LineKind::Blank
        };
        let line = ScannedLine {
            kind,
            code: String::from_utf8_lossy(&self.code).into_owned(),
            masked: String::from_utf8_lossy(&self.masked).into_owned(),
            comment: String::from_utf8_lossy(&self.comment).trim().to_owned(),
        };
        *self = LineBuilder::default();
        line
    }

    fn push_code(&mut self, byte: u8) {
        self.code.push(byte);
        self.masked.push(byte);
        if !byte.is_ascii_whitespace() {
            self.has_code = true;
        }
    }

    fn push_code_str(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.push_code(byte);
        }
    }

    fn push_string_content(&mut self, byte: u8) {
        self.code.push(byte);
        self.masked.push(if byte == b'\t' { b'\t' } else { b' ' });
        if !byte.is_ascii_whitespace() {
            self.has_code = true;
        }
    }

    fn push_comment(&mut self, byte: u8) {
        self.comment.push(byte);
        if !byte.is_ascii_whitespace() {
            self.has_comment = true;
        }
    }
}

enum State {
    Code,
    Block { rule: usize, depth: u32 },
    Str { rule: usize },
    RustRaw { hashes: usize },
}

fn starts_with(bytes: &[u8], index: usize, token: &str) -> bool {
    bytes
        .get(index..index + token.len())
        .is_some_and(|slice| slice == token.as_bytes())
}

fn is_identifier_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

/// Length in bytes of the UTF-8 sequence starting with `byte`.
fn utf8_width(byte: u8) -> usize {
    match byte {
        0x00..=0x7f => 1,
        0xc0..=0xdf => 2,
        0xe0..=0xef => 3,
        0xf0..=0xf7 => 4,
        _ => 1,
    }
}

/// Scans `text` according to `syntax`.
pub fn scan(text: &str, syntax: &Syntax) -> ScannedFile {
    let bytes = text.as_bytes();
    let mut lines = Vec::new();
    let mut line = LineBuilder::default();
    let mut state = State::Code;
    let mut index = 0;
    let mut line_start = 0;

    while index < bytes.len() {
        let byte = bytes[index];
        if byte == b'\n' {
            if let State::Str { rule } = state
                && !syntax.strings[rule].multiline
            {
                state = State::Code;
            }
            lines.push(line.finish());
            index += 1;
            line_start = index;
            continue;
        }
        if byte == b'\r' && bytes.get(index + 1) == Some(&b'\n') {
            index += 1;
            continue;
        }

        match state {
            State::Code => {
                if let Some(rule) = syntax
                    .block_comments
                    .iter()
                    .position(|rule| starts_with(bytes, index, &rule.open))
                {
                    line.has_comment = true;
                    index += syntax.block_comments[rule].open.len();
                    state = State::Block { rule, depth: 1 };
                    continue;
                }
                let boundary = index == line_start || bytes[index - 1].is_ascii_whitespace();
                if let Some(comment) = syntax.line_comments.iter().find(|comment| {
                    starts_with(bytes, index, &comment.token)
                        && (!comment.requires_boundary || boundary)
                }) {
                    line.has_comment = true;
                    index += comment.token.len();
                    while index < bytes.len() && bytes[index] != b'\n' {
                        if bytes[index] != b'\r' {
                            line.push_comment(bytes[index]);
                        }
                        index += 1;
                    }
                    continue;
                }
                let previous_is_identifier =
                    index > line_start && is_identifier_byte(bytes[index - 1]);
                if syntax.rust_raw_strings
                    && !previous_is_identifier
                    && let Some((prefix_len, hashes)) = rust_raw_string_start(bytes, index)
                {
                    line.push_code_str(&bytes[index..index + prefix_len]);
                    index += prefix_len;
                    state = State::RustRaw { hashes };
                    continue;
                }
                if syntax.rust_char_literals && byte == b'\'' {
                    index = scan_rust_quote(bytes, index, &mut line);
                    continue;
                }
                if let Some(rule) = syntax
                    .strings
                    .iter()
                    .position(|rule| starts_with(bytes, index, &rule.open))
                {
                    let open = syntax.strings[rule].open.as_bytes();
                    line.push_code_str(open);
                    index += open.len();
                    state = State::Str { rule };
                    continue;
                }
                line.push_code(byte);
                index += 1;
            }
            State::Block { rule, depth } => {
                let comment = &syntax.block_comments[rule];
                if comment.nested && starts_with(bytes, index, &comment.open) {
                    state = State::Block {
                        rule,
                        depth: depth + 1,
                    };
                    index += comment.open.len();
                } else if starts_with(bytes, index, &comment.close) {
                    index += comment.close.len();
                    state = if depth <= 1 {
                        State::Code
                    } else {
                        State::Block {
                            rule,
                            depth: depth - 1,
                        }
                    };
                } else {
                    line.push_comment(byte);
                    index += 1;
                }
            }
            State::Str { rule } => {
                let string = &syntax.strings[rule];
                if let Some(escape) = string.escape
                    && byte == escape as u8
                    && index + 1 < bytes.len()
                    && bytes[index + 1] != b'\n'
                {
                    let width = 1 + utf8_width(bytes[index + 1]);
                    for &escaped in &bytes[index..(index + width).min(bytes.len())] {
                        line.push_string_content(escaped);
                    }
                    index += width;
                } else if starts_with(bytes, index, &string.close) {
                    line.push_code_str(string.close.as_bytes());
                    index += string.close.len();
                    state = State::Code;
                } else {
                    line.push_string_content(byte);
                    index += 1;
                }
            }
            State::RustRaw { hashes } => {
                if byte == b'"'
                    && bytes.len() >= index + 1 + hashes
                    && bytes[index + 1..index + 1 + hashes]
                        .iter()
                        .all(|&b| b == b'#')
                {
                    line.push_code_str(&bytes[index..index + 1 + hashes]);
                    index += 1 + hashes;
                    state = State::Code;
                } else {
                    line.push_string_content(byte);
                    index += 1;
                }
            }
        }
    }
    if index > line_start || !line.code.is_empty() || !line.comment.is_empty() || line.has_comment {
        lines.push(line.finish());
    }
    ScannedFile { lines }
}

/// Recognizes `r"`, `r#"`, `br##"` and returns (prefix length, number of hashes).
fn rust_raw_string_start(bytes: &[u8], index: usize) -> Option<(usize, usize)> {
    let mut cursor = index;
    if bytes.get(cursor) == Some(&b'b') {
        cursor += 1;
    }
    if bytes.get(cursor) != Some(&b'r') {
        return None;
    }
    cursor += 1;
    let mut hashes = 0;
    while bytes.get(cursor) == Some(&b'#') {
        hashes += 1;
        cursor += 1;
    }
    if bytes.get(cursor) == Some(&b'"') {
        Some((cursor + 1 - index, hashes))
    } else {
        None
    }
}

/// Handles a `'` in Rust: a character literal (`'a'`, `'\n'`, `'\u{1F600}'`) or a
/// lifetime/label (`'a`, `'static`). Returns the index after the consumed bytes.
fn scan_rust_quote(bytes: &[u8], index: usize, line: &mut LineBuilder) -> usize {
    let next = bytes.get(index + 1).copied();
    let closing = match next {
        Some(b'\\') => {
            // Escaped character literal: find the closing quote on the same line.
            let mut cursor = index + 2;
            while cursor < bytes.len() && bytes[cursor] != b'\n' && cursor < index + 16 {
                if bytes[cursor] == b'\'' {
                    break;
                }
                cursor += 1;
            }
            (bytes.get(cursor) == Some(&b'\'')).then_some(cursor)
        }
        Some(byte) if byte != b'\n' && byte != b'\'' => {
            let after = index + 1 + utf8_width(byte);
            (bytes.get(after) == Some(&b'\'')).then_some(after)
        }
        _ => None,
    };
    match closing {
        Some(end) => {
            line.push_code(b'\'');
            for &content in &bytes[index + 1..end] {
                line.push_string_content(content);
            }
            line.push_code(b'\'');
            end + 1
        }
        None => {
            line.push_code(b'\'');
            index + 1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spec::{BlockComment, LineComment, StringRule};

    fn c_like() -> Syntax {
        Syntax {
            line_comments: vec![LineComment {
                token: "//".into(),
                requires_boundary: false,
            }],
            block_comments: vec![BlockComment {
                open: "/*".into(),
                close: "*/".into(),
                nested: false,
            }],
            strings: vec![StringRule::simple("\""), StringRule::simple("'")],
            ..Syntax::default()
        }
    }

    fn rust_like() -> Syntax {
        Syntax {
            block_comments: vec![BlockComment {
                open: "/*".into(),
                close: "*/".into(),
                nested: true,
            }],
            strings: vec![StringRule::multiline("\"")],
            rust_char_literals: true,
            rust_raw_strings: true,
            ..c_like()
        }
    }

    #[test]
    fn classifies_lines() {
        let text = "int a = 1; // trailing\n\n// only comment\n/* block\n   still */ int b;\n";
        let scanned = scan(text, &c_like());
        let kinds: Vec<_> = scanned.lines.iter().map(|l| l.kind).collect();
        assert_eq!(
            kinds,
            vec![
                LineKind::Code,
                LineKind::Blank,
                LineKind::Comment,
                LineKind::Comment,
                LineKind::Code
            ]
        );
        let counts = scanned.line_counts();
        assert_eq!(
            (counts.total, counts.code, counts.comment, counts.blank),
            (5, 2, 2, 1)
        );
        assert_eq!(scanned.lines[0].comment, "trailing");
        assert_eq!(scanned.lines[4].masked.trim(), "int b;");
    }

    #[test]
    fn comment_tokens_inside_strings_are_code() {
        let scanned = scan("let url = \"http://example.com\"; // real\n", &c_like());
        let line = &scanned.lines[0];
        assert_eq!(line.code.trim(), "let url = \"http://example.com\";");
        assert_eq!(line.masked.trim(), "let url = \"                  \";");
        assert_eq!(line.comment, "real");
    }

    #[test]
    fn escaped_quotes_do_not_end_strings() {
        let scanned = scan("s = \"a \\\" // b\"; x\n", &c_like());
        assert_eq!(scanned.lines[0].kind, LineKind::Code);
        assert!(scanned.lines[0].comment.is_empty());
        assert!(scanned.lines[0].masked.ends_with("; x"));
    }

    #[test]
    fn unterminated_single_line_strings_stop_at_newline() {
        let scanned = scan("x = \"oops\ny = 1; // c\n", &c_like());
        assert_eq!(scanned.lines[1].comment, "c");
        assert_eq!(scanned.lines[1].masked.trim(), "y = 1;");
    }

    #[test]
    fn nested_block_comments_in_rust() {
        let text = "/* outer /* inner */ still comment */ fn main() {}\n";
        let scanned = scan(text, &rust_like());
        assert_eq!(scanned.lines[0].kind, LineKind::Code);
        assert_eq!(scanned.lines[0].masked.trim(), "fn main() {}");
    }

    #[test]
    fn rust_lifetimes_and_char_literals() {
        let text = "fn f<'a>(x: &'a str) -> char { let q = '\"'; let e = '\\n'; 'x' }\n";
        let scanned = scan(text, &rust_like());
        let masked = &scanned.lines[0].masked;
        assert!(masked.contains("<'a>"), "{masked}");
        assert!(masked.contains("&'a str"), "{masked}");
        assert!(masked.contains("q = ' '"), "{masked}");
        assert!(masked.ends_with("' ' }"), "{masked}");
    }

    #[test]
    fn rust_raw_strings_hide_quotes() {
        let text = "let s = r#\"a \"quoted\" // not a comment\"#; // yes\n";
        let scanned = scan(text, &rust_like());
        assert_eq!(scanned.lines[0].comment, "yes");
        assert!(!scanned.lines[0].masked.contains("quoted"));
        assert!(scanned.lines[0].code.contains("quoted"));
    }

    #[test]
    fn boundary_comments_require_whitespace() {
        let syntax = Syntax {
            line_comments: vec![LineComment {
                token: "#".into(),
                requires_boundary: true,
            }],
            ..Syntax::default()
        };
        let scanned = scan("echo $# items # count\n# full\n", &syntax);
        assert_eq!(scanned.lines[0].masked.trim(), "echo $# items");
        assert_eq!(scanned.lines[0].comment, "count");
        assert_eq!(scanned.lines[1].kind, LineKind::Comment);
    }

    #[test]
    fn handles_crlf_missing_trailing_newline_and_empty_input() {
        let scanned = scan("a;\r\nb;", &c_like());
        assert_eq!(scanned.lines.len(), 2);
        assert_eq!(scanned.lines[0].code, "a;");
        assert_eq!(scanned.lines[1].code, "b;");
        assert!(scan("", &c_like()).lines.is_empty());
        assert_eq!(scan("\n", &c_like()).lines.len(), 1);
    }

    #[test]
    fn preserves_multibyte_text() {
        let scanned = scan("let s = \"héllo\"; // ünïcode ✓\n", &c_like());
        assert!(scanned.lines[0].code.contains("héllo"));
        assert_eq!(scanned.lines[0].comment, "ünïcode ✓");
    }

    #[test]
    fn multiline_strings_span_lines() {
        let syntax = Syntax {
            line_comments: vec![LineComment {
                token: "#".into(),
                requires_boundary: false,
            }],
            strings: vec![StringRule::multiline("\"\"\""), StringRule::simple("\"")],
            ..Syntax::default()
        };
        let scanned = scan("x = \"\"\"\n# not a comment\n\"\"\"\n# comment\n", &syntax);
        assert_eq!(scanned.lines[1].kind, LineKind::Code);
        assert_eq!(scanned.lines[3].kind, LineKind::Comment);
    }
}
