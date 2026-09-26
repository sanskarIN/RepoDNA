//! Formatting and escaping helpers shared by the report formats.

use repodna_core::time::Timestamp;

/// Formats an integer with thousands separators, e.g. `12,345`.
pub fn thousands(value: u64) -> String {
    let digits = value.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            out.push(',');
        }
        out.push(digit);
    }
    out
}

/// A number with thousands separators and the matching noun form: `1 file`, `12,345 files`.
pub fn counted(value: u64, one: &str, many: &str) -> String {
    format!(
        "{} {}",
        thousands(value),
        if value == 1 { one } else { many }
    )
}

/// Formats a 0–1 ratio as a whole percentage, e.g. `86%`; a share that would round to zero
/// but is not zero is `<1%`.
pub fn whole_percent(ratio: f64) -> String {
    let value = (ratio * 100.0).round();
    if value == 0.0 && ratio > 0.0 {
        "<1%".to_owned()
    } else {
        format!("{value:.0}%")
    }
}

/// Formats a 0–1 ratio as a percentage with one decimal, e.g. `12.5%`.
pub fn percent(ratio: f64) -> String {
    if !ratio.is_finite() {
        return "–".to_owned();
    }
    let text = format!("{:.1}%", ratio * 100.0);
    // A share too small to show is still not zero.
    if ratio > 0.0 && text == "0.0%" {
        "<0.1%".to_owned()
    } else {
        text
    }
}

/// Formats a byte count with a binary unit, e.g. `1.5 MiB`.
pub fn bytes(value: u64) -> String {
    const UNITS: [&str; 5] = ["KiB", "MiB", "GiB", "TiB", "PiB"];
    if value < 1024 {
        return format!("{value} B");
    }
    let mut amount = value as f64;
    let mut unit = UNITS[0];
    for candidate in UNITS {
        amount /= 1024.0;
        unit = candidate;
        if amount < 1024.0 {
            break;
        }
    }
    format!("{amount:.1} {unit}")
}

/// Formats a duration given in days as years or days, e.g. `3.2 years` or `45 days`.
pub fn span(days: i64) -> String {
    if days >= 365 {
        format!("{:.1} years", days as f64 / 365.25)
    } else if days <= 0 {
        "less than a day".to_owned()
    } else if days == 1 {
        "1 day".to_owned()
    } else {
        format!("{days} days")
    }
}

/// Formats a date as `YYYY-MM-DD`.
pub fn date(timestamp: Timestamp) -> String {
    timestamp.date_string()
}

/// Formats a number compactly: integers without decimals, others with up to two.
pub fn number(value: f64) -> String {
    repodna_core::evidence::format_number(value)
}

/// Escapes text for HTML and XML (element content and quoted attribute values).
pub fn escape_html(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(character),
        }
    }
    out
}

/// Escapes text for a Markdown table cell or inline text: pipes, backticks, emphasis
/// characters, and line breaks cannot break the surrounding structure.
pub fn escape_markdown(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            '|' | '`' | '*' | '_' | '[' | ']' | '<' | '>' | '#' | '\\' => {
                out.push('\\');
                out.push(character);
            }
            '\n' | '\r' => out.push(' '),
            _ => out.push(character),
        }
    }
    out
}

/// Wraps a path or identifier in a Markdown code span, choosing a fence that cannot clash
/// with backticks inside the text.
pub fn code(text: &str) -> String {
    let text = text.replace(['\n', '\r'], " ");
    let mut fence = "`".to_owned();
    while text.contains(fence.as_str()) {
        fence.push('`');
    }
    let padding = if text.starts_with('`') || text.ends_with('`') {
        " "
    } else {
        ""
    };
    format!("{fence}{padding}{text}{padding}{fence}")
}

/// Joins items as a readable list: `a`, `a and b`, `a, b, and c`.
pub fn list(items: &[String]) -> String {
    match items {
        [] => String::new(),
        [one] => one.clone(),
        [first, second] => format!("{first} and {second}"),
        [rest @ .., last] => format!("{}, and {last}", rest.join(", ")),
    }
}

/// Makes a CSV field safe: quotes it when needed and neutralizes spreadsheet formulas.
pub fn csv_field(text: &str) -> String {
    let guarded = if text.starts_with(['=', '+', '-', '@', '\t', '\r']) {
        format!("'{text}")
    } else {
        text.to_owned()
    };
    if guarded.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", guarded.replace('"', "\"\""))
    } else {
        guarded
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_numbers() {
        assert_eq!(thousands(0), "0");
        assert_eq!(thousands(999), "999");
        assert_eq!(thousands(1_000), "1,000");
        assert_eq!(thousands(1_234_567), "1,234,567");
        assert_eq!(percent(0.125), "12.5%");
        assert_eq!(percent(f64::NAN), "–");
        assert_eq!(bytes(512), "512 B");
        assert_eq!(bytes(1536), "1.5 KiB");
        assert_eq!(bytes(5 * 1024 * 1024 * 1024), "5.0 GiB");
        assert_eq!(span(0), "less than a day");
        assert_eq!(span(1), "1 day");
        assert_eq!(span(45), "45 days");
        assert_eq!(span(730), "2.0 years");
        assert_eq!(number(3.0), "3");
        assert_eq!(number(2.456), "2.46");
    }

    #[test]
    fn escapes_markup() {
        assert_eq!(
            escape_html("<a href=\"x\">'&'</a>"),
            "&lt;a href=&quot;x&quot;&gt;&#39;&amp;&#39;&lt;/a&gt;"
        );
        assert_eq!(escape_markdown("a|b*c_d\ne"), "a\\|b\\*c\\_d e");
        assert_eq!(code("src/main.rs"), "`src/main.rs`");
        assert_eq!(code("a`b"), "``a`b``");
        assert_eq!(code("`x"), "`` `x ``");
    }

    #[test]
    fn builds_lists_and_csv_fields() {
        let items: Vec<String> = ["a", "b", "c"].iter().map(|s| (*s).to_owned()).collect();
        assert_eq!(list(&items[..1]), "a");
        assert_eq!(list(&items[..2]), "a and b");
        assert_eq!(list(&items), "a, b, and c");
        assert_eq!(csv_field("plain"), "plain");
        assert_eq!(csv_field("a,b"), "\"a,b\"");
        assert_eq!(csv_field("say \"hi\""), "\"say \"\"hi\"\"\"");
        assert_eq!(csv_field("=SUM(A1)"), "'=SUM(A1)");
    }

    #[test]
    fn whole_percentages_show_small_shares() {
        assert_eq!(whole_percent(0.86), "86%");
        assert_eq!(whole_percent(0.004), "<1%");
        assert_eq!(whole_percent(0.0), "0%");
        assert_eq!(whole_percent(0.005), "1%");
        assert_eq!(percent(0.8674), "86.7%");
        assert_eq!(percent(0.0004), "<0.1%");
        assert_eq!(percent(0.0), "0.0%");
    }
}
