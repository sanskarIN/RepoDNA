//! JVM ecosystems: Maven (`pom.xml`) and Gradle (`build.gradle[.kts]`, `settings.gradle[.kts]`,
//! `gradle/libs.versions.toml`, `gradle.lockfile`).
//!
//! Build files are programs (Gradle) or templated XML (Maven), so extraction reads literal
//! declarations only; interpolated values such as `${version}` are kept as written.

use std::sync::LazyLock;

use regex::Regex;
use repodna_core::model::dependencies::{DependencyScope, ManifestKind};

use crate::model::{
    DeclaredDependency, EcosystemProvider, FileMatch, ParsedLockfile, ParsedManifest, file_name,
};

fn re(pattern: &str) -> Regex {
    Regex::new(pattern).unwrap_or_else(|error| panic!("invalid JVM pattern {pattern}: {error}"))
}

static XML_COMMENT: LazyLock<Regex> = LazyLock::new(|| re(r"(?s)<!--.*?-->"));
static DEPENDENCY_BLOCK: LazyLock<Regex> =
    LazyLock::new(|| re(r"(?s)<dependency>(.*?)</dependency>"));
static MODULE: LazyLock<Regex> = LazyLock::new(|| re(r"<module>\s*([^<\s]+)\s*</module>"));

/// Removes every `<tag>…</tag>` block (non-nested) for each tag.
fn strip_blocks(text: &str, tags: &[&str]) -> String {
    let mut result = text.to_owned();
    for tag in tags {
        let open = format!("<{tag}>");
        let close = format!("</{tag}>");
        while let Some(start) = result.find(&open) {
            let Some(end) = result[start..].find(&close) else {
                break;
            };
            result.replace_range(start..start + end + close.len(), "");
        }
    }
    result
}

/// Returns the trimmed text of the first `<tag>…</tag>` element without nested markup.
fn element(text: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let start = text.find(&open)? + open.len();
    let end = text[start..].find(&close)? + start;
    let value = text[start..end].trim();
    (!value.is_empty() && !value.contains('<')).then(|| value.to_owned())
}

/// Maven provider.
pub struct Maven;

impl EcosystemProvider for Maven {
    fn ecosystem(&self) -> &'static str {
        "maven"
    }

    fn matches(&self, path: &str) -> Option<FileMatch> {
        (file_name(path) == "pom.xml").then_some(FileMatch {
            kind: ManifestKind::Manifest,
        })
    }

    fn parse_manifest(&self, _path: &str, content: &str) -> Result<ParsedManifest, String> {
        if !content.contains("<project") {
            return Err("pom.xml has no <project> element".to_owned());
        }
        let text = XML_COMMENT.replace_all(content, "").into_owned();
        let header = strip_blocks(
            &text,
            &[
                "parent",
                "dependencies",
                "dependencyManagement",
                "build",
                "profiles",
                "reporting",
                "modules",
                "properties",
                "developers",
                "licenses",
                "scm",
            ],
        );
        let mut manifest = ParsedManifest {
            package_name: match (element(&header, "groupId"), element(&header, "artifactId")) {
                (Some(group), Some(artifact)) => Some(format!("{group}:{artifact}")),
                (None, artifact) => artifact,
                (Some(_), None) => None,
            },
            package_version: element(&header, "version"),
            description: element(&header, "description"),
            ..ParsedManifest::default()
        };
        let without_management = strip_blocks(
            &text,
            &["dependencyManagement", "plugins", "pluginManagement"],
        );
        for block in DEPENDENCY_BLOCK.captures_iter(&without_management) {
            let body = &block[1];
            let (Some(group), Some(artifact)) =
                (element(body, "groupId"), element(body, "artifactId"))
            else {
                manifest.partial = true;
                continue;
            };
            let scope = match element(body, "scope").as_deref() {
                Some("test") => DependencyScope::Development,
                Some("provided" | "system") => DependencyScope::Build,
                _ if element(body, "optional").as_deref() == Some("true") => {
                    DependencyScope::Optional
                }
                _ => DependencyScope::Runtime,
            };
            manifest.dependencies.push(DeclaredDependency::registry(
                format!("{group}:{artifact}"),
                element(body, "version"),
                scope,
            ));
        }
        let modules: Vec<String> = MODULE
            .captures_iter(&text)
            .map(|c| c[1].to_owned())
            .collect();
        if !modules.is_empty() {
            manifest.workspace_tool = Some("maven-modules".to_owned());
            manifest.workspace_members = modules;
        }
        if let Some(java) = element(&text, "maven.compiler.release")
            .or_else(|| element(&text, "maven.compiler.source"))
            .or_else(|| element(&text, "java.version"))
        {
            manifest.requirements.push(("Java".to_owned(), java));
        }
        Ok(manifest)
    }

    fn parse_lockfile(&self, _path: &str, _content: &str) -> Result<ParsedLockfile, String> {
        Err("Maven has no lockfile format".to_owned())
    }
}

static GRADLE_DEPENDENCY: LazyLock<Regex> = LazyLock::new(|| {
    re(
        r#"\b(implementation|api|compileOnly|runtimeOnly|testImplementation|testCompileOnly|testRuntimeOnly|androidTestImplementation|debugImplementation|releaseImplementation|kapt|ksp|annotationProcessor|classpath|compile|testCompile|developmentOnly)\s*\(?\s*["']([^"':\s]+):([^"':\s]+)(?::([^"'\s]+))?["']"#,
    )
});
static GRADLE_INCLUDE: LazyLock<Regex> = LazyLock::new(|| re(r#"["']:?([A-Za-z0-9_.\-:]+)["']"#));
static GRADLE_JAVA: LazyLock<Regex> = LazyLock::new(|| {
    re(
        r#"(?:jvmToolchain\s*\(\s*(\d+)|JavaVersion\.VERSION_(\d+(?:_\d+)?)|languageVersion(?:\.set)?\s*[=(]\s*JavaLanguageVersion\.of\(\s*(\d+))"#,
    )
});

/// Gradle provider.
pub struct Gradle;

impl Gradle {
    fn parse_build_script(content: &str) -> ParsedManifest {
        let mut manifest = ParsedManifest::default();
        for captures in GRADLE_DEPENDENCY.captures_iter(content) {
            let configuration = &captures[1];
            let scope = if configuration.starts_with("test")
                || configuration.starts_with("androidTest")
                || configuration == "debugImplementation"
            {
                DependencyScope::Development
            } else if matches!(
                configuration,
                "compileOnly" | "kapt" | "ksp" | "annotationProcessor" | "classpath"
            ) {
                DependencyScope::Build
            } else {
                DependencyScope::Runtime
            };
            manifest.dependencies.push(DeclaredDependency::registry(
                format!("{}:{}", &captures[2], &captures[3]),
                captures.get(4).map(|m| m.as_str().to_owned()),
                scope,
            ));
        }
        // Version-catalog references (`libs.foo`) and map notation are not resolved here.
        if content.contains("libs.") || content.contains("group:") {
            manifest.partial = true;
        }
        if let Some(captures) = GRADLE_JAVA.captures(content) {
            let version = captures
                .iter()
                .skip(1)
                .flatten()
                .next()
                .map(|m| m.as_str().replace('_', "."));
            if let Some(version) = version {
                manifest.requirements.push(("Java".to_owned(), version));
            }
        }
        manifest
    }

    fn parse_settings(content: &str) -> ParsedManifest {
        let mut members = Vec::new();
        for line in content
            .lines()
            .map(str::trim)
            .filter(|line| line.starts_with("include"))
        {
            for captures in GRADLE_INCLUDE.captures_iter(line) {
                members.push(captures[1].replace(':', "/"));
            }
        }
        ParsedManifest {
            workspace_tool: (!members.is_empty()).then(|| "gradle-multi-project".to_owned()),
            workspace_members: members,
            ..ParsedManifest::default()
        }
    }

    fn parse_catalog(content: &str) -> Result<ParsedManifest, String> {
        let table: toml::Table = content
            .parse()
            .map_err(|error: toml::de::Error| error.to_string())?;
        let versions = table.get("versions").and_then(toml::Value::as_table);
        let mut manifest = ParsedManifest::default();
        if let Some(libraries) = table.get("libraries").and_then(toml::Value::as_table) {
            for value in libraries.values() {
                let (module, version) = match value {
                    toml::Value::String(coordinates) => {
                        let mut parts = coordinates.splitn(3, ':');
                        let group = parts.next().unwrap_or_default();
                        let artifact = parts.next().unwrap_or_default();
                        (
                            format!("{group}:{artifact}"),
                            parts.next().map(str::to_owned),
                        )
                    }
                    toml::Value::Table(entry) => {
                        let module = entry
                            .get("module")
                            .and_then(toml::Value::as_str)
                            .map(str::to_owned)
                            .or_else(|| {
                                Some(format!(
                                    "{}:{}",
                                    entry.get("group")?.as_str()?,
                                    entry.get("name")?.as_str()?
                                ))
                            });
                        let version = match entry.get("version") {
                            Some(toml::Value::String(version)) => Some(version.clone()),
                            Some(toml::Value::Table(reference)) => reference
                                .get("ref")
                                .and_then(toml::Value::as_str)
                                .and_then(|key| versions?.get(key)?.as_str().map(str::to_owned)),
                            _ => None,
                        };
                        match module {
                            Some(module) => (module, version),
                            None => {
                                manifest.partial = true;
                                continue;
                            }
                        }
                    }
                    _ => continue,
                };
                manifest.dependencies.push(DeclaredDependency::registry(
                    module,
                    version,
                    DependencyScope::Runtime,
                ));
            }
        }
        Ok(manifest)
    }
}

impl EcosystemProvider for Gradle {
    fn ecosystem(&self) -> &'static str {
        "gradle"
    }

    fn matches(&self, path: &str) -> Option<FileMatch> {
        let kind = match file_name(path) {
            "build.gradle"
            | "build.gradle.kts"
            | "settings.gradle"
            | "settings.gradle.kts"
            | "libs.versions.toml" => ManifestKind::Manifest,
            "gradle.lockfile" => ManifestKind::Lockfile,
            _ => return None,
        };
        Some(FileMatch { kind })
    }

    fn parse_manifest(&self, path: &str, content: &str) -> Result<ParsedManifest, String> {
        match file_name(path) {
            "settings.gradle" | "settings.gradle.kts" => Ok(Self::parse_settings(content)),
            "libs.versions.toml" => Self::parse_catalog(content),
            _ => Ok(Self::parse_build_script(content)),
        }
    }

    fn parse_lockfile(&self, _path: &str, content: &str) -> Result<ParsedLockfile, String> {
        let packages = content
            .lines()
            .map(str::trim)
            .filter(|line| {
                !line.is_empty() && !line.starts_with('#') && !line.starts_with("empty=")
            })
            .filter_map(|line| {
                let coordinates = line.split('=').next()?;
                let mut parts = coordinates.splitn(3, ':');
                let group = parts.next()?;
                let artifact = parts.next()?;
                let version = parts.next()?;
                Some((format!("{group}:{artifact}"), version.to_owned()))
            })
            .collect();
        Ok(ParsedLockfile { packages })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const POM: &str = r#"<?xml version="1.0"?>
<project>
  <parent><groupId>org.parent</groupId><artifactId>parent</artifactId><version>9</version></parent>
  <groupId>com.acme</groupId>
  <artifactId>service</artifactId>
  <version>2.1.0</version>
  <description>Acme service</description>
  <properties><java.version>21</java.version></properties>
  <modules><module>core</module><module>web</module></modules>
  <dependencyManagement><dependencies><dependency><groupId>x</groupId><artifactId>managed</artifactId></dependency></dependencies></dependencyManagement>
  <dependencies>
    <!-- <dependency><groupId>commented</groupId><artifactId>out</artifactId></dependency> -->
    <dependency><groupId>org.springframework</groupId><artifactId>spring-core</artifactId><version>${spring.version}</version></dependency>
    <dependency><groupId>junit</groupId><artifactId>junit</artifactId><version>4.13.2</version><scope>test</scope></dependency>
    <dependency><groupId>javax.servlet</groupId><artifactId>servlet-api</artifactId><scope>provided</scope></dependency>
  </dependencies>
</project>"#;

    #[test]
    fn parses_maven_poms() {
        let manifest = Maven.parse_manifest("pom.xml", POM).unwrap();
        assert_eq!(manifest.package_name.as_deref(), Some("com.acme:service"));
        assert_eq!(manifest.package_version.as_deref(), Some("2.1.0"));
        assert_eq!(manifest.description.as_deref(), Some("Acme service"));
        assert_eq!(manifest.workspace_members, vec!["core", "web"]);
        assert_eq!(manifest.requirements, vec![("Java".into(), "21".into())]);
        let deps: Vec<_> = manifest
            .dependencies
            .iter()
            .map(|d| (d.name.as_str(), d.scope))
            .collect();
        assert_eq!(
            deps,
            vec![
                ("org.springframework:spring-core", DependencyScope::Runtime),
                ("junit:junit", DependencyScope::Development),
                ("javax.servlet:servlet-api", DependencyScope::Build),
            ]
        );
        assert_eq!(
            manifest.dependencies[0].requirement.as_deref(),
            Some("${spring.version}")
        );
        assert!(Maven.parse_manifest("pom.xml", "<html/>").is_err());
    }

    #[test]
    fn parses_gradle_build_scripts() {
        let manifest = Gradle
            .parse_manifest(
                "build.gradle.kts",
                "dependencies {\n    implementation(\"com.squareup.okhttp3:okhttp:4.12.0\")\n    testImplementation(\"org.junit.jupiter:junit-jupiter:5.10.0\")\n    compileOnly 'org.projectlombok:lombok:1.18.30'\n    implementation(libs.kotlinx.coroutines)\n}\nkotlin { jvmToolchain(17) }\n",
            )
            .unwrap();
        assert_eq!(manifest.dependencies.len(), 3);
        assert_eq!(manifest.dependencies[1].scope, DependencyScope::Development);
        assert_eq!(manifest.dependencies[2].scope, DependencyScope::Build);
        assert!(
            manifest.partial,
            "version catalog references are unresolved"
        );
        assert_eq!(manifest.requirements, vec![("Java".into(), "17".into())]);
    }

    #[test]
    fn parses_gradle_settings_catalogs_and_lockfiles() {
        let settings = Gradle
            .parse_manifest(
                "settings.gradle",
                "rootProject.name = 'app'\ninclude ':core', ':feature:login'\n",
            )
            .unwrap();
        assert_eq!(settings.workspace_members, vec!["core", "feature/login"]);
        let catalog = Gradle
            .parse_manifest(
                "gradle/libs.versions.toml",
                "[versions]\nktor = \"2.3.9\"\n[libraries]\nktor-core = { module = \"io.ktor:ktor-server-core\", version.ref = \"ktor\" }\nguava = \"com.google.guava:guava:33.0.0-jre\"\n",
            )
            .unwrap();
        assert_eq!(catalog.dependencies.len(), 2);
        let ktor = catalog
            .dependencies
            .iter()
            .find(|d| d.name == "io.ktor:ktor-server-core")
            .unwrap();
        assert_eq!(ktor.requirement.as_deref(), Some("2.3.9"));
        let lock = Gradle
            .parse_lockfile(
                "gradle.lockfile",
                "# comment\ncom.google.guava:guava:33.0.0-jre=runtimeClasspath\nempty=\n",
            )
            .unwrap();
        assert_eq!(
            lock.packages,
            vec![("com.google.guava:guava".into(), "33.0.0-jre".into())]
        );
    }
}
