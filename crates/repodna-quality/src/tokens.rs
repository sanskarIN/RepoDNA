//! Compact token streams for duplication and similarity detection.

use repodna_parser::{FileAnalysis, Token};

/// Normalized token hashes of one file with the line of each token.
///
/// Hashes are folded to 32 bits to halve memory use; matches are always verified on the
/// full window, and a 32-bit collision across a whole window of tokens is negligible.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TokenStream {
    /// Token hashes.
    pub hashes: Vec<u32>,
    /// Line (1-based) of each token.
    pub lines: Vec<u32>,
}

/// Folds a 64-bit hash into 32 bits.
fn fold(hash: u64) -> u32 {
    // Truncation is the point: both halves are mixed into the result.
    #[allow(clippy::cast_possible_truncation)]
    let folded = (hash ^ (hash >> 32)) as u32;
    folded
}

impl TokenStream {
    /// Builds a stream from parser tokens, dropping tokens on lines for which `skip`
    /// returns `true`.
    pub fn from_tokens(tokens: &[Token], skip: impl Fn(u32) -> bool) -> Self {
        let mut stream = Self::default();
        for token in tokens.iter().filter(|token| !skip(token.line)) {
            stream.hashes.push(fold(token.hash));
            stream.lines.push(token.line);
        }
        stream
    }

    /// Builds a stream for a file, dropping import statements: import blocks repeat across
    /// files by design and would otherwise dominate duplication results.
    pub fn for_file(tokens: &[Token], analysis: Option<&FileAnalysis>) -> Self {
        let mut import_lines: Vec<u32> = analysis
            .map(|analysis| analysis.imports.iter().map(|import| import.line).collect())
            .unwrap_or_default();
        import_lines.sort_unstable();
        import_lines.dedup();
        Self::from_tokens(tokens, |line| import_lines.binary_search(&line).is_ok())
    }

    /// Number of tokens.
    pub fn len(&self) -> usize {
        self.hashes.len()
    }

    /// Returns `true` when the stream has no tokens.
    pub fn is_empty(&self) -> bool {
        self.hashes.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use repodna_parser::{LanguageRegistry, analyze_and_tokenize};

    #[test]
    fn drops_import_lines() {
        let spec = LanguageRegistry::builtin().get("python").unwrap();
        let (analysis, tokens) =
            analyze_and_tokenize(spec, "import os\nfrom a import b\nx = os.path\n");
        let stream = TokenStream::for_file(&tokens, Some(&analysis));
        assert!(stream.lines.iter().all(|&line| line == 3));
        assert_eq!(stream.len(), stream.lines.len());
        assert!(!stream.is_empty());
        let everything = TokenStream::from_tokens(&tokens, |_| false);
        assert_eq!(everything.len(), tokens.len());
        assert_eq!(fold(0x0000_0001_0000_0001), 0);
    }
}
