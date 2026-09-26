//! Go modules: `go.mod`, `go.work`, and `go.sum`.

use repodna_core::model::dependencies::{DependencyScope, ManifestKind};

use crate::model::{
    DeclaredDependency, EcosystemProvider, FileMatch, ParsedLockfile, ParsedManifest, file_name,
};

/// Go provider.
pub struct Go;

/// Iterates the logical directive lines of a `go.mod`/`go.work` file, expanding
/// parenthesized blocks into `(directive, entry)` pairs.
fn directives(content: &str) -> Vec<(String, String)> {
    let mut entries = Vec::new();
    let mut block: Option<String> = None;
    for raw in content.lines() {
        let line = raw.split("//").next().unwrap_or(raw).trim();
        let indirect = raw.contains("// indirect");
        if line.is_empty() {
            continue;
        }
        if let Some(directive) = &block {
            if line == ")" {
                block = None;
            } else {
                let entry = if indirect {
                    format!("{line} indirect")
                } else {
                    line.to_owned()
                };
                entries.push((directive.clone(), entry));
            }
            continue;
        }
        let (directive, rest) = line.split_once(char::is_whitespace).unwrap_or((line, ""));
        let rest = rest.trim();
        if rest == "(" {
            block = Some(directive.to_owned());
        } else {
            let entry = if indirect {
                format!("{rest} indirect")
            } else {
                rest.to_owned()
            };
            entries.push((directive.to_owned(), entry));
        }
    }
    entries
}

impl EcosystemProvider for Go {
    fn ecosystem(&self) -> &'static str {
        "go"
    }

    fn matches(&self, path: &str) -> Option<FileMatch> {
        let kind = match file_name(path) {
            "go.mod" | "go.work" => ManifestKind::Manifest,
            "go.sum" => ManifestKind::Lockfile,
            _ => return None,
        };
        Some(FileMatch { kind })
    }

    fn parse_manifest(&self, path: &str, content: &str) -> Result<ParsedManifest, String> {
        let mut manifest = ParsedManifest::default();
        let is_work = file_name(path) == "go.work";
        if is_work {
            manifest.workspace_tool = Some("go-work".to_owned());
        }
        for (directive, entry) in directives(content) {
            let mut fields = entry.split_whitespace();
            match directive.as_str() {
                "module" => {
                    manifest.package_name = fields.next().map(|m| m.trim_matches('"').to_owned())
                }
                "go" => {
                    if let Some(version) = fields.next() {
                        manifest
                            .requirements
                            .push(("Go".to_owned(), version.to_owned()));
                    }
                }
                "toolchain" => {
                    if let Some(version) = fields.next() {
                        manifest.requirements.push((
                            "Go toolchain".to_owned(),
                            version.trim_start_matches("go").to_owned(),
                        ));
                    }
                }
                "require" if !entry.ends_with(" indirect") => {
                    if let (Some(module), Some(version)) = (fields.next(), fields.next()) {
                        manifest.dependencies.push(DeclaredDependency::registry(
                            module,
                            Some(version.to_owned()),
                            DependencyScope::Runtime,
                        ));
                    }
                }
                "use" if is_work => {
                    if let Some(member) = fields.next() {
                        manifest
                            .workspace_members
                            .push(member.trim_start_matches("./").to_owned());
                    }
                }
                _ => {}
            }
        }
        if manifest.package_name.is_none() && !is_work {
            return Err("go.mod has no module directive".to_owned());
        }
        Ok(manifest)
    }

    fn parse_lockfile(&self, _path: &str, content: &str) -> Result<ParsedLockfile, String> {
        let mut packages: Vec<(String, String)> = content
            .lines()
            .filter_map(|line| {
                let mut fields = line.split_whitespace();
                let module = fields.next()?;
                let version = fields.next()?.trim_end_matches("/go.mod");
                Some((module.to_owned(), version.to_owned()))
            })
            .collect();
        packages.sort();
        packages.dedup();
        Ok(ParsedLockfile { packages })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_go_mod() {
        let manifest = Go
            .parse_manifest(
                "go.mod",
                "module github.com/acme/api\n\ngo 1.22\ntoolchain go1.22.3\n\nrequire github.com/gorilla/mux v1.8.1\n\nrequire (\n\tgithub.com/lib/pq v1.10.9\n\tgolang.org/x/sys v0.20.0 // indirect\n)\n",
            )
            .unwrap();
        assert_eq!(
            manifest.package_name.as_deref(),
            Some("github.com/acme/api")
        );
        let names: Vec<_> = manifest
            .dependencies
            .iter()
            .map(|d| d.name.as_str())
            .collect();
        assert_eq!(names, vec!["github.com/gorilla/mux", "github.com/lib/pq"]);
        assert!(
            manifest
                .requirements
                .contains(&("Go".into(), "1.22".into()))
        );
        assert!(Go.parse_manifest("go.mod", "go 1.22\n").is_err());
    }

    #[test]
    fn parses_go_work_and_go_sum() {
        let work = Go
            .parse_manifest("go.work", "go 1.22\n\nuse (\n\t./api\n\t./worker\n)\n")
            .unwrap();
        assert_eq!(work.workspace_members, vec!["api", "worker"]);
        let sum = Go
            .parse_lockfile(
                "go.sum",
                "github.com/lib/pq v1.10.9 h1:abc=\ngithub.com/lib/pq v1.10.9/go.mod h1:def=\n",
            )
            .unwrap();
        assert_eq!(
            sum.packages,
            vec![("github.com/lib/pq".into(), "v1.10.9".into())]
        );
    }
}
