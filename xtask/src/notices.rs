//! `cargo xtask notices` writes `THIRD-PARTY-NOTICES.txt`: the attribution notices and
//! license texts of the third-party software that RepoDNA's downloads contain.
//!
//! The file covers the Rust crates compiled into the command line and the desktop app on
//! every platform a release is built for (normal dependencies only: build tools and test
//! helpers are not distributed), the npm packages bundled into the web interface (which the
//! command line, the desktop app, and the web version all contain), and the DejaVu fonts.
//!
//! When a component offers a choice of licenses, the Apache License 2.0 (RepoDNA's own
//! license) is chosen where possible, then the MIT License. MIT texts that differ from the
//! standard text only in their copyright lines are listed by copyright line above one copy
//! of the text; every other license text is reproduced as the component ships it.
//! `cargo xtask notices --check` fails when the file is out of date.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

use crate::{Result, root, say};

/// The notices file, relative to the repository root.
pub const FILE: &str = "THIRD-PARTY-NOTICES.txt";

/// A Rust program RepoDNA distributes, the features its release build enables, and the
/// platforms releases build it for. Keep them in step with `.github/workflows/release.yml`.
struct Program {
    manifest: &'static str,
    package: &'static str,
    features: &'static str,
    targets: &'static [&'static str],
}

const PROGRAMS: [Program; 2] = [
    Program {
        manifest: "Cargo.toml",
        package: "repodna-cli",
        features: "",
        targets: &[
            "x86_64-unknown-linux-musl",
            "aarch64-unknown-linux-musl",
            "x86_64-apple-darwin",
            "aarch64-apple-darwin",
            "x86_64-pc-windows-msvc",
        ],
    },
    Program {
        manifest: "apps/desktop/src-tauri/Cargo.toml",
        package: "repodna-desktop",
        features: "custom-protocol",
        targets: &[
            "x86_64-unknown-linux-gnu",
            "x86_64-apple-darwin",
            "aarch64-apple-darwin",
            "x86_64-pc-windows-msvc",
        ],
    },
];

/// The web interface, whose runtime dependencies are bundled.
const WEB_PACKAGE: &str = "apps/web/package.json";

/// Build tools whose code ends up in the web bundle, without their own dependencies: Vite
/// adds its module preloading helpers.
const BUNDLED_TOOLS: [&str; 1] = ["vite"];

/// The license of the DejaVu fonts embedded for PNG rendering.
const FONT_LICENSE: &str = "crates/repodna-report/assets/fonts/LICENSE-DejaVu.txt";

/// Licenses in order of preference when a component offers a choice.
const PREFERENCE: [&str; 14] = [
    "Apache-2.0",
    "MIT",
    "BSD-3-Clause",
    "BSD-2-Clause",
    "ISC",
    "Zlib",
    "0BSD",
    "MIT-0",
    "Unlicense",
    "CC0-1.0",
    "Unicode-3.0",
    "BSL-1.0",
    "MPL-2.0",
    "Apache-2.0 WITH LLVM-exception",
];

/// The standard MIT License text, without the copyright line.
const MIT: &str = r#"Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE."#;

/// The standard BSD 3-Clause License text, without the copyright line.
const BSD_3_CLAUSE: &str = r#"Redistribution and use in source and binary forms, with or without
modification, are permitted provided that the following conditions are met:

1. Redistributions of source code must retain the above copyright notice, this
   list of conditions and the following disclaimer.

2. Redistributions in binary form must reproduce the above copyright notice,
   this list of conditions and the following disclaimer in the documentation
   and/or other materials provided with the distribution.

3. Neither the name of the copyright holder nor the names of its
   contributors may be used to endorse or promote products derived from
   this software without specific prior written permission.

THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS"
AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE
FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL
DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER
CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY,
OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE."#;

/// Where a component comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum Ecosystem {
    Crate,
    Npm,
}

/// A third-party component: one crate or npm package, in every version in use.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Component {
    name: String,
    ecosystem: Ecosystem,
    /// The declared SPDX license expression.
    license: String,
    /// The directories of the versions in use.
    directories: BTreeSet<PathBuf>,
    /// Declared authors, for components that ship no license text.
    authors: BTreeSet<String>,
}

impl Component {
    fn label(&self) -> String {
        match self.ecosystem {
            Ecosystem::Crate => self.name.clone(),
            Ecosystem::Npm => format!("{} (npm)", self.name),
        }
    }

    /// Where the component's source code is published.
    fn source(&self) -> String {
        match self.ecosystem {
            Ecosystem::Crate => format!("https://crates.io/crates/{}", self.name),
            Ecosystem::Npm => format!("https://www.npmjs.com/package/{}", self.name),
        }
    }
}

/// Credits a component whose license text has no copyright line, and says when the
/// package ships no license text at all.
fn by(authors: &BTreeSet<String>, missing_from: Option<&String>) -> String {
    let authors: Vec<&str> = authors
        .iter()
        .map(String::as_str)
        .filter(|a| !a.is_empty())
        .collect();
    let mut notes = Vec::new();
    if !authors.is_empty() {
        notes.push(format!("by {}", authors.join(", ")));
    }
    if let Some(source) = missing_from {
        notes.push(format!(
            "the package includes no license text; see {source}"
        ));
    }
    if notes.is_empty() {
        String::new()
    } else {
        format!(" ({})", notes.join("; "))
    }
}

type Components = BTreeMap<(Ecosystem, String, String), Component>;

fn add(
    components: &mut Components,
    ecosystem: Ecosystem,
    name: &str,
    license: &str,
    directory: PathBuf,
    authors: impl IntoIterator<Item = String>,
) {
    let component = components
        .entry((ecosystem, name.to_owned(), license.to_owned()))
        .or_insert_with(|| Component {
            name: name.to_owned(),
            ecosystem,
            license: license.to_owned(),
            directories: BTreeSet::new(),
            authors: BTreeSet::new(),
        });
    component.directories.insert(directory);
    component.authors.extend(authors);
}

fn cargo() -> String {
    std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned())
}

/// Runs cargo in the repository root and returns its standard output.
fn cargo_output(args: &[&str]) -> Result<Vec<u8>> {
    let output = Command::new(cargo())
        .args(args)
        .current_dir(root())
        .output()
        .map_err(|e| format!("could not run cargo: {e}"))?;
    if output.status.success() {
        Ok(output.stdout)
    } else {
        Err(format!(
            "cargo {} failed:\n{}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr)
        ))
    }
}

/// The crates a program links on one platform, as `(name, version)`, from `cargo tree`,
/// which resolves features as the build does. Path dependencies (RepoDNA's own crates) are
/// left out.
fn linked_crates(program: &Program, target: &str) -> Result<BTreeSet<(String, String)>> {
    let mut args = vec![
        "tree",
        "--locked",
        "--manifest-path",
        program.manifest,
        "--package",
        program.package,
        "--target",
        target,
        "--edges",
        "normal",
        "--prefix",
        "none",
        "--format",
        "{p}",
    ];
    if !program.features.is_empty() {
        args.extend(["--features", program.features]);
    }
    let output = cargo_output(&args)?;
    let mut crates = BTreeSet::new();
    for line in String::from_utf8_lossy(&output).lines() {
        // `name v1.2.3`, `name v1.2.3 (*)` when repeated, or `name v1.2.3 (/path)` for a
        // path dependency.
        let mut parts = line.split_whitespace();
        let (Some(name), Some(version)) = (parts.next(), parts.next()) else {
            continue;
        };
        let path = parts.next().is_some_and(|rest| rest != "(*)");
        if !path && let Some(version) = version.strip_prefix('v') {
            crates.insert((name.to_owned(), version.to_owned()));
        }
    }
    Ok(crates)
}

/// Adds the third-party crates a program links on any of its platforms.
fn add_crates(program: &Program, components: &mut Components) -> Result<()> {
    let metadata: Value = serde_json::from_slice(&cargo_output(&[
        "metadata",
        "--format-version",
        "1",
        "--locked",
        "--manifest-path",
        program.manifest,
    ])?)
    .map_err(|e| format!("cargo metadata: {e}"))?;
    let packages: HashMap<(&str, &str), &Value> = metadata["packages"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|p| !p["source"].is_null())
        .filter_map(|p| Some(((p["name"].as_str()?, p["version"].as_str()?), p)))
        .collect();
    for target in program.targets {
        for (name, version) in linked_crates(program, target)? {
            let p = packages
                .get(&(name.as_str(), version.as_str()))
                .ok_or_else(|| format!("{name} {version} is not in the cargo metadata"))?;
            let license = p["license"].as_str().unwrap_or("NOASSERTION");
            let directory = Path::new(p["manifest_path"].as_str().unwrap_or_default())
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_default();
            let authors = p["authors"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .map(without_email);
            add(
                components,
                Ecosystem::Crate,
                &name,
                license,
                directory,
                authors,
            );
        }
    }
    Ok(())
}

/// `Name <mail@example.com>` without the address.
fn without_email(author: &str) -> String {
    author.split('<').next().unwrap_or(author).trim().to_owned()
}

fn read_json(path: &Path) -> Result<Value> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))
}

/// Adds the npm packages bundled into the web interface.
fn add_npm_packages(components: &mut Components) -> Result<()> {
    let root = root();
    let web = read_json(&root.join(WEB_PACKAGE))?;
    let modules = root.join("node_modules");
    // (name, directory whose node_modules is searched first, follow its dependencies)
    let mut queue: Vec<(String, PathBuf, bool)> = web["dependencies"]
        .as_object()
        .into_iter()
        .flatten()
        .map(|(name, _)| name.clone())
        .filter(|name| !name.starts_with("@repodna/"))
        .map(|name| (name, root.clone(), true))
        .collect();
    queue.extend(
        BUNDLED_TOOLS
            .iter()
            .map(|name| ((*name).to_owned(), root.clone(), false)),
    );
    let mut seen = BTreeSet::new();
    while let Some((name, parent, follow)) = queue.pop() {
        let nested = parent.join("node_modules").join(&name);
        let directory = if nested.is_dir() {
            nested
        } else {
            modules.join(&name)
        };
        if !seen.insert(directory.clone()) {
            continue;
        }
        let manifest = read_json(&directory.join("package.json"))
            .map_err(|_| format!("the npm package {name} is not installed; run `npm ci` first"))?;
        let license = manifest["license"]
            .as_str()
            .or_else(|| manifest["licenses"][0]["type"].as_str())
            .unwrap_or("NOASSERTION")
            .to_owned();
        let author = match &manifest["author"] {
            Value::String(author) => Some(without_email(author)),
            Value::Object(author) => author
                .get("name")
                .and_then(Value::as_str)
                .map(str::to_owned),
            _ => None,
        };
        if follow {
            for dependency in manifest["dependencies"].as_object().into_iter().flatten() {
                queue.push((dependency.0.clone(), directory.clone(), true));
            }
        }
        add(
            components,
            Ecosystem::Npm,
            &name,
            &license,
            directory,
            author,
        );
    }
    Ok(())
}

/// A parsed SPDX license expression.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Expression {
    License(String),
    And(Vec<Expression>),
    Or(Vec<Expression>),
}

/// Parses an SPDX expression; the old `MIT/Apache-2.0` form means `MIT OR Apache-2.0`.
fn parse(text: &str) -> Result<Expression> {
    let spaced = text
        .replace('(', " ( ")
        .replace(')', " ) ")
        .replace('/', " OR ");
    let tokens: Vec<&str> = spaced.split_whitespace().collect();
    let mut position = 0;
    let expression = parse_or(&tokens, &mut position)?;
    if position == tokens.len() {
        Ok(expression)
    } else {
        Err(format!("cannot read the license expression `{text}`"))
    }
}

fn keyword(token: Option<&&str>, word: &str) -> bool {
    token.is_some_and(|t| t.eq_ignore_ascii_case(word))
}

fn parse_or(tokens: &[&str], position: &mut usize) -> Result<Expression> {
    let mut items = vec![parse_and(tokens, position)?];
    while keyword(tokens.get(*position), "OR") {
        *position += 1;
        items.push(parse_and(tokens, position)?);
    }
    Ok(if items.len() == 1 {
        items.remove(0)
    } else {
        Expression::Or(items)
    })
}

fn parse_and(tokens: &[&str], position: &mut usize) -> Result<Expression> {
    let mut items = vec![parse_primary(tokens, position)?];
    while keyword(tokens.get(*position), "AND") {
        *position += 1;
        items.push(parse_primary(tokens, position)?);
    }
    Ok(if items.len() == 1 {
        items.remove(0)
    } else {
        Expression::And(items)
    })
}

fn parse_primary(tokens: &[&str], position: &mut usize) -> Result<Expression> {
    match tokens.get(*position) {
        Some(&"(") => {
            *position += 1;
            let inner = parse_or(tokens, position)?;
            if tokens.get(*position) != Some(&")") {
                return Err("unbalanced parentheses in a license expression".to_owned());
            }
            *position += 1;
            Ok(inner)
        }
        Some(&token) if token != ")" && !["AND", "OR", "WITH"].contains(&token) => {
            *position += 1;
            let mut license = token.to_owned();
            if keyword(tokens.get(*position), "WITH") {
                let exception = tokens.get(*position + 1).ok_or("WITH needs an exception")?;
                license = format!("{license} WITH {exception}");
                *position += 2;
            }
            Ok(Expression::License(license))
        }
        _ => Err("incomplete license expression".to_owned()),
    }
}

fn rank(license: &str) -> usize {
    PREFERENCE
        .iter()
        .position(|known| *known == license)
        .unwrap_or(PREFERENCE.len())
}

/// The licenses to comply with: every part of an `AND`, and the preferred choice of an `OR`.
fn choose(expression: &Expression) -> BTreeSet<String> {
    match expression {
        Expression::License(license) => BTreeSet::from([license.clone()]),
        Expression::And(items) => items.iter().flat_map(choose).collect(),
        Expression::Or(items) => items
            .iter()
            .map(choose)
            .min_by_key(|set| (set.iter().map(|l| rank(l)).max(), set.len()))
            .unwrap_or_default(),
    }
}

/// Collapses whitespace and straightens quotes, for comparing license texts.
fn normalize(text: &str) -> String {
    text.replace(['\u{201c}', '\u{201d}'], "\"")
        .replace(['\u{2018}', '\u{2019}'], "'")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// The license a text states, recognized by phrases every copy of it contains. A file that
/// states several licenses is classified by the first one.
fn classify(text: &str) -> Option<&'static str> {
    let t = normalize(text).to_lowercase();
    let at = |phrase: &str| t.find(phrase);
    let after = |start: usize, phrase: &str| t[start..].contains(phrase);
    let candidates = [
        at("apache license")
            .filter(|&i| after(i, "version 2.0"))
            .map(|i| (i, "Apache-2.0")),
        at("mozilla public license").map(|i| (i, "MPL-2.0")),
        at("unicode license")
            .or_else(|| at("unicode, inc"))
            .map(|i| (i, "Unicode-3.0")),
        at("permission is hereby granted, free of charge").map(|i| {
            let mit = after(
                i,
                "the above copyright notice and this permission notice shall be included",
            );
            (i, if mit { "MIT" } else { "MIT-0" })
        }),
        at("redistribution and use in source and binary forms").map(|i| {
            let three = after(i, "neither the name");
            (
                i,
                if three {
                    "BSD-3-Clause"
                } else {
                    "BSD-2-Clause"
                },
            )
        }),
        at("permission is granted to anyone to use this software for any purpose")
            .map(|i| (i, "Zlib")),
        at("distribute this software for any purpose with or without fee is hereby granted").map(
            |i| {
                let isc = after(i, "provided that the above copyright notice");
                (i, if isc { "ISC" } else { "0BSD" })
            },
        ),
        at("free and unencumbered software released into the public domain")
            .map(|i| (i, "Unlicense")),
        at("boost software license").map(|i| (i, "BSL-1.0")),
        at("cc0")
            .filter(|_| t.contains("creative commons"))
            .map(|i| (i, "CC0-1.0")),
    ];
    candidates
        .into_iter()
        .flatten()
        .min_by_key(|(position, _)| *position)
        .map(|(_, license)| license)
}

/// Reads a text file with Unix line endings and without trailing spaces. A section that
/// lists the licenses of a package's own bundled dependencies (as Vite's license does) is
/// left out: only the package's own code is distributed.
fn read_text(path: &Path) -> Option<String> {
    let bytes = std::fs::read(path).ok()?;
    let text = String::from_utf8_lossy(&bytes).replace("\r\n", "\n");
    let lines: Vec<&str> = text
        .lines()
        .map(str::trim_end)
        .take_while(|line| {
            !line
                .to_lowercase()
                .starts_with("# licenses of bundled dependencies")
        })
        .collect();
    let text = lines.join("\n");
    let text = text.trim_matches('\n');
    (!text.is_empty()).then(|| text.to_owned())
}

/// The license and notice files at the top of a package, sorted by name.
fn files_named(directory: &Path, prefixes: &[&str]) -> Vec<(String, String)> {
    let mut found: Vec<(String, String)> = std::fs::read_dir(directory)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|entry| entry.path().is_file())
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            let lower = name.to_lowercase();
            prefixes
                .iter()
                .any(|prefix| lower.starts_with(prefix))
                .then_some(name)
        })
        .filter_map(|name| Some((name.clone(), read_text(&directory.join(&name))?)))
        .collect();
    found.sort();
    found
}

fn license_files(directory: &Path) -> Vec<(String, String)> {
    files_named(
        directory,
        &["license", "licence", "copying", "copyright", "unlicense"],
    )
}

/// A line such as `Copyright (c) 2020 Someone`, but not a wrapped line of license text
/// such as `COPYRIGHT HOLDERS BE LIABLE`.
fn is_copyright_line(line: &str) -> bool {
    let lower = line.trim().to_lowercase();
    if lower.starts_with("(c)") || lower.starts_with('\u{a9}') {
        return true;
    }
    let Some(rest) = lower.strip_prefix("copyright") else {
        return false;
    };
    let rest = rest.trim_start_matches([' ', ':']);
    !rest.is_empty()
        && !["holder", "notice", "owner", "and "]
            .iter()
            .any(|word| rest.starts_with(word))
}

/// The copyright lines of an MIT text that is otherwise the standard text.
fn mit_copyrights(text: &str) -> Option<Vec<String>> {
    let titles = [
        "mit license",
        "the mit license",
        "mit license (mit)",
        "the mit license (mit)",
        "the mit license (expat)",
    ];
    let mut copyrights = Vec::new();
    let mut rest = String::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if is_copyright_line(trimmed) {
            copyrights.push(trimmed.to_owned());
        } else if !titles.contains(&trimmed.to_lowercase().as_str())
            && !(trimmed.len() > 2 && trimmed.chars().all(|c| c == '=' || c == '-'))
        {
            rest.push_str(line);
            rest.push('\n');
        }
    }
    (normalize(&rest) == normalize(MIT)).then_some(copyrights)
}

/// What the notices say about one component under one license.
#[derive(Debug, Default)]
struct Group {
    /// Components used under the license without text of their own in this file.
    listed: BTreeMap<String, Vec<String>>,
    /// Verbatim license texts, keyed by their normalized form, and the components that
    /// ship each.
    texts: BTreeMap<String, (String, BTreeSet<String>)>,
    /// Components that ship no text for the license, with where their source is.
    missing: BTreeMap<String, String>,
}

fn title(license: &str) -> &str {
    match license {
        "Apache-2.0" => "Apache License 2.0",
        "MIT" => "MIT License",
        "BSD-3-Clause" => "BSD 3-Clause License",
        "BSD-2-Clause" => "BSD 2-Clause License",
        "ISC" => "ISC License",
        "Zlib" => "zlib License",
        "0BSD" => "BSD Zero Clause License",
        "MIT-0" => "MIT No Attribution License",
        "Unlicense" => "The Unlicense",
        "CC0-1.0" => "Creative Commons Zero v1.0 Universal",
        "Unicode-3.0" => "Unicode License v3",
        "BSL-1.0" => "Boost Software License 1.0",
        "MPL-2.0" => "Mozilla Public License 2.0",
        other => other,
    }
}

/// Separates the license texts of different components.
const RULE: &str =
    "--------------------------------------------------------------------------------";

/// Wraps `items` as comma-separated lines indented by two spaces.
fn wrap_list(items: &[String]) -> String {
    let mut out = String::new();
    let mut line = String::from(" ");
    for (index, item) in items.iter().enumerate() {
        let piece = if index + 1 < items.len() {
            format!("{item},")
        } else {
            item.clone()
        };
        if line.len() + 1 + piece.len() > 90 && line.len() > 1 {
            out.push_str(&line);
            out.push('\n');
            line = String::from(" ");
        }
        line.push(' ');
        line.push_str(&piece);
    }
    out.push_str(&line);
    out.push('\n');
    out
}

fn heading(out: &mut String, text: &str, underline: char) {
    out.push('\n');
    out.push_str(text);
    out.push('\n');
    out.push_str(&underline.to_string().repeat(text.chars().count()));
    out.push_str("\n\n");
}

/// Builds the notices from the components found.
fn render(components: &Components) -> Result<String> {
    let root = root();
    let mut authors: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for component in components.values() {
        authors
            .entry(component.label())
            .or_default()
            .extend(component.authors.iter().cloned());
    }
    let mut groups: BTreeMap<(usize, String), Group> = BTreeMap::new();
    let mut notices: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for component in components.values() {
        let chosen = choose(&parse(&component.license)?);
        let label = component.label();
        let files: Vec<(String, String)> = component
            .directories
            .iter()
            .flat_map(|directory| license_files(directory))
            .collect();
        for license in chosen {
            if !PREFERENCE.contains(&license.as_str()) {
                return Err(format!(
                    "{label} is licensed under {license}, which xtask/src/notices.rs does not know; add it there"
                ));
            }
            let group = groups.entry((rank(&license), license.clone())).or_default();
            if license == "Apache-2.0" {
                group.listed.entry(label.clone()).or_default();
                for directory in &component.directories {
                    for (_, text) in files_named(directory, &["notice"]) {
                        notices.entry(text).or_default().insert(label.clone());
                    }
                }
                continue;
            }
            let texts: Vec<&String> = files
                .iter()
                .filter(|(_, text)| classify(text) == Some(license.as_str()))
                .map(|(_, text)| text)
                .collect();
            if texts.is_empty() {
                group.missing.insert(label.clone(), component.source());
                continue;
            }
            for text in texts {
                match (license.as_str(), mit_copyrights(text)) {
                    ("MIT", Some(copyrights)) => {
                        let lines = group.listed.entry(label.clone()).or_default();
                        for copyright in copyrights {
                            if !lines.contains(&copyright) {
                                lines.push(copyright);
                            }
                        }
                    }
                    _ => {
                        group
                            .texts
                            .entry(normalize(text))
                            .or_insert_with(|| (text.clone(), BTreeSet::new()))
                            .1
                            .insert(label.clone());
                    }
                }
            }
        }
    }

    let crates = components
        .values()
        .filter(|c| c.ecosystem == Ecosystem::Crate)
        .map(|c| &c.name)
        .collect::<BTreeSet<_>>()
        .len();
    let packages = components
        .values()
        .filter(|c| c.ecosystem == Ecosystem::Npm)
        .map(|c| &c.name)
        .collect::<BTreeSet<_>>()
        .len();
    let mut out = String::new();
    out.push_str("Third-party software in RepoDNA\n===============================\n\n");
    out.push_str(&format!(
        "RepoDNA is licensed under the Apache License 2.0 (see LICENSE and NOTICE). Its downloads
include the third-party software listed here: {crates} Rust crates compiled into the `repodna`
command line and the desktop app, {packages} npm packages bundled into the web interface
(which the command line, the desktop app, and the web version all contain), and the DejaVu
fonts. Each component is listed under the license RepoDNA uses it under; where a component
offers a choice, the Apache License 2.0 or the MIT License is used. Components marked (npm)
are npm packages; the others are Rust crates, whose source code is available at
https://crates.io/crates/NAME.

This file is generated by `cargo xtask notices`.
"
    ));

    for ((_, license), group) in &groups {
        heading(&mut out, title(license), '=');
        if license == "Apache-2.0" {
            out.push_str(
                "Used under the Apache License 2.0, whose text is at the end of this file:\n\n",
            );
            let names: Vec<String> = group.listed.keys().cloned().collect();
            out.push_str(&wrap_list(&names));
            if !notices.is_empty() {
                out.push_str("\nThe NOTICE files these components include:\n");
                for (text, labels) in &notices {
                    let labels: Vec<String> = labels.iter().cloned().collect();
                    out.push_str(&format!("\n{RULE}\n{}\n{text}\n", wrap_list(&labels)));
                }
            }
            continue;
        }
        if license == "MPL-2.0" {
            out.push_str(
                "The source code of these components is available from crates.io, as noted above.\n",
            );
        }
        if !group.listed.is_empty() || !group.missing.is_empty() {
            out.push_str(&format!(
                "Used under the {} with these copyright notices; the license text follows them.\n\n",
                title(license)
            ));
            for (label, copyrights) in &group.listed {
                if copyrights.is_empty() {
                    out.push_str(&format!("  {label}{}\n", by(&authors[label], None)));
                }
                for copyright in copyrights {
                    out.push_str(&format!("  {label}: {copyright}\n"));
                }
            }
        }
        for (label, source) in &group.missing {
            out.push_str(&format!("  {label}{}\n", by(&authors[label], Some(source))));
        }
        let standard = match license.as_str() {
            "MIT" => Some(MIT),
            "BSD-3-Clause" => Some(BSD_3_CLAUSE),
            _ => None,
        };
        if !group.listed.is_empty() || !group.missing.is_empty() {
            match standard {
                Some(text) => out.push_str(&format!("\n{text}\n")),
                None if !group.texts.is_empty() => out.push_str(
                    "\nThe components above ship no license text; the text below applies to them.\n",
                ),
                None => {
                    return Err(format!(
                        "no {license} text is available for {}",
                        group.missing.keys().cloned().collect::<Vec<_>>().join(", ")
                    ));
                }
            }
        }
        for (text, labels) in group.texts.values() {
            let labels: Vec<String> = labels.iter().cloned().collect();
            out.push_str(&format!("\n{RULE}\n{}\n{text}\n", wrap_list(&labels)));
        }
    }

    heading(&mut out, "Fonts", '=');
    out.push_str(
        "The DejaVu Sans fonts are embedded in the command line and the desktop app to render PNG\nimages. Their license:\n\n",
    );
    let fonts =
        read_text(&root.join(FONT_LICENSE)).ok_or_else(|| format!("cannot read {FONT_LICENSE}"))?;
    out.push_str(&fonts);
    out.push('\n');

    if components
        .values()
        .any(|c| c.name == "libsqlite3-sys" && c.ecosystem == Ecosystem::Crate)
    {
        heading(&mut out, "SQLite", '=');
        out.push_str(
            "The command line and the desktop app include SQLite, compiled in by the libsqlite3-sys\ncrate. SQLite is in the public domain: https://sqlite.org/copyright.html\n",
        );
    }

    heading(&mut out, "Apache License 2.0", '=');
    let apache =
        read_text(&root.join("LICENSE")).ok_or_else(|| "cannot read LICENSE".to_owned())?;
    out.push_str(&apache);
    out.push('\n');
    Ok(out)
}

/// Finds every component and renders the notices.
fn generate() -> Result<String> {
    let mut components = Components::new();
    for program in &PROGRAMS {
        add_crates(program, &mut components)?;
    }
    add_npm_packages(&mut components)?;
    render(&components)
}

/// Runs `cargo xtask notices [--check]`.
pub fn run(args: &[String]) -> Result<()> {
    let check = args.iter().any(|arg| arg == "--check");
    let path = root().join(FILE);
    let text = generate()?;
    if check {
        let current = std::fs::read_to_string(&path).unwrap_or_default();
        if current.replace("\r\n", "\n") != text {
            return Err(format!(
                "{FILE} is out of date; run `cargo xtask notices` and commit the result"
            ));
        }
        say(&format!("{FILE} is up to date."));
        return Ok(());
    }
    std::fs::write(&path, &text).map_err(|e| format!("cannot write {FILE}: {e}"))?;
    say(&format!("Wrote {FILE} ({} KB).", text.len().div_ceil(1024)));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chosen(expression: &str) -> Vec<String> {
        choose(&parse(expression).unwrap()).into_iter().collect()
    }

    #[test]
    fn prefers_apache_then_mit() {
        assert_eq!(chosen("MIT OR Apache-2.0"), ["Apache-2.0"]);
        assert_eq!(chosen("MIT/Apache-2.0"), ["Apache-2.0"]);
        assert_eq!(chosen("Unlicense OR MIT"), ["MIT"]);
        assert_eq!(
            chosen("Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT"),
            ["Apache-2.0"]
        );
        assert_eq!(chosen("Zlib OR Apache-2.0 OR MIT"), ["Apache-2.0"]);
    }

    #[test]
    fn keeps_every_part_of_a_conjunction() {
        assert_eq!(
            chosen("(MIT OR Apache-2.0) AND Unicode-3.0"),
            ["Apache-2.0", "Unicode-3.0"]
        );
        assert_eq!(chosen("BSD-3-Clause AND MIT"), ["BSD-3-Clause", "MIT"]);
        assert_eq!(
            chosen("Apache-2.0 WITH LLVM-exception"),
            ["Apache-2.0 WITH LLVM-exception"]
        );
        assert!(parse("MIT OR").is_err());
        assert!(parse("(MIT").is_err());
    }

    #[test]
    fn recognizes_license_texts() {
        let mit = format!("MIT License\n\nCopyright (c) 2020 Someone\n\n{MIT}\n");
        assert_eq!(classify(&mit), Some("MIT"));
        assert_eq!(
            mit_copyrights(&mit),
            Some(vec!["Copyright (c) 2020 Someone".to_owned()])
        );
        let changed = mit.replace("merge, publish", "merge, sell, publish");
        assert_eq!(mit_copyrights(&changed), None);
        let wrapped = format!(
            "The MIT License (MIT)\n=====================\n\nCopyright \u{a9} 2015 Someone\n\n{}\n",
            MIT.replace(
                "AUTHORS OR COPYRIGHT HOLDERS",
                "AUTHORS OR\nCOPYRIGHT HOLDERS"
            )
        );
        assert_eq!(
            mit_copyrights(&wrapped),
            Some(vec!["Copyright \u{a9} 2015 Someone".to_owned()])
        );
        assert!(!is_copyright_line(
            "COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM"
        ));
        assert!(!is_copyright_line(
            "copyright notice and this permission notice"
        ));
        assert_eq!(classify(BSD_3_CLAUSE), Some("BSD-3-Clause"));
        assert_eq!(
            classify(&std::fs::read_to_string(root().join("LICENSE")).unwrap()),
            Some("Apache-2.0")
        );
        assert_eq!(classify("All rights reserved."), None);
    }

    #[test]
    fn wraps_lists() {
        let items: Vec<String> = (0..40).map(|i| format!("crate-{i}")).collect();
        let text = wrap_list(&items);
        assert!(text.lines().all(|line| line.len() <= 90));
        assert!(text.ends_with("crate-39\n"));
    }
}
