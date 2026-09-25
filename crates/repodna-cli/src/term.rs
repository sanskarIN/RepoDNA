//! Terminal styling. Color is optional decoration: every colored element also carries a
//! symbol or a word, so meaning never depends on color alone.

use std::io::IsTerminal;

use repodna_core::severity::Severity;

use crate::cli::ColorChoice;

/// Width of a terminal whose size cannot be read.
const DEFAULT_WIDTH: usize = 100;
/// Width used when output is piped to a file or another program.
const PIPED_WIDTH: usize = 160;

/// Styles text for one output stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Style {
    /// Emit ANSI color codes.
    pub color: bool,
}

impl Style {
    /// Decides whether to color output written to standard output or standard error.
    pub fn detect(choice: ColorChoice, stderr: bool) -> Self {
        let terminal = if stderr {
            std::io::stderr().is_terminal()
        } else {
            std::io::stdout().is_terminal()
        };
        let no_color = std::env::var_os("NO_COLOR").is_some_and(|value| !value.is_empty());
        let dumb = std::env::var("TERM").is_ok_and(|term| term == "dumb");
        Self {
            color: match choice {
                ColorChoice::Always => true,
                ColorChoice::Never => false,
                ColorChoice::Auto => terminal && !no_color && !dumb,
            },
        }
    }

    /// No styling.
    pub const fn plain() -> Self {
        Self { color: false }
    }

    fn paint(self, code: &str, text: &str) -> String {
        if self.color && !text.is_empty() {
            format!("\u{1b}[{code}m{text}\u{1b}[0m")
        } else {
            text.to_owned()
        }
    }

    /// Bold text.
    pub fn bold(self, text: &str) -> String {
        self.paint("1", text)
    }

    /// Dimmed text.
    pub fn dim(self, text: &str) -> String {
        self.paint("2", text)
    }

    /// Green text (success).
    pub fn green(self, text: &str) -> String {
        self.paint("32", text)
    }

    /// Yellow text (caution).
    pub fn yellow(self, text: &str) -> String {
        self.paint("33", text)
    }

    /// Red text (failure).
    pub fn red(self, text: &str) -> String {
        self.paint("31", text)
    }

    /// Cyan text (code and paths).
    pub fn cyan(self, text: &str) -> String {
        self.paint("36", text)
    }

    /// A severity label such as `[warning]`, colored by severity.
    pub fn severity(self, severity: Severity) -> String {
        let label = format!("[{}]", severity.label().to_ascii_lowercase());
        match severity {
            Severity::Critical => self.paint("1;31", &label),
            Severity::Warning => self.paint("33", &label),
            Severity::Attention => self.paint("36", &label),
            Severity::Info => self.paint("2", &label),
        }
    }
}

/// Width for wrapping: `COLUMNS` when set; otherwise the terminal's width when standard
/// output is a terminal, or a generous 160 columns when it is piped. Clamped to 60–240.
pub fn width() -> usize {
    let configured = std::env::var("COLUMNS")
        .ok()
        .and_then(|value| value.trim().parse::<usize>().ok());
    let detected = || {
        std::io::stdout()
            .is_terminal()
            .then(|| terminal_size::terminal_size().map(|(w, _)| usize::from(w.0)))
            .flatten()
    };
    configured
        .or_else(detected)
        .unwrap_or(if std::io::stdout().is_terminal() {
            DEFAULT_WIDTH
        } else {
            PIPED_WIDTH
        })
        .clamp(60, 240)
}

/// Number of characters as displayed (ANSI codes excluded).
pub fn display_width(text: &str) -> usize {
    let mut width = 0;
    let mut in_escape = false;
    for c in text.chars() {
        if in_escape {
            if c.is_ascii_alphabetic() {
                in_escape = false;
            }
        } else if c == '\u{1b}' {
            in_escape = true;
        } else {
            width += 1;
        }
    }
    width
}

/// Wraps plain text to `width` columns; continuation lines start with `indent`.
pub fn wrap(text: &str, width: usize, first: &str, indent: &str) -> String {
    let mut out = String::new();
    let mut line = first.to_owned();
    let mut line_width = display_width(first);
    let mut empty = true;
    for word in text.split_whitespace() {
        let word_width = display_width(word);
        if !empty && line_width + 1 + word_width > width {
            out.push_str(line.trim_end());
            out.push('\n');
            line = indent.to_owned();
            line_width = display_width(indent);
            empty = true;
        }
        if !empty {
            line.push(' ');
            line_width += 1;
        }
        line.push_str(word);
        line_width += word_width;
        empty = false;
    }
    out.push_str(line.trim_end());
    out
}

/// Formats an integer with thousands separators.
pub fn thousands(value: u64) -> String {
    repodna_report::text::thousands(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn styles_only_when_enabled() {
        let plain = Style::plain();
        assert_eq!(plain.bold("x"), "x");
        assert_eq!(plain.severity(Severity::Warning), "[warning]");
        let color = Style { color: true };
        assert_eq!(color.red("x"), "\u{1b}[31mx\u{1b}[0m");
        assert_eq!(display_width(&color.red("abc")), 3);
        assert_eq!(Style::detect(ColorChoice::Never, false), Style::plain());
        assert!(Style::detect(ColorChoice::Always, false).color);
    }

    #[test]
    fn wraps_with_hanging_indent() {
        assert_eq!(
            wrap("one two three four", 9, "- ", "  "),
            "- one two\n  three\n  four"
        );
        assert_eq!(wrap("", 10, "> ", "  "), ">");
        assert_eq!(thousands(1234567), "1,234,567");
    }
}
