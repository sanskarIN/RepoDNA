//! Module inference: grouping code files into components.
//!
//! Package manifests (a crate, an npm package, a Go module, …) declare module boundaries
//! directly. Everything else is grouped by directory, following common layout conventions:
//!
//! - Single-child directory chains are collapsed, so `src/main/java/com/example/app` counts
//!   as one step and deep namespace directories do not hide the real components.
//! - Conventional source roots (`src`, `lib`, `app`, …) are looked into, so `src/parser`
//!   and `src/codegen` become separate modules instead of one `src` module.
//! - Grouping directories (`packages`, `apps`, `crates`, `cmd`, `internal`, …) contribute
//!   one module per child directory.
//! - A module that holds at least half of all code files (and at least ten files) is split
//!   into its subdirectories, at most twice, so one giant module cannot hide the structure.

use std::collections::{BTreeMap, HashMap};

use repodna_core::confidence::Confidence;
use repodna_core::evidence::Evidence;
use repodna_core::model::architecture::{ModuleKind, PackageBoundary};
use repodna_core::paths;

/// Identifier of the module holding code files at the repository root.
pub const ROOT_MODULE: &str = "(root)";

/// Directories whose children are usually independent components.
const GROUPING_DIRS: &[&str] = &[
    "apps", "cmd", "crates", "internal", "libs", "modules", "packages", "pkg", "plugins",
    "services",
];

/// Conventional source roots whose children are the real components.
const SOURCE_ROOTS: &[&str] = &["app", "lib", "source", "sources", "Sources", "src"];

/// How many nested source roots are looked into (`src` → `src/main` → …).
const MAX_SOURCE_DEPTH: usize = 3;

/// Minimum number of files before a dominant module is split.
const DOMINANT_MIN_FILES: usize = 10;

/// Maximum number of dominant-module split passes.
const SPLIT_PASSES: usize = 2;

/// A module inferred from the repository layout.
#[derive(Debug, Clone, PartialEq)]
pub struct InferredModule {
    /// Stable identifier: the module directory, or [`ROOT_MODULE`].
    pub id: String,
    /// Display name, unique among the modules.
    pub name: String,
    /// Repository-relative module directory (empty for the root module).
    pub path: String,
    /// How the boundary was established.
    pub kind: ModuleKind,
    /// Confidence of the boundary.
    pub confidence: Confidence,
    /// Evidence for the boundary.
    pub evidence: Vec<Evidence>,
}

/// Modules and the assignment of files to them.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ModuleMap {
    /// Modules sorted by identifier.
    pub modules: Vec<InferredModule>,
    /// Index into `modules` for every input file, in input order.
    pub assignment: Vec<usize>,
}

impl ModuleMap {
    /// Index of the module with identifier `id`.
    pub fn index_of(&self, id: &str) -> Option<usize> {
        self.modules
            .binary_search_by(|module| module.id.as_str().cmp(id))
            .ok()
    }

    /// Index of the module containing the directory or file `path`: the module whose
    /// directory is the longest prefix of `path`.
    pub fn index_containing(&self, path: &str) -> Option<usize> {
        self.modules
            .iter()
            .enumerate()
            .filter(|(_, module)| paths::is_within(path, &module.path))
            .max_by_key(|(_, module)| module.path.len())
            .map(|(index, _)| index)
    }
}

/// A module under construction.
#[derive(Debug, Clone)]
struct Draft {
    path: String,
    kind: ModuleKind,
    package: Option<usize>,
    files: Vec<usize>,
}

impl Draft {
    fn directory(path: String, files: Vec<usize>) -> Self {
        let kind = if path.is_empty() {
            ModuleKind::Root
        } else {
            ModuleKind::Directory
        };
        Self {
            path,
            kind,
            package: None,
            files,
        }
    }
}

/// Returns `path` relative to the directory `base`. `path` must be inside `base`.
fn relative<'p>(path: &'p str, base: &str) -> &'p str {
    if base.is_empty() {
        path
    } else {
        path.get(base.len() + 1..).unwrap_or_default()
    }
}

fn child_path(base: &str, child: &str) -> String {
    if base.is_empty() {
        child.to_owned()
    } else {
        format!("{base}/{child}")
    }
}

/// Descends from `base` while every file lives below one single child directory.
fn collapse(paths: &[&str], base: &str, files: &[usize]) -> String {
    let mut base = base.to_owned();
    loop {
        let mut only_child: Option<&str> = None;
        for &file in files {
            let Some((first, _)) = relative(paths[file], &base).split_once('/') else {
                return base;
            };
            match only_child {
                None => only_child = Some(first),
                Some(child) if child == first => {}
                Some(_) => return base,
            }
        }
        match only_child {
            Some(child) => base = child_path(&base, child),
            None => return base,
        }
    }
}

/// Splits `files` into the files directly inside `base` and those below each child.
fn group_by_child<'p>(
    paths: &[&'p str],
    base: &str,
    files: &[usize],
) -> (Vec<usize>, BTreeMap<&'p str, Vec<usize>>) {
    let mut direct = Vec::new();
    let mut children: BTreeMap<&'p str, Vec<usize>> = BTreeMap::new();
    for &file in files {
        match relative(paths[file], base).split_once('/') {
            None => direct.push(file),
            Some((child, _)) => children.entry(child).or_default().push(file),
        }
    }
    (direct, children)
}

/// Groups `files`, all inside `base`, into directory modules.
fn partition(paths: &[&str], base: &str, files: &[usize], depth: usize, out: &mut Vec<Draft>) {
    let base = collapse(paths, base, files);
    let (direct, children) = group_by_child(paths, &base, files);
    if !direct.is_empty() {
        out.push(Draft::directory(base.clone(), direct));
    }
    let base_name = paths::file_name(&base);
    for (child, members) in children {
        let path = child_path(&base, child);
        let source_root = SOURCE_ROOTS.contains(&child)
            || (base_name == "src" && matches!(child, "main" | "test"));
        if source_root && depth < MAX_SOURCE_DEPTH {
            partition(paths, &path, &members, depth + 1, out);
        } else if GROUPING_DIRS.contains(&child) {
            let (direct, grandchildren) = group_by_child(paths, &path, &members);
            if !direct.is_empty() {
                out.push(Draft::directory(path.clone(), direct));
            }
            for (grandchild, members) in grandchildren {
                out.push(Draft::directory(child_path(&path, grandchild), members));
            }
        } else {
            out.push(Draft::directory(path, members));
        }
    }
}

/// Splits modules that hold at least half of all files into their subdirectories.
fn split_dominant(paths: &[&str], mut drafts: Vec<Draft>) -> Vec<Draft> {
    for _ in 0..SPLIT_PASSES {
        let total: usize = drafts.iter().map(|draft| draft.files.len()).sum();
        let mut changed = false;
        let mut next = Vec::with_capacity(drafts.len());
        for draft in drafts {
            if draft.files.len() >= DOMINANT_MIN_FILES && draft.files.len() * 2 >= total {
                let mut parts = Vec::new();
                partition(paths, &draft.path, &draft.files, 0, &mut parts);
                if parts.len() >= 2 {
                    for mut part in parts {
                        // The package's own directory keeps the package identity.
                        if draft.kind == ModuleKind::Package && part.path == draft.path {
                            part.kind = ModuleKind::Package;
                            part.package = draft.package;
                        }
                        next.push(part);
                    }
                    changed = true;
                    continue;
                }
            }
            next.push(draft);
        }
        drafts = next;
        if !changed {
            break;
        }
    }
    drafts
}

/// Gives every module a short display name, extending directory names with parent
/// components until the names are unique.
fn assign_names(modules: &mut [InferredModule], packages: &[Option<&PackageBoundary>]) {
    let mut components = vec![1usize; modules.len()];
    loop {
        let names: Vec<String> = modules
            .iter()
            .zip(packages)
            .zip(&components)
            .map(|((module, package), &count)| match (module.kind, package) {
                (ModuleKind::Root, _) => ROOT_MODULE.to_owned(),
                (ModuleKind::Package, Some(package)) => package.name.clone(),
                _ => {
                    let parts: Vec<&str> = module.path.split('/').collect();
                    parts[parts.len().saturating_sub(count)..].join("/")
                }
            })
            .collect();
        let mut seen: HashMap<&str, Vec<usize>> = HashMap::new();
        for (index, name) in names.iter().enumerate() {
            seen.entry(name.as_str()).or_default().push(index);
        }
        let mut changed = false;
        for indices in seen.values().filter(|indices| indices.len() > 1) {
            for &index in indices {
                let depth = paths::depth(&modules[index].path);
                if modules[index].kind == ModuleKind::Directory && components[index] < depth {
                    components[index] += 1;
                    changed = true;
                }
            }
        }
        if !changed {
            for (module, name) in modules.iter_mut().zip(names) {
                module.name = name;
            }
            return;
        }
    }
}

/// Infers modules for `files` (repository-relative paths) given the declared packages.
///
/// Files are assigned to the deepest package that contains them; a package at the
/// repository root does not count as a boundary, because it contains everything.
pub fn infer_modules(files: &[&str], packages: &[PackageBoundary]) -> ModuleMap {
    // Deduplicate package roots (two manifests can share a directory).
    let mut package_roots: BTreeMap<&str, usize> = BTreeMap::new();
    for (index, package) in packages.iter().enumerate() {
        if !package.path.is_empty() {
            package_roots.entry(package.path.as_str()).or_insert(index);
        }
    }

    let mut by_package: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    let mut unpackaged = Vec::new();
    for (file, path) in files.iter().enumerate() {
        let owner = paths::ancestors(path)
            .into_iter()
            .rev()
            .find_map(|dir| package_roots.get(dir).copied());
        match owner {
            Some(package) => by_package.entry(package).or_default().push(file),
            None => unpackaged.push(file),
        }
    }

    let mut drafts = Vec::new();
    for (package, members) in by_package {
        drafts.push(Draft {
            path: packages[package].path.clone(),
            kind: ModuleKind::Package,
            package: Some(package),
            files: members,
        });
    }
    if !unpackaged.is_empty() {
        partition(files, "", &unpackaged, 0, &mut drafts);
    }
    let mut drafts = split_dominant(files, drafts);

    // Merge drafts that ended up with the same directory (defensive; normally impossible).
    drafts.sort_by(|a, b| a.path.cmp(&b.path));
    let mut merged: Vec<Draft> = Vec::with_capacity(drafts.len());
    for draft in drafts {
        match merged.last_mut() {
            Some(last) if last.path == draft.path => {
                last.files.extend(draft.files);
                if draft.kind == ModuleKind::Package {
                    last.kind = ModuleKind::Package;
                    last.package = draft.package;
                }
            }
            _ => merged.push(draft),
        }
    }

    let mut modules = Vec::with_capacity(merged.len());
    let mut module_packages = Vec::with_capacity(merged.len());
    for draft in &merged {
        let count = draft.files.len();
        let plural = if count == 1 { "" } else { "s" };
        let package = draft.package.map(|index| &packages[index]);
        let (confidence, evidence) = match (draft.kind, package) {
            (ModuleKind::Package, Some(package)) => (
                Confidence::High,
                Evidence::file(&package.manifest)
                    .with_note(format!("{} package manifest", package.ecosystem)),
            ),
            (ModuleKind::Root, _) => (
                Confidence::Medium,
                Evidence::directory("")
                    .with_note(format!("{count} code file{plural} at the repository root")),
            ),
            _ => (
                Confidence::Medium,
                Evidence::directory(&draft.path)
                    .with_note(format!("{count} code file{plural} grouped by directory")),
            ),
        };
        modules.push(InferredModule {
            id: if draft.path.is_empty() {
                ROOT_MODULE.to_owned()
            } else {
                draft.path.clone()
            },
            name: String::new(),
            path: draft.path.clone(),
            kind: draft.kind,
            confidence,
            evidence: vec![evidence],
        });
        module_packages.push(package);
    }
    assign_names(&mut modules, &module_packages);

    // Sort by identifier and record each file's module.
    let mut order: Vec<usize> = (0..modules.len()).collect();
    order.sort_by(|&a, &b| modules[a].id.cmp(&modules[b].id));
    let mut position = vec![0usize; modules.len()];
    for (new_index, &old_index) in order.iter().enumerate() {
        position[old_index] = new_index;
    }
    let mut assignment = vec![0usize; files.len()];
    for (old_index, draft) in merged.iter().enumerate() {
        for &file in &draft.files {
            assignment[file] = position[old_index];
        }
    }
    let mut slots: Vec<Option<InferredModule>> = modules.into_iter().map(Some).collect();
    let modules = order
        .iter()
        .filter_map(|&index| slots[index].take())
        .collect();
    ModuleMap {
        modules,
        assignment,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn package(name: &str, path: &str) -> PackageBoundary {
        PackageBoundary {
            name: name.into(),
            path: path.into(),
            ecosystem: "cargo".into(),
            manifest: if path.is_empty() {
                "Cargo.toml".into()
            } else {
                format!("{path}/Cargo.toml")
            },
            internal_dependencies: Vec::new(),
        }
    }

    fn ids(map: &ModuleMap) -> Vec<&str> {
        map.modules
            .iter()
            .map(|module| module.id.as_str())
            .collect()
    }

    fn module_of<'m>(map: &'m ModuleMap, files: &[&str], path: &str) -> &'m str {
        let index = files.iter().position(|file| *file == path).unwrap();
        &map.modules[map.assignment[index]].id
    }

    #[test]
    fn single_crate_modules_follow_source_directories() {
        let files = [
            "build.rs",
            "src/main.rs",
            "src/lib.rs",
            "src/parser/mod.rs",
            "src/parser/lexer.rs",
            "src/codegen/emit.rs",
            "tests/cli.rs",
        ];
        let map = infer_modules(&files, &[package("tool", "")]);
        assert_eq!(
            ids(&map),
            vec![ROOT_MODULE, "src", "src/codegen", "src/parser", "tests"]
        );
        assert_eq!(module_of(&map, &files, "src/parser/lexer.rs"), "src/parser");
        assert_eq!(module_of(&map, &files, "build.rs"), ROOT_MODULE);
        assert_eq!(map.modules[0].kind, ModuleKind::Root);
        assert_eq!(map.modules[2].name, "codegen");
    }

    #[test]
    fn workspace_packages_are_modules() {
        let files = [
            "crates/a/src/lib.rs",
            "crates/a/src/x.rs",
            "crates/b/src/lib.rs",
            "xtask/src/main.rs",
            "scripts/release.py",
        ];
        let packages = [package("alpha", "crates/a"), package("beta", "crates/b")];
        let map = infer_modules(&files, &packages);
        assert_eq!(ids(&map), vec!["crates/a", "crates/b", "scripts", "xtask"]);
        assert_eq!(map.modules[0].kind, ModuleKind::Package);
        assert_eq!(map.modules[0].name, "alpha");
        assert_eq!(map.modules[0].confidence, Confidence::High);
        assert_eq!(map.modules[3].kind, ModuleKind::Directory);
    }

    #[test]
    fn collapses_namespace_directories() {
        let files = [
            "src/main/java/com/example/app/service/A.java",
            "src/main/java/com/example/app/model/B.java",
            "src/test/java/com/example/app/service/ATest.java",
        ];
        let map = infer_modules(&files, &[]);
        assert_eq!(
            ids(&map),
            vec![
                "src/main/java/com/example/app/model",
                "src/main/java/com/example/app/service",
                "src/test/java/com/example/app/service",
            ]
        );
        let names: Vec<_> = map.modules.iter().map(|m| m.name.as_str()).collect();
        assert_eq!(names[0], "model");
        assert_ne!(names[1], names[2]);
    }

    #[test]
    fn grouping_directories_contribute_children() {
        let files = [
            "main.go",
            "cmd/server/main.go",
            "internal/auth/auth.go",
            "internal/db/db.go",
            "internal/version.go",
        ];
        let map = infer_modules(&files, &[]);
        assert_eq!(
            ids(&map),
            vec![
                ROOT_MODULE,
                "cmd/server",
                "internal",
                "internal/auth",
                "internal/db"
            ]
        );
    }

    #[test]
    fn dominant_modules_are_split() {
        let mut files: Vec<String> = (0..6).map(|i| format!("mypkg/core/m{i}.py")).collect();
        files.extend((0..6).map(|i| format!("mypkg/io/m{i}.py")));
        files.push("mypkg/__init__.py".into());
        files.push("tests/test_core.py".into());
        let refs: Vec<&str> = files.iter().map(String::as_str).collect();
        let map = infer_modules(&refs, &[]);
        assert_eq!(ids(&map), vec!["mypkg", "mypkg/core", "mypkg/io", "tests"]);
        assert_eq!(module_of(&map, &refs, "mypkg/io/m3.py"), "mypkg/io");
    }

    #[test]
    fn dominant_packages_keep_their_identity() {
        let mut files: Vec<String> = (0..6).map(|i| format!("app/src/ui/v{i}.ts")).collect();
        files.extend((0..6).map(|i| format!("app/src/api/a{i}.ts")));
        files.push("app/vite.config.ts".into());
        files.push("tools/gen.ts".into());
        let refs: Vec<&str> = files.iter().map(String::as_str).collect();
        let map = infer_modules(&refs, &[package("web", "app")]);
        assert_eq!(ids(&map), vec!["app", "app/src/api", "app/src/ui", "tools"]);
        assert_eq!(map.modules[0].kind, ModuleKind::Package);
        assert_eq!(map.modules[0].name, "web");
        assert_eq!(map.index_of("app/src/ui"), Some(2));
        assert_eq!(map.index_containing("app/src/ui/new.ts"), Some(2));
        assert_eq!(map.index_containing("app/README.md"), Some(0));
    }

    #[test]
    fn empty_input_has_no_modules() {
        let map = infer_modules(&[], &[]);
        assert!(map.modules.is_empty());
        assert!(map.assignment.is_empty());
    }
}
