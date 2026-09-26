//! Normalized token streams for duplication and similarity detection.
//!
//! Tokens come from masked lines, so comments and whitespace never affect matching and
//! string or numeric literals are normalized: two blocks that differ only in literal
//! values or formatting produce the same token sequence.

use crate::scanner::{LineKind, ScannedLine};

/// A hashed token and the line it came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Token {
    /// FNV-1a hash of the normalized token text.
    pub hash: u64,
    /// Line (1-based).
    pub line: u32,
}

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// FNV-1a 64-bit hash: small, fast, and stable across platforms and releases.
pub fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(FNV_OFFSET, |hash, &byte| {
        (hash ^ u64::from(byte)).wrapping_mul(FNV_PRIME)
    })
}

/// Tokenizes code lines into normalized, hashed tokens.
pub fn tokenize(lines: &[ScannedLine]) -> Vec<Token> {
    let string_hash = fnv1a(b"<str>");
    let number_hash = fnv1a(b"<num>");
    let mut tokens = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        if line.kind != LineKind::Code {
            continue;
        }
        let line_number = u32::try_from(index + 1).unwrap_or(u32::MAX);
        let bytes = line.masked.as_bytes();
        let mut position = 0;
        while position < bytes.len() {
            let byte = bytes[position];
            if byte.is_ascii_whitespace() {
                position += 1;
            } else if byte.is_ascii_alphabetic() || byte == b'_' || byte == b'$' || byte >= 0x80 {
                let start = position;
                while position < bytes.len()
                    && (bytes[position].is_ascii_alphanumeric()
                        || bytes[position] == b'_'
                        || bytes[position] == b'$'
                        || bytes[position] >= 0x80)
                {
                    position += 1;
                }
                tokens.push(Token {
                    hash: fnv1a(&bytes[start..position]),
                    line: line_number,
                });
            } else if byte.is_ascii_digit() {
                while position < bytes.len()
                    && (bytes[position].is_ascii_alphanumeric()
                        || bytes[position] == b'.'
                        || bytes[position] == b'_')
                {
                    position += 1;
                }
                tokens.push(Token {
                    hash: number_hash,
                    line: line_number,
                });
            } else if matches!(byte, b'"' | b'\'' | b'`') {
                // A masked string literal: its delimiters remain and its contents are blanks.
                tokens.push(Token {
                    hash: string_hash,
                    line: line_number,
                });
                position += 1;
            } else {
                tokens.push(Token {
                    hash: fnv1a(&bytes[position..position + 1]),
                    line: line_number,
                });
                position += 1;
            }
        }
    }
    tokens
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::LanguageRegistry;
    use crate::scanner::scan;

    fn hashes(source: &str) -> Vec<u64> {
        let spec = LanguageRegistry::builtin().get("javascript").unwrap();
        tokenize(&scan(source, &spec.syntax).lines)
            .into_iter()
            .map(|t| t.hash)
            .collect()
    }

    #[test]
    fn ignores_formatting_comments_and_literal_values() {
        let a = hashes("const total = price * 3; // tax\nlog(\"done\");\n");
        let b = hashes("const   total=price*42;\n/* note */ log('finished');\n");
        assert_eq!(a, b);
    }

    #[test]
    fn identifiers_are_significant() {
        assert_ne!(hashes("add(a, b);"), hashes("sub(a, b);"));
    }

    #[test]
    fn records_line_numbers() {
        let spec = LanguageRegistry::builtin().get("javascript").unwrap();
        let tokens = tokenize(&scan("a;\n\nb;\n", &spec.syntax).lines);
        let lines: Vec<u32> = tokens.iter().map(|t| t.line).collect();
        assert_eq!(lines, vec![1, 1, 3, 3]);
    }

    #[test]
    fn fnv_matches_reference_values() {
        assert_eq!(fnv1a(b""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(fnv1a(b"a"), 0xaf63_dc4c_8601_ec8c);
    }
}
