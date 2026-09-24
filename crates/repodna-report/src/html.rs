//! Renders the document model as a single self-contained HTML page.
//!
//! The page has no external resources: styles, charts, and the small optional script are
//! inline, and a Content Security Policy allows only those exact inline blocks (by hash),
//! so a hosted report cannot load or send anything. Everything is readable without
//! JavaScript; the script only adds a severity filter to the findings.

use repodna_core::config::ReportTheme;
use repodna_core::finding::Finding;
use repodna_core::hash::sha256;
use repodna_core::severity::Severity;

use crate::doc::{Block, Blocks, Inline, NoteKind, Rich, Table};
use crate::palette::{DARK, LIGHT, css_variables};
use crate::text::escape_html as esc;

/// Page metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Page {
    /// Document title.
    pub title: String,
    /// Theme.
    pub theme: ReportTheme,
    /// Footer text (HTML-escaped by the renderer), e.g. the generator line.
    pub footer: Vec<Inline>,
    /// Machine-readable signature (JSON).
    pub signature: String,
    /// RepoDNA version for the generator meta tag.
    pub version: String,
}

const SCRIPT: &str = r#"(function(){var list=document.querySelectorAll('.finding');if(!list.length)return;var bar=document.getElementById('finding-filter');if(!bar)return;bar.hidden=false;var buttons=bar.querySelectorAll('button');function apply(level){list.forEach(function(el){el.hidden=level!=='all'&&el.getAttribute('data-severity')!==level;});buttons.forEach(function(b){b.setAttribute('aria-pressed',String(b.getAttribute('data-level')===level));});}buttons.forEach(function(b){b.addEventListener('click',function(){apply(b.getAttribute('data-level'));});});})();"#;

const BASE_CSS: &str = r#"
*,*::before,*::after{box-sizing:border-box}
html{-webkit-text-size-adjust:100%}
body{margin:0;background:var(--page);color:var(--ink);font:15px/1.55 var(--font);}
a{color:var(--link)}
a:focus-visible,button:focus-visible{outline:2px solid var(--link);outline-offset:2px}
.skip{position:absolute;left:-999px;top:0;background:var(--surface);padding:8px 12px;z-index:10}
.skip:focus{left:8px}
.layout{display:grid;grid-template-columns:250px minmax(0,1fr);max-width:1400px;margin:0 auto}
.toc{position:sticky;top:0;align-self:start;max-height:100vh;overflow:auto;padding:28px 18px;border-right:1px solid var(--grid)}
.toc .brand{font-weight:700;font-size:18px;margin:0 0 14px}
.toc .brand span{color:var(--muted);font-weight:400;font-size:13px;display:block}
.toc ol{list-style:none;margin:0;padding:0}
.toc li a{display:block;padding:5px 8px;border-radius:6px;color:var(--ink-2);text-decoration:none;font-size:14px}
.toc li a:hover{background:var(--hover);color:var(--ink)}
main{padding:32px 44px 64px;min-width:0}
section{background:var(--surface);border:1px solid var(--border);border-radius:14px;padding:26px 30px;margin:0 0 22px}
h1{font-size:32px;line-height:1.2;margin:0 0 10px}
h2{font-size:22px;margin:0 0 14px}
h3{font-size:17px;margin:26px 0 10px}
h4{font-size:15px;margin:20px 0 8px}
p{margin:0 0 12px}
code{font:13px/1.4 var(--mono);background:var(--code-bg);padding:1px 5px;border-radius:5px;overflow-wrap:anywhere}
pre{background:var(--code-bg);padding:12px 14px;border-radius:8px;overflow:auto;font:13px/1.45 var(--mono)}
pre code{background:none;padding:0}
ul{margin:0 0 12px;padding-left:22px}
li{margin:3px 0}
.stats{display:grid;grid-template-columns:repeat(auto-fill,minmax(165px,1fr));gap:10px;margin:0 0 16px}
.stats div{border:1px solid var(--border);border-radius:10px;padding:10px 14px;background:var(--page)}
.stats dt{color:var(--muted);font-size:13px}
.stats dd{margin:2px 0 0;font-size:20px;font-weight:650;overflow-wrap:break-word}
.nw{white-space:nowrap}
.table-wrap{overflow-x:auto;margin:0 0 14px;border:1px solid var(--border);border-radius:10px}
table{border-collapse:collapse;width:100%;font-size:14px}
th,td{padding:7px 11px;text-align:left;vertical-align:top;border-bottom:1px solid var(--grid)}
th{font-weight:600;color:var(--ink-2);background:var(--page);white-space:nowrap}
tr:last-child td{border-bottom:none}
td.num,th.num{text-align:right;font-variant-numeric:tabular-nums;white-space:nowrap}
.omitted{color:var(--muted);font-size:13px;margin:-6px 0 14px}
.note{border-left:3px solid var(--link);background:var(--page);padding:9px 13px;border-radius:0 8px 8px 0;color:var(--ink-2);font-size:14px}
.note.caution{border-left-color:var(--muted)}
figure{margin:0 0 16px}
figure .chart{overflow-x:auto}
figcaption{color:var(--muted);font-size:13px;margin-top:6px}
.card svg{width:100%;height:auto;max-width:900px;display:block}
.finding{border:1px solid var(--border);border-radius:12px;padding:16px 20px;margin:0 0 14px;background:var(--page)}
.finding h4{margin:6px 0 6px;font-size:16px}
.finding .meta{color:var(--muted);font-size:13px;margin-bottom:10px}
.finding dl{margin:0 0 8px}
.finding dt{font-weight:600;font-size:14px;margin-top:8px}
.finding dd{margin:2px 0 0}
.finding.suppressed{opacity:.75}
.sev{display:inline-flex;align-items:center;gap:6px;font-size:13px;font-weight:600;color:var(--ink-2)}
.sev::before{content:"";width:10px;height:10px;border-radius:50%;background:var(--sev)}
.sev-critical{--sev:#d03b3b}.sev-warning{--sev:#ec835a}.sev-attention{--sev:#fab219}.sev-info{--sev:#898781}
.filter{display:flex;flex-wrap:wrap;gap:8px;margin:0 0 14px}
.filter button{font:inherit;font-size:13px;border:1px solid var(--border);background:var(--page);color:var(--ink);border-radius:999px;padding:4px 12px;cursor:pointer}
.filter button[aria-pressed="true"]{background:var(--ink);color:var(--surface)}
.report-footer{color:var(--muted);font-size:13px;text-align:center;margin-top:28px}
.viz{max-width:100%;height:auto;font-family:var(--font);font-size:12px}
.viz text{fill:var(--ink)}
.viz .viz-muted{fill:var(--muted);font-size:11px}
.viz .viz-value{fill:var(--ink-2);font-weight:600}
.viz .viz-grid{stroke:var(--grid);stroke-width:1}
.viz .viz-grid-shape{fill:none;stroke:var(--grid);stroke-width:1}
.viz .viz-axis{stroke:var(--axis);stroke-width:1}
.viz .viz-s1{fill:var(--series-1)}.viz .viz-s2{fill:var(--series-2)}.viz .viz-s3{fill:var(--series-3)}.viz .viz-s4{fill:var(--series-4)}
.viz .viz-s5{fill:var(--series-5)}.viz .viz-s6{fill:var(--series-6)}.viz .viz-s7{fill:var(--series-7)}.viz .viz-s8{fill:var(--series-8)}
.viz .viz-other{fill:var(--other)}
.viz .viz-q0{fill:var(--seq-0)}.viz .viz-q1{fill:var(--seq-1)}.viz .viz-q2{fill:var(--seq-2)}.viz .viz-q3{fill:var(--seq-3)}
.viz .viz-q4{fill:var(--seq-4)}.viz .viz-q5{fill:var(--seq-5)}.viz .viz-q6{fill:var(--seq-6)}
.viz .viz-line{fill:none;stroke:var(--series-1);stroke-width:2;stroke-linejoin:round;stroke-linecap:round}
.viz .viz-wash{fill:var(--series-1);fill-opacity:.1}
.viz .viz-outline{stroke:var(--series-1);stroke-width:2;stroke-linejoin:round}
.viz .viz-dot{fill:var(--series-1)}
.viz .viz-ring{stroke:var(--surface);stroke-width:2}
.viz .viz-hit{fill:transparent}
.viz .viz-tile{fill:var(--seq-2)}
.viz .viz-tile-label{font-weight:600}
.viz .viz-tile-value{fill:var(--ink-2);font-size:11px}
.viz .viz-node rect{fill:var(--surface);stroke:var(--axis)}
.viz .viz-edge{fill:none;stroke:var(--muted);stroke-width:1}
.viz .viz-arrowhead{fill:var(--muted)}
.viz rect:hover,.viz path:hover,.viz circle:hover{opacity:.82}
@media (max-width:900px){.layout{display:block}.toc{position:static;max-height:none;border-right:none;border-bottom:1px solid var(--grid)}.toc ol{columns:2}main{padding:18px 16px 40px}section{padding:18px 16px}}
@media print{body{background:#fff}.toc,.skip,.filter{display:none!important}.layout{display:block}main{padding:0}section{border:none;padding:0;margin:0 0 18px;break-inside:auto}.finding,figure,tr{break-inside:avoid}h2,h3{break-after:avoid}a{color:inherit}.finding[hidden]{display:block!important}}
@media (prefers-reduced-motion:reduce){*{transition:none!important}}
"#;

fn theme_css(theme: ReportTheme) -> String {
    let (palette, extra) = match theme {
        ReportTheme::Professional => (&LIGHT, ""),
        ReportTheme::Minimal => (
            &LIGHT,
            "section{border:none;border-radius:0;padding:26px 0;border-bottom:1px solid var(--grid)}.stats div{background:none}",
        ),
        ReportTheme::Technical => (
            &LIGHT,
            "body{font-size:14px}table{font-size:13px}td,th{padding:5px 9px}section{border-radius:6px}h2,h3{font-family:var(--mono)}",
        ),
        ReportTheme::Dark => (&DARK, ""),
    };
    let dark = theme == ReportTheme::Dark;
    format!(
        ":root{{{vars}--link:{link};--border:{border};--hover:{hover};--code-bg:{code};--font:system-ui,-apple-system,'Segoe UI','DejaVu Sans',sans-serif;--mono:ui-monospace,SFMono-Regular,Menlo,Consolas,'DejaVu Sans Mono',monospace;color-scheme:{scheme}}}{extra}",
        vars = css_variables(palette),
        link = if dark { "#6da7ec" } else { "#1c5cab" },
        border = if dark {
            "rgba(255,255,255,.10)"
        } else {
            "rgba(11,11,11,.10)"
        },
        hover = if dark { "#262625" } else { "#f0efec" },
        code = if dark { "#262625" } else { "#f0efec" },
        scheme = if dark { "dark" } else { "light" },
    )
}

fn base64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        out.push(char::from(TABLE[(n >> 18) as usize & 63]));
        out.push(char::from(TABLE[(n >> 12) as usize & 63]));
        out.push(if chunk.len() > 1 {
            char::from(TABLE[(n >> 6) as usize & 63])
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            char::from(TABLE[n as usize & 63])
        } else {
            '='
        });
    }
    out
}

fn csp_hash(content: &str) -> String {
    format!("'sha256-{}'", base64(&sha256(content.as_bytes())))
}

/// Renders inline content.
pub fn inline(rich: &Rich) -> String {
    rich.iter()
        .map(|part| match part {
            Inline::Text(text) => esc(text),
            Inline::Strong(text) => format!("<strong>{}</strong>", esc(text)),
            Inline::Code(text) if text.chars().count() <= 16 => {
                format!(r#"<code class="nw">{}</code>"#, esc(text))
            }
            Inline::Code(text) => format!("<code>{}</code>", esc(text)),
            Inline::Link { text, url } => {
                if url.starts_with("https://") {
                    format!(
                        r#"<a href="{}" rel="noopener noreferrer">{}</a>"#,
                        esc(url),
                        esc(text)
                    )
                } else {
                    esc(text)
                }
            }
        })
        .collect()
}

fn table(out: &mut String, table: &Table) {
    out.push_str(r#"<div class="table-wrap"><table><thead><tr>"#);
    for (header, numeric) in table.headers.iter().zip(&table.numeric) {
        out.push_str(&format!(
            r#"<th scope="col"{}>{}</th>"#,
            if *numeric { r#" class="num""# } else { "" },
            esc(header)
        ));
    }
    out.push_str("</tr></thead><tbody>");
    for row in &table.rows {
        out.push_str("<tr>");
        for (index, cell) in row.iter().enumerate() {
            let numeric = table.numeric.get(index).copied().unwrap_or(false);
            let class = if numeric {
                r#" class="num""#
            } else if is_date(cell) {
                r#" class="nw""#
            } else {
                ""
            };
            out.push_str(&format!("<td{class}>{}</td>", inline(cell)));
        }
        out.push_str("</tr>");
    }
    out.push_str("</tbody></table></div>");
    if table.omitted > 0 {
        out.push_str(&format!(
            r#"<p class="omitted">{} more not shown; the JSON artifact has the complete list.</p>"#,
            table.omitted
        ));
    }
}

/// `true` for a cell holding only a `YYYY-MM-DD` date, which should not wrap.
fn is_date(cell: &Rich) -> bool {
    match cell.as_slice() {
        [Inline::Text(text)] => {
            let bytes = text.as_bytes();
            bytes.len() == 10
                && bytes[4] == b'-'
                && bytes[7] == b'-'
                && bytes
                    .iter()
                    .enumerate()
                    .all(|(i, b)| i == 4 || i == 7 || b.is_ascii_digit())
        }
        _ => false,
    }
}

fn severity_class(severity: Severity) -> &'static str {
    match severity {
        Severity::Critical => "critical",
        Severity::Warning => "warning",
        Severity::Attention => "attention",
        Severity::Info => "info",
    }
}

fn finding(out: &mut String, finding: &Finding) {
    let class = severity_class(finding.severity);
    out.push_str(&format!(
        r#"<article class="finding{}" data-severity="{class}" id="finding-{}"><span class="sev sev-{class}">{}</span><h4>{}</h4><p class="meta">Rule <code>{}</code> · {} confidence{}</p><dl>"#,
        if finding.is_suppressed() { " suppressed" } else { "" },
        esc(&finding.id.replace(':', "-")),
        finding.severity.label(),
        esc(&finding.title),
        esc(&finding.rule),
        finding.confidence.label(),
        finding.suppressed.as_ref().map_or_else(String::new, |s| format!(
            " · <strong>Suppressed</strong> ({}: {})",
            esc(&s.source),
            esc(&s.reason)
        ))
    ));
    for (label, text) in [
        ("What was observed", &finding.summary),
        ("Why it may matter", &finding.rationale),
        ("How it was determined", &finding.method),
    ] {
        if !text.is_empty() {
            out.push_str(&format!("<dt>{label}</dt><dd>{}</dd>", esc(text)));
        }
    }
    let lists: [(&str, Vec<String>); 3] = [
        (
            "Evidence",
            finding
                .evidence
                .iter()
                .map(|e| format!("<code>{}</code>", esc(&e.describe())))
                .collect(),
        ),
        (
            "Limitations",
            finding.limitations.iter().map(|t| esc(t)).collect(),
        ),
        (
            "Next steps",
            finding.next_steps.iter().map(|t| esc(t)).collect(),
        ),
    ];
    for (label, items) in lists {
        if !items.is_empty() {
            out.push_str(&format!(
                "<dt>{label}</dt><dd><ul>{}</ul></dd>",
                items
                    .iter()
                    .map(|item| format!("<li>{item}</li>"))
                    .collect::<String>()
            ));
        }
    }
    out.push_str("</dl></article>");
}

const FILTER: &str = r#"<div class="filter" id="finding-filter" role="group" aria-label="Show findings by severity" hidden><button type="button" data-level="all" aria-pressed="true">All</button><button type="button" data-level="critical" aria-pressed="false">Critical</button><button type="button" data-level="warning" aria-pressed="false">Warning</button><button type="button" data-level="attention" aria-pressed="false">Attention</button></div>"#;

/// Renders blocks as the body of the page, wrapping each level-1 or level-2 heading and
/// its content in a section.
fn body(blocks: &Blocks) -> (String, Vec<(String, String)>) {
    let mut out = String::new();
    let mut toc = Vec::new();
    let mut open = false;
    let mut filter_added = false;
    for block in &blocks.0 {
        match block {
            Block::Heading { level, text, id } if *level <= 2 => {
                if open {
                    out.push_str("</section>");
                }
                let anchor = id.clone().unwrap_or_else(|| format!("s{}", toc.len() + 1));
                out.push_str(&format!(
                    r#"<section id="{}" aria-labelledby="{}-title"><h{level} id="{}-title">{}</h{level}>"#,
                    esc(&anchor),
                    esc(&anchor),
                    esc(&anchor),
                    esc(text)
                ));
                toc.push((anchor, text.clone()));
                open = true;
            }
            Block::Heading { level, text, id } => {
                let id_attr = id
                    .as_ref()
                    .map_or_else(String::new, |id| format!(r#" id="{}""#, esc(id)));
                out.push_str(&format!("<h{level}{id_attr}>{}</h{level}>", esc(text)));
            }
            Block::Paragraph(rich) => out.push_str(&format!("<p>{}</p>", inline(rich))),
            Block::Note(kind, rich) => {
                let (class, label) = match kind {
                    NoteKind::Info => ("note", "Note"),
                    NoteKind::Caution => ("note caution", "Not available"),
                };
                out.push_str(&format!(
                    r#"<p class="{class}" role="note"><strong>{label}:</strong> {}</p>"#,
                    inline(rich)
                ));
            }
            Block::List(items) => {
                out.push_str("<ul>");
                for item in items {
                    out.push_str(&format!("<li>{}</li>", inline(item)));
                }
                out.push_str("</ul>");
            }
            Block::Table(t) => table(&mut out, t),
            Block::Stats(stats) => {
                out.push_str(r#"<dl class="stats">"#);
                for (label, value) in stats {
                    out.push_str(&format!(
                        "<div><dt>{}</dt><dd>{}</dd></div>",
                        esc(label),
                        esc(value)
                    ));
                }
                out.push_str("</dl>");
            }
            Block::Figure { svg, caption } => {
                let class = if caption == "Project DNA card" {
                    "chart card"
                } else {
                    "chart"
                };
                out.push_str(&format!(
                    r#"<figure><div class="{class}">{svg}</div><figcaption>{}</figcaption></figure>"#,
                    esc(caption)
                ));
            }
            Block::Finding(f) => {
                if !filter_added {
                    out.push_str(FILTER);
                    filter_added = true;
                }
                finding(&mut out, f);
            }
            Block::Preformatted(text) => {
                out.push_str(&format!("<pre><code>{}</code></pre>", esc(text)));
            }
        }
    }
    if open {
        out.push_str("</section>");
    }
    (out, toc)
}

/// Renders a complete HTML page.
pub fn render(blocks: &Blocks, page: &Page) -> String {
    let (content, toc) = body(blocks);
    let css = format!("{}{}", theme_css(page.theme), BASE_CSS);
    let csp = format!(
        "default-src 'none'; style-src {}; script-src {}; img-src data:; base-uri 'none'; form-action 'none'",
        csp_hash(&css),
        csp_hash(SCRIPT)
    );
    let toc_items: String = toc
        .iter()
        .filter(|(id, _)| id != "cover")
        .map(|(id, title)| format!(r##"<li><a href="#{}">{}</a></li>"##, esc(id), esc(title)))
        .collect();
    format!(
        r##"<!doctype html>
<html lang="en" data-theme="{theme}">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<meta http-equiv="Content-Security-Policy" content="{csp}">
<meta name="generator" content="RepoDNA {version}">
<meta name="robots" content="noindex">
<title>{title}</title>
<style>{css}</style>
<script type="application/json" id="repodna-signature">{signature}</script>
</head>
<body>
<a class="skip" href="#main">Skip to the report</a>
<div class="layout">
<nav class="toc" aria-label="Report sections"><p class="brand">RepoDNA<span>Project DNA report</span></p><ol>{toc_items}</ol></nav>
<main id="main">
{content}
<footer class="report-footer"><p>{footer}</p></footer>
</main>
</div>
<script>{script}</script>
</body>
</html>
"##,
        theme = page.theme.id(),
        csp = esc(&csp),
        version = esc(&page.version),
        title = esc(&page.title),
        signature = page.signature.replace("</", "<\\/"),
        footer = inline(&page.footer),
        script = SCRIPT,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doc::{Blocks, Table, plain};
    use repodna_core::confidence::Confidence;
    use repodna_core::finding::FindingCategory;

    fn page(theme: ReportTheme) -> Page {
        Page {
            title: "widget — Project DNA".into(),
            theme,
            footer: vec![Inline::Text("Generated by RepoDNA".into())],
            signature: r#"{"generatedBy":"RepoDNA","x":"</script>"}"#.into(),
            version: "1.0.0".into(),
        }
    }

    #[test]
    fn encodes_base64() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn renders_a_self_contained_accessible_page() {
        let mut blocks = Blocks::default();
        blocks.heading(1, "widget — Project DNA", Some("cover"));
        blocks.heading(2, "Language map", Some("languages"));
        blocks.text("Text with <script>alert(1)</script>");
        let mut t = Table::new(&["Name", "Files#"]);
        t.row(vec![plain("Rust"), plain("3")]);
        blocks.table(t);
        blocks.figure(Some("<svg class=\"viz\"></svg>".into()), "A chart");
        blocks.0.push(Block::Finding(Box::new(Finding::new(
            "a.rule",
            "x",
            FindingCategory::Structure,
            Severity::Warning,
            Confidence::High,
            "Title & more",
        ))));
        let html = render(&blocks, &page(ReportTheme::Dark));
        assert!(html.starts_with("<!doctype html>"));
        assert!(html.contains(r#"<html lang="en" data-theme="dark">"#));
        assert!(html.contains("color-scheme:dark"));
        assert!(html.contains("Text with &lt;script&gt;alert(1)&lt;/script&gt;"));
        assert!(html.contains(r#"<th scope="col" class="num">Files</th>"#));
        assert!(html.contains(r##"<li><a href="#languages">Language map</a></li>"##));
        assert!(!html.contains(r##"href="#cover""##));
        assert!(html.contains(r#"<section id="languages" aria-labelledby="languages-title">"#));
        assert!(html.contains(r#"data-severity="warning""#));
        assert!(html.contains("Title &amp; more"));
        assert!(html.contains(r#"id="finding-filter""#));
        assert!(
            html.contains("<\\/script>"),
            "the signature cannot close its script element"
        );
        assert!(!html.contains("http://"), "no external resources");
        let css = format!("{}{}", theme_css(ReportTheme::Dark), BASE_CSS);
        assert!(html.contains(&esc(&csp_hash(&css))));
        assert!(html.contains(&esc(&csp_hash(SCRIPT))));
        assert!(html.contains(&format!("<style>{css}</style>")));
        assert!(html.contains(&format!("<script>{SCRIPT}</script>")));
    }

    #[test]
    fn themes_differ() {
        let light = theme_css(ReportTheme::Professional);
        assert!(light.contains("--surface:#fcfcfb"));
        assert!(theme_css(ReportTheme::Minimal).contains("section{border:none"));
        assert!(theme_css(ReportTheme::Technical).contains("font-family:var(--mono)"));
        assert!(theme_css(ReportTheme::Dark).contains("--surface:#1a1a19"));
    }
}
