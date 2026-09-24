//! Import resolution: mapping import specifiers to repository files or external packages.
//!
//! Resolution is static and per language. It understands relative paths, module systems
//! (Rust crates and modules, Python packages, Go modules, JVM packages, C# namespaces, PHP
//! namespaces), workspace package names, and common aliases such as `@/`. Imports whose
//! target is computed at runtime, configured through custom search paths, or produced by
//! code generation are invisible.

use std::collections::{HashMap, HashSet};

use repodna_core::confidence::Confidence;
use repodna_core::model::architecture::PackageBoundary;
use repodna_core::paths;
use repodna_parser::RawImport;
use repodna_parser::spec::ImportKind;

use crate::SourceFile;

/// Maximum number of files linked by an import of a whole package, module, or namespace.
pub const MAX_PACKAGE_TARGETS: usize = 20;

/// The outcome of resolving one import.
#[derive(Debug, Clone, PartialEq)]
pub enum Resolution {
    /// Repository files.
    Internal {
        /// Target file indices.
        targets: Vec<usize>,
        /// Confidence that the targets are right.
        confidence: Confidence,
        /// `true` when a whole package or namespace was imported (Go packages, Swift
        /// modules, C# namespaces, JVM wildcards). Such imports say which *module* is used
        /// but not which file, so they only produce module-level edges.
        package_level: bool,
    },
    /// A package from outside the repository.
    External {
        /// Package or module name.
        name: String,
        /// Ecosystem implied by the importing language.
        ecosystem: &'static str,
    },
    /// Standard library, runtime or virtual modules, URLs, and dynamic paths. Not counted as
    /// resolved or unresolved.
    Ignored,
    /// Looks like a repository import, but no file matched.
    Unresolved,
}

fn internal(file: usize, confidence: Confidence) -> Resolution {
    Resolution::Internal {
        targets: vec![file],
        confidence,
        package_level: false,
    }
}

fn package_level(targets: Vec<usize>, confidence: Confidence) -> Resolution {
    if targets.is_empty() {
        Resolution::Unresolved
    } else {
        Resolution::Internal {
            targets,
            confidence,
            package_level: true,
        }
    }
}

fn external(name: &str, ecosystem: &'static str) -> Resolution {
    Resolution::External {
        name: name.to_owned(),
        ecosystem,
    }
}

fn child_path(base: &str, child: &str) -> String {
    if base.is_empty() {
        child.to_owned()
    } else if child.is_empty() {
        base.to_owned()
    } else {
        format!("{base}/{child}")
    }
}

/// Removes a query string or fragment (`./a.svg?raw`, `./b.css#x`).
fn strip_query(specifier: &str) -> &str {
    specifier.split(['?', '#']).next().unwrap_or_default()
}

fn is_relative(specifier: &str) -> bool {
    specifier == "."
        || specifier == ".."
        || specifier.starts_with("./")
        || specifier.starts_with("../")
}

/// The package part of an npm specifier: `@scope/name` or `name`.
fn npm_package_name(specifier: &str) -> &str {
    let segments = if specifier.starts_with('@') { 2 } else { 1 };
    first_segments(specifier, segments, '/')
}

/// The first `count` segments of `text` split by `separator`.
fn first_segments(text: &str, count: usize, separator: char) -> &str {
    match text.match_indices(separator).nth(count.saturating_sub(1)) {
        Some((index, _)) => &text[..index],
        None => text,
    }
}

fn normalize_name(name: &str) -> String {
    name.to_ascii_lowercase().replace('-', "_")
}

/// Rust files that are crate roots: `mod x;` in them looks in their own directory.
fn is_rust_crate_root(path: &str) -> bool {
    let name = paths::file_name(path);
    matches!(name, "lib.rs" | "main.rs" | "build.rs")
        || matches!(
            paths::file_name(paths::parent(path)),
            "bin" | "benches" | "examples" | "tests"
        )
}

/// The directory in which `mod x;` declared in `path` looks for `x.rs` or `x/mod.rs`.
fn rust_child_dir(path: &str) -> String {
    let parent = paths::parent(path);
    if is_rust_crate_root(path) || paths::file_name(path) == "mod.rs" {
        parent.to_owned()
    } else {
        child_path(parent, paths::file_stem(path))
    }
}

/// The source directory of the crate containing `path`: the nearest `src` ancestor, or the
/// file's own directory for integration tests, benches, examples, and build scripts.
fn rust_crate_src(path: &str) -> String {
    paths::ancestors(path)
        .into_iter()
        .rev()
        .find(|dir| paths::file_name(dir) == "src")
        .map_or_else(|| paths::parent(path).to_owned(), str::to_owned)
}

/// The module path of a Rust file relative to its crate source directory.
fn rust_module_path<'p>(path: &'p str, src: &str) -> Vec<&'p str> {
    let relative = if src.is_empty() {
        path
    } else {
        path.strip_prefix(src)
            .and_then(|rest| rest.strip_prefix('/'))
            .unwrap_or(path)
    };
    let relative = relative.strip_suffix(".rs").unwrap_or(relative);
    let mut parts: Vec<&str> = relative.split('/').collect();
    let crate_root =
        parts.len() == 1 && (matches!(parts[0], "lib" | "main") || paths::file_name(src) != "src");
    if crate_root || parts.last() == Some(&"mod") {
        parts.pop();
    }
    parts
}

/// Top-level modules of the Python standard library.
const PYTHON_STDLIB: &[&str] = &[
    "__future__",
    "_thread",
    "abc",
    "aifc",
    "argparse",
    "array",
    "ast",
    "asynchat",
    "asyncio",
    "asyncore",
    "atexit",
    "audioop",
    "base64",
    "bdb",
    "binascii",
    "bisect",
    "builtins",
    "bz2",
    "cProfile",
    "calendar",
    "cgi",
    "cgitb",
    "chunk",
    "cmath",
    "cmd",
    "code",
    "codecs",
    "codeop",
    "collections",
    "colorsys",
    "compileall",
    "concurrent",
    "configparser",
    "contextlib",
    "contextvars",
    "copy",
    "copyreg",
    "crypt",
    "csv",
    "ctypes",
    "curses",
    "dataclasses",
    "datetime",
    "dbm",
    "decimal",
    "difflib",
    "dis",
    "distutils",
    "doctest",
    "email",
    "encodings",
    "ensurepip",
    "enum",
    "errno",
    "faulthandler",
    "fcntl",
    "filecmp",
    "fileinput",
    "fnmatch",
    "fractions",
    "ftplib",
    "functools",
    "gc",
    "getopt",
    "getpass",
    "gettext",
    "glob",
    "graphlib",
    "grp",
    "gzip",
    "hashlib",
    "heapq",
    "hmac",
    "html",
    "http",
    "idlelib",
    "imaplib",
    "imghdr",
    "imp",
    "importlib",
    "inspect",
    "io",
    "ipaddress",
    "itertools",
    "json",
    "keyword",
    "lib2to3",
    "linecache",
    "locale",
    "logging",
    "lzma",
    "mailbox",
    "mailcap",
    "marshal",
    "math",
    "mimetypes",
    "mmap",
    "modulefinder",
    "msilib",
    "msvcrt",
    "multiprocessing",
    "netrc",
    "nis",
    "nntplib",
    "ntpath",
    "numbers",
    "opcode",
    "operator",
    "optparse",
    "os",
    "ossaudiodev",
    "pathlib",
    "pdb",
    "pickle",
    "pickletools",
    "pipes",
    "pkgutil",
    "platform",
    "plistlib",
    "poplib",
    "posix",
    "posixpath",
    "pprint",
    "profile",
    "pstats",
    "pty",
    "pwd",
    "py_compile",
    "pyclbr",
    "pydoc",
    "queue",
    "quopri",
    "random",
    "re",
    "readline",
    "reprlib",
    "resource",
    "rlcompleter",
    "runpy",
    "sched",
    "secrets",
    "select",
    "selectors",
    "shelve",
    "shlex",
    "shutil",
    "signal",
    "site",
    "smtpd",
    "smtplib",
    "sndhdr",
    "socket",
    "socketserver",
    "spwd",
    "sqlite3",
    "ssl",
    "stat",
    "statistics",
    "string",
    "stringprep",
    "struct",
    "subprocess",
    "sunau",
    "symtable",
    "sys",
    "sysconfig",
    "syslog",
    "tabnanny",
    "tarfile",
    "telnetlib",
    "tempfile",
    "termios",
    "textwrap",
    "threading",
    "time",
    "timeit",
    "tkinter",
    "token",
    "tokenize",
    "tomllib",
    "trace",
    "traceback",
    "tracemalloc",
    "tty",
    "turtle",
    "turtledemo",
    "types",
    "typing",
    "unicodedata",
    "unittest",
    "urllib",
    "uu",
    "uuid",
    "venv",
    "warnings",
    "wave",
    "weakref",
    "webbrowser",
    "winreg",
    "winsound",
    "wsgiref",
    "xdrlib",
    "xml",
    "xmlrpc",
    "zipapp",
    "zipfile",
    "zipimport",
    "zlib",
    "zoneinfo",
];

/// Directories that are commonly on the Python import path.
const PYTHON_ROOT_DIRS: &[&str] = &["lib", "python", "src"];

/// Node.js built-in modules.
const NODE_BUILTINS: &[&str] = &[
    "assert",
    "async_hooks",
    "buffer",
    "child_process",
    "cluster",
    "console",
    "constants",
    "crypto",
    "dgram",
    "diagnostics_channel",
    "dns",
    "domain",
    "events",
    "fs",
    "http",
    "http2",
    "https",
    "inspector",
    "module",
    "net",
    "os",
    "path",
    "perf_hooks",
    "process",
    "punycode",
    "querystring",
    "readline",
    "repl",
    "stream",
    "string_decoder",
    "sys",
    "timers",
    "tls",
    "trace_events",
    "tty",
    "url",
    "util",
    "v8",
    "vm",
    "wasi",
    "worker_threads",
    "zlib",
];

/// Extensions tried, in order, for extension-less JavaScript and TypeScript imports.
const JS_EXTENSIONS: &[&str] = &[
    "ts", "tsx", "js", "jsx", "mjs", "cjs", "mts", "cts", "d.ts", "vue", "svelte", "json",
];

/// Ruby standard library features commonly passed to `require`.
const RUBY_STDLIB: &[&str] = &[
    "English",
    "abbrev",
    "base64",
    "benchmark",
    "bigdecimal",
    "bundler",
    "cgi",
    "coverage",
    "csv",
    "date",
    "delegate",
    "digest",
    "drb",
    "erb",
    "etc",
    "fcntl",
    "fiber",
    "fiddle",
    "fileutils",
    "find",
    "forwardable",
    "getoptlong",
    "io",
    "ipaddr",
    "json",
    "logger",
    "matrix",
    "minitest",
    "monitor",
    "mutex_m",
    "net",
    "objspace",
    "observer",
    "open-uri",
    "open3",
    "openssl",
    "optparse",
    "ostruct",
    "pathname",
    "pp",
    "prettyprint",
    "prime",
    "psych",
    "racc",
    "rbconfig",
    "rdoc",
    "readline",
    "resolv",
    "ripper",
    "rubygems",
    "securerandom",
    "set",
    "shellwords",
    "singleton",
    "socket",
    "stringio",
    "strscan",
    "syslog",
    "tempfile",
    "test",
    "thread",
    "time",
    "timeout",
    "tmpdir",
    "tsort",
    "un",
    "uri",
    "weakref",
    "yaml",
    "zlib",
];

/// Apple and Swift system modules.
const SWIFT_SYSTEM: &[&str] = &[
    "ARKit",
    "AVFoundation",
    "AVKit",
    "Accelerate",
    "AppKit",
    "AuthenticationServices",
    "CloudKit",
    "Combine",
    "Contacts",
    "CoreBluetooth",
    "CoreData",
    "CoreFoundation",
    "CoreGraphics",
    "CoreHaptics",
    "CoreImage",
    "CoreLocation",
    "CoreML",
    "CoreMotion",
    "CoreText",
    "CryptoKit",
    "Darwin",
    "Dispatch",
    "EventKit",
    "Foundation",
    "FoundationNetworking",
    "GameKit",
    "Glibc",
    "HealthKit",
    "LocalAuthentication",
    "MapKit",
    "MessageUI",
    "Metal",
    "MetalKit",
    "Network",
    "OSLog",
    "ObjectiveC",
    "Observation",
    "Photos",
    "PhotosUI",
    "QuartzCore",
    "RealityKit",
    "RegexBuilder",
    "SafariServices",
    "SceneKit",
    "Security",
    "SpriteKit",
    "StoreKit",
    "Swift",
    "SwiftData",
    "SwiftUI",
    "System",
    "Testing",
    "UIKit",
    "UserNotifications",
    "Vision",
    "WebKit",
    "WidgetKit",
    "XCTest",
    "os",
];

/// JVM package prefixes provided by the platform or the language runtime.
const JVM_BUILTIN: &[&str] = &[
    "android", "com.sun", "dalvik", "groovy", "java", "javax", "jdk", "kotlin", "scala", "sun",
];

/// Maps import specifiers to repository files, workspace packages, or external packages.
#[derive(Debug)]
pub struct Resolver<'a> {
    files: &'a [SourceFile<'a>],
    by_path: HashMap<&'a str, usize>,
    by_name: HashMap<&'a str, Vec<usize>>,
    by_dir: HashMap<&'a str, Vec<usize>>,
    sorted: Vec<usize>,
    python: HashMap<String, Vec<(usize, bool)>>,
    jvm_types: HashMap<String, Vec<usize>>,
    jvm_packages: HashMap<String, Vec<usize>>,
    jvm_roots: HashSet<String>,
    cs_namespaces: HashMap<String, Vec<usize>>,
    cs_roots: HashSet<String>,
    php_classes: HashMap<String, Vec<usize>>,
    php_roots: HashSet<String>,
    rust_crates: HashMap<String, String>,
    npm_packages: HashMap<String, String>,
    npm_roots: Vec<String>,
    go_modules: Vec<(String, String)>,
    dart_packages: HashMap<String, String>,
    swift_targets: HashMap<String, String>,
    declared: HashMap<&'static str, HashSet<String>>,
}

impl<'a> Resolver<'a> {
    /// Indexes `files` and the workspace `packages`.
    pub fn new(files: &'a [SourceFile<'a>], packages: &[PackageBoundary]) -> Self {
        let mut resolver = Self {
            files,
            by_path: HashMap::with_capacity(files.len()),
            by_name: HashMap::new(),
            by_dir: HashMap::new(),
            sorted: (0..files.len()).collect(),
            python: HashMap::new(),
            jvm_types: HashMap::new(),
            jvm_packages: HashMap::new(),
            jvm_roots: HashSet::new(),
            cs_namespaces: HashMap::new(),
            cs_roots: HashSet::new(),
            php_classes: HashMap::new(),
            php_roots: HashSet::new(),
            rust_crates: HashMap::new(),
            npm_packages: HashMap::new(),
            npm_roots: Vec::new(),
            go_modules: Vec::new(),
            dart_packages: HashMap::new(),
            swift_targets: HashMap::new(),
            declared: HashMap::new(),
        };
        resolver
            .sorted
            .sort_by(|&a, &b| files[a].path.cmp(files[b].path));
        for position in 0..resolver.sorted.len() {
            resolver.index_file(resolver.sorted[position]);
        }
        for package in packages {
            let dir = package.path.clone();
            match package.ecosystem.as_str() {
                "cargo" => {
                    resolver
                        .rust_crates
                        .entry(package.name.replace('-', "_"))
                        .or_insert_with(|| child_path(&dir, "src"));
                }
                "npm" => {
                    resolver
                        .npm_packages
                        .entry(package.name.clone())
                        .or_insert_with(|| dir.clone());
                    resolver.npm_roots.push(dir);
                }
                "go" => resolver.go_modules.push((package.name.clone(), dir)),
                "pub" => {
                    resolver
                        .dart_packages
                        .entry(package.name.clone())
                        .or_insert(dir);
                }
                _ => {}
            }
        }
        // Deepest npm roots and longest Go module paths first.
        resolver
            .npm_roots
            .sort_by(|a, b| b.len().cmp(&a.len()).then_with(|| a.cmp(b)));
        resolver
            .go_modules
            .sort_by(|a, b| b.0.len().cmp(&a.0.len()).then_with(|| a.cmp(b)));
        resolver
    }

    fn index_file(&mut self, index: usize) {
        let files = self.files;
        let file = &files[index];
        let path = file.path;
        self.by_path.insert(path, index);
        self.by_name
            .entry(paths::file_name(path))
            .or_default()
            .push(index);
        self.by_dir
            .entry(paths::parent(path))
            .or_default()
            .push(index);
        let package = file
            .analysis
            .and_then(|analysis| analysis.package.as_deref());
        match file.language.unwrap_or_default() {
            "python" => self.index_python(index, path),
            "java" | "kotlin" | "scala" | "groovy" => {
                let stem = paths::file_stem(path);
                let (type_name, package): (String, String) = match package {
                    Some(package) => (format!("{package}.{stem}"), package.to_owned()),
                    None => (stem.to_owned(), String::new()),
                };
                self.jvm_types.entry(type_name).or_default().push(index);
                if !package.is_empty() {
                    if package.contains('.') {
                        self.jvm_roots
                            .insert(first_segments(&package, 2, '.').to_owned());
                    }
                    self.jvm_packages.entry(package).or_default().push(index);
                }
            }
            "csharp" => {
                if let Some(namespace) = package {
                    self.cs_roots
                        .insert(first_segments(namespace, 1, '.').to_owned());
                    self.cs_namespaces
                        .entry(namespace.to_owned())
                        .or_default()
                        .push(index);
                }
            }
            "php" => {
                let stem = paths::file_stem(path);
                let class = match package {
                    Some(namespace) => {
                        self.php_roots
                            .insert(first_segments(namespace, 1, '\\').to_owned());
                        format!("{namespace}\\{stem}")
                    }
                    None => stem.to_owned(),
                };
                self.php_classes.entry(class).or_default().push(index);
            }
            "swift" => {
                let parts: Vec<&str> = path.split('/').collect();
                if let Some(position) = parts
                    .iter()
                    .position(|part| matches!(*part, "Sources" | "Tests"))
                    .filter(|&position| position + 2 < parts.len())
                {
                    self.swift_targets
                        .entry(parts[position + 1].to_owned())
                        .or_insert_with(|| parts[..=position + 1].join("/"));
                }
            }
            _ => {}
        }
    }

    fn index_python(&mut self, index: usize, path: &str) {
        let Some(module) = path
            .strip_suffix(".py")
            .or_else(|| path.strip_suffix(".pyi"))
        else {
            return;
        };
        let mut components: Vec<&str> = module.split('/').collect();
        if components.last() == Some(&"__init__") {
            components.pop();
        }
        for start in 0..components.len() {
            let anchored = start == 0 || PYTHON_ROOT_DIRS.contains(&components[start - 1]);
            self.python
                .entry(components[start..].join("."))
                .or_default()
                .push((index, anchored));
        }
    }

    /// Records a dependency declared in a manifest, which helps classify ambiguous imports.
    pub fn declare_dependency(&mut self, ecosystem: &str, name: &str) {
        let ecosystem: &'static str = match ecosystem {
            "cargo" => "cargo",
            "npm" => "npm",
            "pypi" => "pypi",
            "go" => "go",
            "maven" | "gradle" => "maven",
            "nuget" => "nuget",
            "composer" => "composer",
            "rubygems" => "rubygems",
            "pub" => "pub",
            "swiftpm" => "swiftpm",
            _ => return,
        };
        let name = if ecosystem == "go" {
            name.to_owned()
        } else {
            normalize_name(name)
        };
        self.declared.entry(ecosystem).or_default().insert(name);
    }

    fn is_declared(&self, ecosystem: &str, name: &str) -> bool {
        self.declared
            .get(ecosystem)
            .is_some_and(|names| names.contains(&normalize_name(name)))
    }

    fn file(&self, path: &str) -> Option<usize> {
        self.by_path.get(path).copied()
    }

    fn first<I>(&self, candidates: I) -> Option<usize>
    where
        I: IntoIterator,
        I::Item: AsRef<str>,
    {
        candidates
            .into_iter()
            .find_map(|candidate| self.file(candidate.as_ref()))
    }

    /// Files inside the directory `dir`, recursively, in path order.
    fn files_within<'s>(&'s self, dir: &str) -> impl Iterator<Item = usize> + 's {
        let prefix = if dir.is_empty() {
            String::new()
        } else {
            format!("{dir}/")
        };
        let start = self
            .sorted
            .partition_point(|&index| self.files[index].path < prefix.as_str());
        self.sorted[start..]
            .iter()
            .copied()
            .take_while(move |&index| self.files[index].path.starts_with(&prefix))
    }

    /// Finds the file whose path ends with `suffix` at a directory boundary. Returns the
    /// shortest match and whether it was the only one.
    fn suffix_match(&self, suffix: &str) -> Option<(usize, bool)> {
        let suffix = suffix.trim_start_matches("./");
        let candidates = self.by_name.get(paths::file_name(suffix))?;
        let matching: Vec<usize> = candidates
            .iter()
            .copied()
            .filter(|&index| {
                let path = self.files[index].path;
                path == suffix
                    || (path.len() > suffix.len()
                        && path.ends_with(suffix)
                        && path.as_bytes()[path.len() - suffix.len() - 1] == b'/')
            })
            .collect();
        let best = matching
            .iter()
            .copied()
            .min_by_key(|&index| (self.files[index].path.len(), self.files[index].path))?;
        Some((best, matching.len() == 1))
    }

    /// Resolves `import`, found in file `from`.
    pub fn resolve(&self, from: usize, import: &RawImport) -> Resolution {
        match self.files[from].language.unwrap_or_default() {
            "rust" => self.rust(from, import),
            "python" => self.python(from, import),
            "javascript" | "typescript" | "vue" | "svelte" => self.javascript(from, import),
            "go" => self.go(import),
            "java" | "kotlin" | "scala" | "groovy" => self.jvm(import),
            "csharp" => self.csharp(import),
            "php" => self.php(from, import),
            "c" | "cpp" | "objective-c" => self.c_family(from, import),
            "ruby" => self.ruby(from, import),
            "dart" => self.dart(from, import),
            "swift" => self.swift(import),
            "html" | "css" | "scss" | "less" => self.web(from, import),
            _ => self.generic(from, import),
        }
    }

    /// Resolves a string literal that looks like a path to another repository file.
    pub fn resolve_reference(&self, from: usize, reference: &str) -> Option<(usize, Confidence)> {
        if reference.contains("://") {
            return None;
        }
        let path = self.files[from].path;
        if let Some(file) = paths::join(paths::parent(path), reference).and_then(|c| self.file(&c))
        {
            return Some((file, Confidence::Medium));
        }
        let trimmed = reference.trim_start_matches("./");
        if let Some(file) = self.file(trimmed) {
            return Some((file, Confidence::Medium));
        }
        if trimmed.contains('/')
            && let Some((file, true)) = self.suffix_match(trimmed)
        {
            return Some((file, Confidence::Low));
        }
        None
    }

    fn rust(&self, from: usize, import: &RawImport) -> Resolution {
        let path = self.files[from].path;
        let specifier = import.specifier.as_str();
        if import.kind == ImportKind::Module {
            let dir = rust_child_dir(path);
            return self
                .first([
                    child_path(&dir, &format!("{specifier}.rs")),
                    child_path(&dir, &format!("{specifier}/mod.rs")),
                ])
                .map_or(Resolution::Unresolved, |file| {
                    internal(file, Confidence::High)
                });
        }
        let segments: Vec<&str> = specifier.split("::").filter(|s| !s.is_empty()).collect();
        let Some(&first) = segments.first() else {
            return Resolution::Ignored;
        };
        let src = rust_crate_src(path);
        let current = rust_module_path(path, &src);
        let resolved = match first {
            "crate" => self.rust_in_crate(&src, &segments[1..]),
            "self" => {
                let mut full = current.clone();
                full.extend(&segments[1..]);
                self.rust_in_crate(&src, &full)
            }
            "super" => {
                let supers = segments.iter().take_while(|s| **s == "super").count();
                let mut full = current[..current.len().saturating_sub(supers)].to_vec();
                full.extend(&segments[supers..]);
                self.rust_in_crate(&src, &full)
            }
            "std" | "core" | "alloc" | "proc_macro" | "test" => return Resolution::Ignored,
            name => {
                if let Some(crate_src) = self.rust_crates.get(name) {
                    self.rust_in_crate(crate_src, &segments[1..])
                } else {
                    // Paths may also start with a child module of the current module.
                    let dir = rust_child_dir(path);
                    let child = self.first([
                        child_path(&dir, &format!("{name}.rs")),
                        child_path(&dir, &format!("{name}/mod.rs")),
                    ]);
                    if child.is_none() {
                        return external(name, "cargo");
                    }
                    let mut full = current.clone();
                    full.extend(&segments);
                    self.rust_in_crate(&src, &full)
                }
            }
        };
        resolved.map_or(Resolution::Unresolved, |file| {
            internal(file, Confidence::Medium)
        })
    }

    /// Finds the file defining the longest module prefix of `segments` in a crate.
    fn rust_in_crate(&self, src: &str, segments: &[&str]) -> Option<usize> {
        for count in (1..=segments.len()).rev() {
            let joined = segments[..count].join("/");
            if let Some(file) = self.first([
                child_path(src, &format!("{joined}.rs")),
                child_path(src, &format!("{joined}/mod.rs")),
            ]) {
                return Some(file);
            }
        }
        self.first([child_path(src, "lib.rs"), child_path(src, "main.rs")])
    }

    fn python_lookup(&self, dotted: &str, anchored_only: bool) -> Option<(usize, Confidence)> {
        let entries = self.python.get(dotted)?;
        let pick = |anchored: bool| {
            entries
                .iter()
                .filter(|(_, is_anchored)| *is_anchored == anchored)
                .map(|(index, _)| *index)
                .min_by_key(|&index| (self.files[index].path.len(), self.files[index].path))
        };
        if let Some(file) = pick(true) {
            let count = entries.iter().filter(|(_, anchored)| *anchored).count();
            let confidence = if count == 1 {
                Confidence::Medium
            } else {
                Confidence::Low
            };
            return Some((file, confidence));
        }
        if anchored_only {
            return None;
        }
        pick(false).map(|file| (file, Confidence::Low))
    }

    fn python(&self, from: usize, import: &RawImport) -> Resolution {
        let path = self.files[from].path;
        let specifier = import.specifier.as_str();
        if specifier.starts_with('.') {
            let dots = specifier.bytes().take_while(|&b| b == b'.').count();
            let rest = &specifier[dots..];
            let mut dir = paths::parent(path);
            for _ in 1..dots {
                if dir.is_empty() {
                    return Resolution::Unresolved;
                }
                dir = paths::parent(dir);
            }
            let base = child_path(dir, &rest.replace('.', "/"));
            let mut candidates = Vec::new();
            for name in &import.names {
                candidates.push(child_path(&base, &format!("{name}.py")));
                candidates.push(child_path(&base, &format!("{name}/__init__.py")));
            }
            if !rest.is_empty() {
                candidates.push(format!("{base}.py"));
            }
            candidates.push(child_path(&base, "__init__.py"));
            return self
                .first(candidates)
                .map_or(Resolution::Unresolved, |file| {
                    internal(file, Confidence::High)
                });
        }
        // Submodules named in `from x import y` first, then the module itself.
        let candidates: Vec<String> = import
            .names
            .iter()
            .map(|name| format!("{specifier}.{name}"))
            .chain([specifier.to_owned()])
            .collect();
        for candidate in &candidates {
            if let Some((file, confidence)) = self.python_lookup(candidate, true) {
                return internal(file, confidence);
            }
        }
        let top = specifier.split('.').next().unwrap_or(specifier);
        if PYTHON_STDLIB.binary_search(&top).is_ok() {
            return Resolution::Ignored;
        }
        if self.is_declared("pypi", top) {
            return external(top, "pypi");
        }
        for candidate in &candidates {
            if let Some((file, confidence)) = self.python_lookup(candidate, false) {
                return internal(file, confidence);
            }
        }
        if self.python_lookup(top, true).is_some() {
            return Resolution::Unresolved;
        }
        external(top, "pypi")
    }

    /// The directory of the npm package containing `path` (the repository root if none).
    fn npm_root_of(&self, path: &str) -> &str {
        self.npm_roots
            .iter()
            .find(|root| paths::is_within(path, root))
            .map_or("", String::as_str)
    }

    fn js_file(&self, base: &str) -> Option<usize> {
        if let Some(file) = self.file(base) {
            return Some(file);
        }
        // TypeScript sources imported by their compiled name (`./a.js` → `./a.ts`).
        for (compiled, sources) in [
            (".js", &[".ts", ".tsx"][..]),
            (".jsx", &[".tsx"][..]),
            (".mjs", &[".mts"][..]),
            (".cjs", &[".cts"][..]),
        ] {
            if let Some(stem) = base.strip_suffix(compiled)
                && let Some(file) = self.first(sources.iter().map(|ext| format!("{stem}{ext}")))
            {
                return Some(file);
            }
        }
        if !base.is_empty()
            && let Some(file) = self.first(JS_EXTENSIONS.iter().map(|ext| format!("{base}.{ext}")))
        {
            return Some(file);
        }
        let index = child_path(base, "index");
        self.first(JS_EXTENSIONS.iter().map(|ext| format!("{index}.{ext}")))
    }

    fn javascript(&self, from: usize, import: &RawImport) -> Resolution {
        let path = self.files[from].path;
        let specifier = strip_query(&import.specifier);
        if specifier.is_empty() {
            return Resolution::Ignored;
        }
        if is_relative(specifier) {
            return paths::join(paths::parent(path), specifier)
                .and_then(|base| self.js_file(&base))
                .map_or(Resolution::Unresolved, |file| {
                    internal(file, Confidence::High)
                });
        }
        let root = self.npm_root_of(path);
        if let Some(rest) = specifier.strip_prefix('/') {
            return [
                child_path(root, rest),
                child_path(root, &format!("public/{rest}")),
                child_path(root, &format!("src/{rest}")),
            ]
            .iter()
            .find_map(|base| self.js_file(base))
            .map_or(Resolution::Unresolved, |file| {
                internal(file, Confidence::Medium)
            });
        }
        if let Some(rest) = specifier
            .strip_prefix("@/")
            .or_else(|| specifier.strip_prefix("~/"))
        {
            return [
                child_path(root, &format!("src/{rest}")),
                child_path(root, rest),
            ]
            .iter()
            .find_map(|base| self.js_file(base))
            .map_or(Resolution::Unresolved, |file| {
                internal(file, Confidence::Medium)
            });
        }
        if specifier.starts_with("node:")
            || specifier.contains("://")
            || specifier.starts_with("data:")
        {
            return Resolution::Ignored;
        }
        let name = npm_package_name(specifier);
        if NODE_BUILTINS.binary_search(&name).is_ok() {
            return Resolution::Ignored;
        }
        if let Some(dir) = self.npm_packages.get(name) {
            let sub = specifier[name.len()..].trim_start_matches('/');
            let bases: Vec<String> = if sub.is_empty() {
                ["src/index", "index", "src/main", "lib/index", "src/lib"]
                    .iter()
                    .map(|base| child_path(dir, base))
                    .collect()
            } else {
                vec![child_path(dir, &format!("src/{sub}")), child_path(dir, sub)]
            };
            if let Some(file) = bases.iter().find_map(|base| self.js_file(base)) {
                return internal(file, Confidence::Medium);
            }
            return package_level(
                self.files_within(dir).take(MAX_PACKAGE_TARGETS).collect(),
                Confidence::Low,
            );
        }
        if specifier.contains(':') || specifier.starts_with('$') {
            // Virtual modules (`virtual:pwa`, `astro:content`) and framework aliases.
            return Resolution::Ignored;
        }
        external(name, "npm")
    }

    fn go(&self, import: &RawImport) -> Resolution {
        let specifier = import.specifier.as_str();
        for (module, root) in &self.go_modules {
            let rest = if specifier == module {
                Some("")
            } else {
                specifier
                    .strip_prefix(module.as_str())
                    .and_then(|rest| rest.strip_prefix('/'))
            };
            if let Some(rest) = rest {
                let dir = child_path(root, rest);
                let targets = self
                    .by_dir
                    .get(dir.as_str())
                    .map(|files| {
                        files
                            .iter()
                            .copied()
                            .filter(|&index| {
                                let path = self.files[index].path;
                                path.ends_with(".go") && !path.ends_with("_test.go")
                            })
                            .take(MAX_PACKAGE_TARGETS)
                            .collect()
                    })
                    .unwrap_or_default();
                return package_level(targets, Confidence::High);
            }
        }
        let host = specifier.split('/').next().unwrap_or(specifier);
        if !host.contains('.') {
            return Resolution::Ignored;
        }
        let declared = self.declared.get("go").and_then(|names| {
            names
                .iter()
                .filter(|name| {
                    specifier == name.as_str()
                        || specifier
                            .strip_prefix(name.as_str())
                            .is_some_and(|rest| rest.starts_with('/'))
                })
                .max_by_key(|name| name.len())
        });
        let name = match declared {
            Some(name) => name.as_str(),
            None if matches!(
                host,
                "github.com" | "gitlab.com" | "bitbucket.org" | "golang.org"
            ) =>
            {
                first_segments(specifier, 3, '/')
            }
            None => first_segments(specifier, 2, '/'),
        };
        external(name, "go")
    }

    fn jvm(&self, import: &RawImport) -> Resolution {
        let raw = import.specifier.as_str();
        let (specifier, wildcard) = match raw.strip_suffix(".*") {
            Some(package) => (package, true),
            None => (raw, false),
        };
        if !wildcard {
            // The type itself, then enclosing types for nested classes and static imports.
            let mut current = specifier;
            for depth in 0..3 {
                if let Some(files) = self.jvm_types.get(current) {
                    let confidence = if depth == 0 && files.len() == 1 {
                        Confidence::High
                    } else {
                        Confidence::Medium
                    };
                    return internal(files[0], confidence);
                }
                match current.rsplit_once('.') {
                    Some((head, _)) => current = head,
                    None => break,
                }
            }
        }
        // A whole package (wildcard), or a top-level function or property in a package.
        let package = if wildcard {
            Some(specifier)
        } else {
            specifier.rsplit_once('.').map(|(head, _)| head)
        };
        let files = package.and_then(|package| self.jvm_packages.get(package));
        if let Some(files) = files {
            return package_level(
                files.iter().copied().take(MAX_PACKAGE_TARGETS).collect(),
                Confidence::Low,
            );
        }
        let builtin = JVM_BUILTIN.iter().any(|prefix| {
            specifier == *prefix
                || specifier
                    .strip_prefix(prefix)
                    .is_some_and(|rest| rest.starts_with('.'))
        });
        if builtin {
            return Resolution::Ignored;
        }
        if self.jvm_roots.contains(first_segments(specifier, 2, '.')) {
            return Resolution::Unresolved;
        }
        external(first_segments(specifier, 3, '.'), "maven")
    }

    fn csharp(&self, import: &RawImport) -> Resolution {
        let specifier = import.specifier.as_str();
        let mut current = specifier;
        for depth in 0..2 {
            if let Some(files) = self.cs_namespaces.get(current) {
                let confidence = if depth == 0 {
                    Confidence::Medium
                } else {
                    Confidence::Low
                };
                return package_level(
                    files.iter().copied().take(MAX_PACKAGE_TARGETS).collect(),
                    confidence,
                );
            }
            match current.rsplit_once('.') {
                Some((head, _)) => current = head,
                None => break,
            }
        }
        let root = first_segments(specifier, 1, '.');
        if root == "System" {
            return Resolution::Ignored;
        }
        if self.cs_roots.contains(root) {
            return Resolution::Unresolved;
        }
        let declared = self.declared.get("nuget").and_then(|names| {
            let lower = specifier.to_ascii_lowercase();
            names
                .iter()
                .filter(|name| {
                    lower == name.as_str()
                        || lower
                            .strip_prefix(name.as_str())
                            .is_some_and(|rest| rest.starts_with('.'))
                })
                .max_by_key(|name| name.len())
                .map(|name| &specifier[..name.len()])
        });
        external(
            declared.unwrap_or_else(|| first_segments(specifier, 2, '.')),
            "nuget",
        )
    }

    fn php(&self, from: usize, import: &RawImport) -> Resolution {
        let path = self.files[from].path;
        let specifier = import.specifier.as_str();
        if specifier.ends_with(".php") || specifier.contains('/') {
            if specifier.contains('$') {
                return Resolution::Ignored;
            }
            let trimmed = specifier.trim_start_matches('/');
            return [
                paths::join(paths::parent(path), trimmed),
                Some(trimmed.to_owned()),
            ]
            .into_iter()
            .flatten()
            .find_map(|candidate| self.file(&candidate))
            .map_or(Resolution::Unresolved, |file| {
                internal(file, Confidence::High)
            });
        }
        let class = specifier.trim_start_matches('\\');
        let own_namespace = self.files[from]
            .analysis
            .and_then(|analysis| analysis.package.as_deref());
        let mut candidates = vec![class.to_owned()];
        if !class.contains('\\')
            && let Some(namespace) = own_namespace
        {
            candidates.push(format!("{namespace}\\{class}"));
        }
        for candidate in &candidates {
            if let Some(files) = self.php_classes.get(candidate) {
                let confidence = if files.len() == 1 {
                    Confidence::High
                } else {
                    Confidence::Medium
                };
                return internal(files[0], confidence);
            }
        }
        let segments: Vec<&str> = class.split('\\').collect();
        if segments.len() == 1 {
            // Global classes such as `Exception`, or traits in the same namespace.
            return Resolution::Ignored;
        }
        // PSR-4 layouts map `App\Models\User` to `app/Models/User.php` or `src/Models/User.php`.
        for start in 1..segments.len() - 1 {
            let suffix = format!("{}.php", segments[start..].join("/"));
            if let Some((file, _)) = self.suffix_match(&suffix) {
                return internal(file, Confidence::Low);
            }
        }
        if self.php_roots.contains(segments[0]) {
            return Resolution::Unresolved;
        }
        external(segments[0], "composer")
    }

    fn c_family(&self, from: usize, import: &RawImport) -> Resolution {
        let path = self.files[from].path;
        let specifier = import.specifier.as_str();
        if import.kind == ImportKind::Import {
            // Objective-C `@import Module;` names a framework module.
            return Resolution::Ignored;
        }
        if import.kind == ImportKind::Include
            && let Some(file) =
                paths::join(paths::parent(path), specifier).and_then(|c| self.file(&c))
        {
            return internal(file, Confidence::High);
        }
        if let Some((file, unique)) = self.suffix_match(specifier) {
            let confidence = if unique {
                Confidence::Medium
            } else {
                Confidence::Low
            };
            return internal(file, confidence);
        }
        if import.kind == ImportKind::SystemInclude {
            Resolution::Ignored
        } else {
            Resolution::Unresolved
        }
    }

    fn ruby(&self, from: usize, import: &RawImport) -> Resolution {
        let path = self.files[from].path;
        let specifier = import.specifier.as_str();
        if specifier.contains("#{") || specifier.contains('$') {
            return Resolution::Ignored;
        }
        let with_extension = if specifier.ends_with(".rb") {
            specifier.to_owned()
        } else {
            format!("{specifier}.rb")
        };
        // `require_relative` resolves against the file's directory.
        if let Some(file) =
            paths::join(paths::parent(path), &with_extension).and_then(|c| self.file(&c))
        {
            return internal(file, Confidence::High);
        }
        // `require` searches the load path, which normally includes each gem's `lib`.
        let lib_candidates = paths::ancestors(path)
            .into_iter()
            .rev()
            .chain([""])
            .map(|dir| child_path(dir, &format!("lib/{with_extension}")));
        if let Some(file) = self.first(lib_candidates) {
            return internal(file, Confidence::Medium);
        }
        if specifier.contains('/')
            && let Some((file, _)) = self.suffix_match(&with_extension)
        {
            return internal(file, Confidence::Low);
        }
        if specifier.starts_with('.') {
            return Resolution::Unresolved;
        }
        let top = specifier.split('/').next().unwrap_or(specifier);
        if RUBY_STDLIB.binary_search(&top).is_ok() {
            return Resolution::Ignored;
        }
        external(top, "rubygems")
    }

    fn dart(&self, from: usize, import: &RawImport) -> Resolution {
        let path = self.files[from].path;
        let specifier = import.specifier.as_str();
        if specifier.starts_with("dart:") {
            return Resolution::Ignored;
        }
        if let Some(rest) = specifier.strip_prefix("package:") {
            let Some((name, sub)) = rest.split_once('/') else {
                return Resolution::Unresolved;
            };
            return match self.dart_packages.get(name) {
                Some(dir) => self
                    .file(&child_path(dir, &format!("lib/{sub}")))
                    .map_or(Resolution::Unresolved, |file| {
                        internal(file, Confidence::High)
                    }),
                None => external(name, "pub"),
            };
        }
        paths::join(paths::parent(path), specifier)
            .and_then(|candidate| self.file(&candidate))
            .map_or(Resolution::Unresolved, |file| {
                internal(file, Confidence::High)
            })
    }

    fn swift(&self, import: &RawImport) -> Resolution {
        let module = import.specifier.split('.').next().unwrap_or_default();
        if let Some(dir) = self.swift_targets.get(module) {
            let targets = self
                .files_within(dir)
                .filter(|&index| self.files[index].path.ends_with(".swift"))
                .take(MAX_PACKAGE_TARGETS)
                .collect();
            return package_level(targets, Confidence::Medium);
        }
        if SWIFT_SYSTEM.binary_search(&module).is_ok() {
            return Resolution::Ignored;
        }
        external(module, "swiftpm")
    }

    fn web(&self, from: usize, import: &RawImport) -> Resolution {
        let path = self.files[from].path;
        let specifier = strip_query(&import.specifier);
        if specifier.is_empty()
            || specifier.starts_with("//")
            || specifier.contains("://")
            || specifier.starts_with("data:")
            || specifier.contains("{{")
            || specifier.contains('$')
        {
            return Resolution::Ignored;
        }
        if let Some(package) = specifier.strip_prefix('~') {
            return external(npm_package_name(package.trim_start_matches('/')), "npm");
        }
        if specifier.contains(':') {
            // Sass built-in modules such as `sass:math`.
            return Resolution::Ignored;
        }
        if let Some(rest) = specifier.strip_prefix('/') {
            // Root-relative URLs are served from a web root: try each ancestor and `public/`.
            let candidates = paths::ancestors(path)
                .into_iter()
                .rev()
                .chain([""])
                .flat_map(|dir| {
                    [
                        child_path(dir, rest),
                        child_path(dir, &format!("public/{rest}")),
                    ]
                });
            return self
                .first(candidates)
                .map_or(Resolution::Unresolved, |file| {
                    internal(file, Confidence::Medium)
                });
        }
        let Some(base) = paths::join(paths::parent(path), specifier) else {
            return Resolution::Unresolved;
        };
        let mut candidates = vec![base.clone()];
        let stylesheet = matches!(
            self.files[from].language.unwrap_or_default(),
            "css" | "scss" | "less"
        );
        if stylesheet && paths::extension(&base).is_none() {
            // Sass partials: `@use "a/b"` may be `a/_b.scss`, `a/b.scss`, or `a/b/_index.scss`.
            let dir = paths::parent(&base);
            let name = paths::file_name(&base);
            for ext in ["scss", "sass", "css", "less"] {
                candidates.push(format!("{base}.{ext}"));
                candidates.push(child_path(dir, &format!("_{name}.{ext}")));
                candidates.push(child_path(&base, &format!("_index.{ext}")));
                candidates.push(child_path(&base, &format!("index.{ext}")));
            }
        }
        self.first(candidates)
            .map_or(Resolution::Unresolved, |file| {
                internal(file, Confidence::High)
            })
    }

    fn generic(&self, from: usize, import: &RawImport) -> Resolution {
        let path = self.files[from].path;
        let specifier = import.specifier.trim_matches(['"', '\'']);
        if specifier.is_empty() || specifier.contains(['$', '~', '%', '(', '{']) {
            return Resolution::Ignored;
        }
        let own_extension = paths::extension(path);
        let path_like = specifier.contains('/') || paths::extension(specifier).is_some();
        if path_like {
            if let Some(file) =
                paths::join(paths::parent(path), specifier).and_then(|c| self.file(&c))
            {
                return internal(file, Confidence::High);
            }
            if let Some(file) = self.file(specifier.trim_start_matches("./")) {
                return internal(file, Confidence::Medium);
            }
            if let Some((file, unique)) = self.suffix_match(specifier) {
                let confidence = if unique {
                    Confidence::Medium
                } else {
                    Confidence::Low
                };
                return internal(file, confidence);
            }
            // Dotted module names (`a.b` in Lua) are not paths when the "extension" is not
            // a known one; fall through to module-name matching.
            let extension = paths::extension(specifier);
            let known = extension.is_some_and(|ext| {
                own_extension.as_deref() == Some(ext.as_str())
                    || matches!(
                        ext.as_str(),
                        "sh" | "bash"
                            | "mk"
                            | "proto"
                            | "lua"
                            | "pl"
                            | "pm"
                            | "r"
                            | "ps1"
                            | "psm1"
                            | "sol"
                            | "ex"
                            | "exs"
                            | "hs"
                    )
            });
            if specifier.contains('/') || known {
                return if specifier.starts_with("google/protobuf/") {
                    Resolution::Ignored
                } else {
                    Resolution::Unresolved
                };
            }
        }
        // Module names: `Foo::Bar` (Perl), `a.b` (Lua), `Foo.Bar` (Elixir, Haskell).
        let Some(extension) = own_extension else {
            return Resolution::Ignored;
        };
        let as_path = specifier.replace("::", "/").replace('.', "/");
        let mut candidates = vec![format!("{as_path}.{extension}")];
        if extension == "pl" {
            candidates.push(format!("{as_path}.pm"));
        }
        if extension == "lua" {
            candidates.push(format!("{as_path}/init.lua"));
        }
        let snake = to_snake_path(&as_path);
        if snake != as_path {
            candidates.push(format!("{snake}.{extension}"));
        }
        for candidate in candidates {
            if let Some((file, unique)) = self.suffix_match(&candidate) {
                let confidence = if unique {
                    Confidence::Medium
                } else {
                    Confidence::Low
                };
                return internal(file, confidence);
            }
        }
        Resolution::Ignored
    }
}

/// Converts `Foo/BarBaz` to `foo/bar_baz` (Elixir file naming).
fn to_snake_path(path: &str) -> String {
    let mut out = String::with_capacity(path.len() + 4);
    let mut previous_lower = false;
    for c in path.chars() {
        if c.is_ascii_uppercase() {
            if previous_lower {
                out.push('_');
            }
            out.push(c.to_ascii_lowercase());
            previous_lower = false;
        } else {
            previous_lower = c.is_ascii_lowercase() || c.is_ascii_digit();
            out.push(c);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use repodna_parser::{FileAnalysis, LanguageRegistry, analyze_source};

    struct Fixture {
        paths: Vec<&'static str>,
        analyses: Vec<Option<FileAnalysis>>,
    }

    impl Fixture {
        fn new(files: &[(&'static str, &str)]) -> Self {
            let registry = LanguageRegistry::builtin();
            let analyses = files
                .iter()
                .map(|(path, text)| {
                    registry
                        .detect_path(path)
                        .map(|spec| analyze_source(spec, text))
                })
                .collect();
            Self {
                paths: files.iter().map(|(path, _)| *path).collect(),
                analyses,
            }
        }

        fn sources(&self) -> Vec<SourceFile<'_>> {
            self.paths
                .iter()
                .zip(&self.analyses)
                .map(|(path, analysis)| SourceFile {
                    path,
                    language: analysis.as_ref().map(|a| a.language.as_str()),
                    code_lines: 1,
                    first_party: true,
                    analysis: analysis.as_ref(),
                })
                .collect()
        }
    }

    /// Resolves every import of `from` and returns (specifier, resolution) pairs.
    fn resolve_all(
        fixture: &Fixture,
        packages: &[PackageBoundary],
        from: &str,
    ) -> Vec<(String, Resolution)> {
        let sources = fixture.sources();
        let resolver = Resolver::new(&sources, packages);
        let index = fixture.paths.iter().position(|p| *p == from).unwrap();
        fixture.analyses[index]
            .as_ref()
            .unwrap()
            .imports
            .iter()
            .map(|import| (import.specifier.clone(), resolver.resolve(index, import)))
            .collect()
    }

    /// Describes a resolution compactly: a target path, `ext:name`, `ignored`, or `unresolved`.
    fn describe(fixture: &Fixture, resolution: &Resolution) -> String {
        match resolution {
            Resolution::Internal {
                targets,
                package_level,
                ..
            } => {
                let names: Vec<&str> = targets.iter().map(|&t| fixture.paths[t]).collect();
                if *package_level {
                    format!("pkg:{}", names.join(","))
                } else {
                    names.join(",")
                }
            }
            Resolution::External { name, .. } => format!("ext:{name}"),
            Resolution::Ignored => "ignored".into(),
            Resolution::Unresolved => "unresolved".into(),
        }
    }

    fn resolved(
        fixture: &Fixture,
        packages: &[PackageBoundary],
        from: &str,
    ) -> Vec<(String, String)> {
        resolve_all(fixture, packages, from)
            .iter()
            .map(|(specifier, resolution)| (specifier.clone(), describe(fixture, resolution)))
            .collect()
    }

    fn package(name: &str, path: &str, ecosystem: &str) -> PackageBoundary {
        PackageBoundary {
            name: name.into(),
            path: path.into(),
            ecosystem: ecosystem.into(),
            manifest: child_path(path, "manifest"),
            internal_dependencies: Vec::new(),
        }
    }

    fn pairs(expected: &[(&str, &str)]) -> Vec<(String, String)> {
        expected
            .iter()
            .map(|(a, b)| ((*a).to_owned(), (*b).to_owned()))
            .collect()
    }

    #[test]
    fn keyword_lists_are_sorted_for_binary_search() {
        for list in [PYTHON_STDLIB, NODE_BUILTINS, RUBY_STDLIB, SWIFT_SYSTEM] {
            assert!(list.windows(2).all(|w| w[0] < w[1]), "{:?}", &list[..3]);
        }
    }

    #[test]
    fn resolves_rust_modules_and_crates() {
        let fixture = Fixture::new(&[
            (
                "crates/app/src/lib.rs",
                "mod parser;\nmod missing;\nuse crate::parser::Lexer;\nuse parser::Token;\nuse std::fmt;\nuse serde::Serialize;\nuse repodna_core::model::Thing;\n",
            ),
            (
                "crates/app/src/parser/mod.rs",
                "mod lexer;\nuse super::helper;\n",
            ),
            (
                "crates/app/src/parser/lexer.rs",
                "use super::Token;\nuse crate::parser;\n",
            ),
            ("crates/core/src/lib.rs", "pub mod model;\n"),
            ("crates/core/src/model.rs", ""),
            ("crates/app/tests/cli.rs", "mod common;\nuse app::parser;\n"),
            ("crates/app/tests/common/mod.rs", ""),
        ]);
        let packages = [
            package("app", "crates/app", "cargo"),
            package("repodna-core", "crates/core", "cargo"),
        ];
        assert_eq!(
            resolved(&fixture, &packages, "crates/app/src/lib.rs"),
            pairs(&[
                ("parser", "crates/app/src/parser/mod.rs"),
                ("missing", "unresolved"),
                ("crate::parser::Lexer", "crates/app/src/parser/mod.rs"),
                ("parser::Token", "crates/app/src/parser/mod.rs"),
                ("std::fmt", "ignored"),
                ("serde::Serialize", "ext:serde"),
                ("repodna_core::model::Thing", "crates/core/src/model.rs"),
            ])
        );
        assert_eq!(
            resolved(&fixture, &packages, "crates/app/src/parser/lexer.rs"),
            pairs(&[
                ("super::Token", "crates/app/src/parser/mod.rs"),
                ("crate::parser", "crates/app/src/parser/mod.rs"),
            ])
        );
        assert_eq!(
            resolved(&fixture, &packages, "crates/app/src/parser/mod.rs"),
            pairs(&[
                ("lexer", "crates/app/src/parser/lexer.rs"),
                ("super::helper", "crates/app/src/lib.rs"),
            ])
        );
        assert_eq!(
            resolved(&fixture, &packages, "crates/app/tests/cli.rs"),
            pairs(&[
                ("common", "crates/app/tests/common/mod.rs"),
                ("app::parser", "crates/app/src/parser/mod.rs"),
            ])
        );
    }

    #[test]
    fn resolves_python_imports() {
        let fixture = Fixture::new(&[
            (
                "src/shop/cart.py",
                "import os\nimport requests\nfrom . import pricing\nfrom .models import Item\nfrom ..shared import util\nfrom shop.models import Order\nimport shop\nfrom shop import nothing_here\nimport yaml\n",
            ),
            ("src/shop/__init__.py", ""),
            ("src/shop/pricing.py", ""),
            ("src/shop/models/__init__.py", ""),
            ("src/shared/util.py", ""),
            ("tests/fixtures/yaml.py", ""),
        ]);
        let found = resolved(&fixture, &[], "src/shop/cart.py");
        assert_eq!(
            found,
            pairs(&[
                ("os", "ignored"),
                ("requests", "ext:requests"),
                (".", "src/shop/pricing.py"),
                (".models", "src/shop/models/__init__.py"),
                ("..shared", "src/shared/util.py"),
                ("shop.models", "src/shop/models/__init__.py"),
                ("shop", "src/shop/__init__.py"),
                ("shop", "src/shop/__init__.py"),
                ("yaml", "tests/fixtures/yaml.py"),
            ])
        );
        // A declared dependency wins over an unanchored local match.
        let sources = fixture.sources();
        let mut resolver = Resolver::new(&sources, &[]);
        resolver.declare_dependency("pypi", "yaml");
        let import = fixture.analyses[0]
            .as_ref()
            .unwrap()
            .imports
            .last()
            .unwrap();
        assert_eq!(resolver.resolve(0, import), external("yaml", "pypi"));
    }

    #[test]
    fn resolves_javascript_and_typescript_imports() {
        let fixture = Fixture::new(&[
            (
                "apps/web/src/main.tsx",
                "import React from 'react';\nimport { App } from './App';\nimport './styles.css';\nimport { api } from '@/lib/api';\nimport { util } from './util.js';\nimport fs from 'node:fs';\nimport path from 'path';\nimport { Button } from '@acme/ui';\nimport { theme } from '@acme/ui/theme';\nimport raw from './data.json?raw';\nimport { x } from './missing';\nimport y from '@scope/pkg/sub';\n",
            ),
            ("apps/web/src/App.tsx", ""),
            ("apps/web/src/styles.css", ""),
            ("apps/web/src/lib/api/index.ts", ""),
            ("apps/web/src/util.ts", ""),
            ("apps/web/src/data.json", "{}"),
            ("packages/ui/src/index.ts", ""),
            ("packages/ui/src/theme.ts", ""),
        ]);
        let packages = [
            package("web", "apps/web", "npm"),
            package("@acme/ui", "packages/ui", "npm"),
        ];
        assert_eq!(
            resolved(&fixture, &packages, "apps/web/src/main.tsx"),
            pairs(&[
                ("react", "ext:react"),
                ("./App", "apps/web/src/App.tsx"),
                ("./styles.css", "apps/web/src/styles.css"),
                ("@/lib/api", "apps/web/src/lib/api/index.ts"),
                ("./util.js", "apps/web/src/util.ts"),
                ("node:fs", "ignored"),
                ("path", "ignored"),
                ("@acme/ui", "packages/ui/src/index.ts"),
                ("@acme/ui/theme", "packages/ui/src/theme.ts"),
                ("./data.json?raw", "apps/web/src/data.json"),
                ("./missing", "unresolved"),
                ("@scope/pkg/sub", "ext:@scope/pkg"),
            ])
        );
    }

    #[test]
    fn resolves_go_packages() {
        let fixture = Fixture::new(&[
            (
                "cmd/server/main.go",
                "package main\n\nimport (\n\t\"fmt\"\n\t\"net/http\"\n\t\"example.com/shop/internal/auth\"\n\t\"example.com/shop/internal/none\"\n\t\"github.com/spf13/cobra/doc\"\n\t\"golang.org/x/text/language\"\n)\n",
            ),
            ("internal/auth/auth.go", "package auth\n"),
            ("internal/auth/token.go", "package auth\n"),
            ("internal/auth/auth_test.go", "package auth\n"),
        ]);
        let packages = [package("example.com/shop", "", "go")];
        assert_eq!(
            resolved(&fixture, &packages, "cmd/server/main.go"),
            pairs(&[
                ("fmt", "ignored"),
                ("net/http", "ignored"),
                (
                    "example.com/shop/internal/auth",
                    "pkg:internal/auth/auth.go,internal/auth/token.go"
                ),
                ("example.com/shop/internal/none", "unresolved"),
                ("github.com/spf13/cobra/doc", "ext:github.com/spf13/cobra"),
                ("golang.org/x/text/language", "ext:golang.org/x/text"),
            ])
        );
    }

    #[test]
    fn resolves_jvm_csharp_and_php_names() {
        let fixture = Fixture::new(&[
            (
                "src/main/java/com/acme/app/App.java",
                "package com.acme.app;\nimport com.acme.app.service.UserService;\nimport com.acme.app.model.*;\nimport static com.acme.app.util.Strings.trim;\nimport java.util.List;\nimport com.google.gson.Gson;\nimport com.acme.app.gone.Missing;\n",
            ),
            (
                "src/main/java/com/acme/app/service/UserService.java",
                "package com.acme.app.service;\n",
            ),
            (
                "src/main/java/com/acme/app/model/User.java",
                "package com.acme.app.model;\n",
            ),
            (
                "src/main/java/com/acme/app/util/Strings.java",
                "package com.acme.app.util;\n",
            ),
            (
                "Api/Controllers/UsersController.cs",
                "using System.Text;\nusing Acme.Api.Services;\nusing Newtonsoft.Json.Linq;\nnamespace Acme.Api.Controllers;\n",
            ),
            (
                "Api/Services/UserService.cs",
                "namespace Acme.Api.Services;\n",
            ),
            (
                "app/Http/Controllers/UserController.php",
                "<?php\nnamespace App\\Http\\Controllers;\nuse App\\Models\\User;\nuse Illuminate\\Http\\Request;\nuse Exception;\nrequire_once __DIR__ . '/helpers.php';\n",
            ),
            ("app/Models/User.php", "<?php\nnamespace App\\Models;\n"),
            ("app/Http/Controllers/helpers.php", "<?php\n"),
        ]);
        assert_eq!(
            resolved(&fixture, &[], "src/main/java/com/acme/app/App.java"),
            pairs(&[
                (
                    "com.acme.app.service.UserService",
                    "src/main/java/com/acme/app/service/UserService.java"
                ),
                (
                    "com.acme.app.model.*",
                    "pkg:src/main/java/com/acme/app/model/User.java"
                ),
                (
                    "com.acme.app.util.Strings.trim",
                    "src/main/java/com/acme/app/util/Strings.java"
                ),
                ("java.util.List", "ignored"),
                ("com.google.gson.Gson", "ext:com.google.gson"),
                ("com.acme.app.gone.Missing", "unresolved"),
            ])
        );
        assert_eq!(
            resolved(&fixture, &[], "Api/Controllers/UsersController.cs"),
            pairs(&[
                ("System.Text", "ignored"),
                ("Acme.Api.Services", "pkg:Api/Services/UserService.cs"),
                ("Newtonsoft.Json.Linq", "ext:Newtonsoft.Json"),
            ])
        );
        assert_eq!(
            resolved(&fixture, &[], "app/Http/Controllers/UserController.php"),
            pairs(&[
                ("App\\Models\\User", "app/Models/User.php"),
                ("Illuminate\\Http\\Request", "ext:Illuminate"),
                ("Exception", "ignored"),
                ("/helpers.php", "app/Http/Controllers/helpers.php"),
            ])
        );
    }

    #[test]
    fn resolves_includes_and_script_requires() {
        let fixture = Fixture::new(&[
            (
                "src/net/socket.c",
                "#include \"socket.h\"\n#include \"util/log.h\"\n#include <stdio.h>\n#include <acme/config.h>\n#include \"gone.h\"\n",
            ),
            ("src/net/socket.h", ""),
            ("src/util/log.h", ""),
            ("include/acme/config.h", ""),
            (
                "lib/shop/cart.rb",
                "require 'json'\nrequire 'rails'\nrequire_relative 'item'\nrequire 'shop/pricing'\n",
            ),
            ("lib/shop/item.rb", ""),
            ("lib/shop/pricing.rb", ""),
            (
                "lib/app.dart",
                "import 'dart:io';\nimport 'package:shop/models/user.dart';\nimport 'package:http/http.dart';\nimport 'src/helpers.dart';\n",
            ),
            ("lib/models/user.dart", ""),
            ("lib/src/helpers.dart", ""),
            (
                "Sources/App/main.swift",
                "import Foundation\nimport Core\nimport Alamofire\n",
            ),
            ("Sources/Core/Model.swift", ""),
            (
                "scripts/build.sh",
                "source ./lib/common.sh\n. \"$HOME/.profile\"\n",
            ),
            ("scripts/lib/common.sh", ""),
        ]);
        let packages = [package("shop", "", "pub")];
        assert_eq!(
            resolved(&fixture, &packages, "src/net/socket.c"),
            pairs(&[
                ("socket.h", "src/net/socket.h"),
                ("util/log.h", "src/util/log.h"),
                ("stdio.h", "ignored"),
                ("acme/config.h", "include/acme/config.h"),
                ("gone.h", "unresolved"),
            ])
        );
        assert_eq!(
            resolved(&fixture, &packages, "lib/shop/cart.rb"),
            pairs(&[
                ("json", "ignored"),
                ("rails", "ext:rails"),
                ("item", "lib/shop/item.rb"),
                ("shop/pricing", "lib/shop/pricing.rb"),
            ])
        );
        assert_eq!(
            resolved(&fixture, &packages, "lib/app.dart"),
            pairs(&[
                ("dart:io", "ignored"),
                ("package:shop/models/user.dart", "lib/models/user.dart"),
                ("package:http/http.dart", "ext:http"),
                ("src/helpers.dart", "lib/src/helpers.dart"),
            ])
        );
        assert_eq!(
            resolved(&fixture, &packages, "Sources/App/main.swift"),
            pairs(&[
                ("Foundation", "ignored"),
                ("Core", "pkg:Sources/Core/Model.swift"),
                ("Alamofire", "ext:Alamofire"),
            ])
        );
        assert_eq!(
            resolved(&fixture, &packages, "scripts/build.sh"),
            pairs(&[
                ("./lib/common.sh", "scripts/lib/common.sh"),
                ("\"$HOME/.profile\"", "ignored"),
            ])
        );
    }

    #[test]
    fn resolves_web_references() {
        let fixture = Fixture::new(&[
            (
                "web/index.html",
                "<link rel=\"stylesheet\" href=\"/styles/main.css\">\n<script type=\"module\" src=\"./src/main.ts\"></script>\n<link href=\"https://fonts.example.com/css\" rel=\"stylesheet\">\n",
            ),
            (
                "web/public/styles/main.css",
                "@import 'base';\n@use 'sass:math';\n",
            ),
            ("web/public/styles/base.css", ""),
            ("web/src/main.ts", ""),
            (
                "web/src/theme.scss",
                "@use 'tokens';\n@import '~bootstrap/scss/bootstrap';\n",
            ),
            ("web/src/_tokens.scss", ""),
        ]);
        assert_eq!(
            resolved(&fixture, &[], "web/index.html"),
            pairs(&[
                ("/styles/main.css", "web/public/styles/main.css"),
                ("./src/main.ts", "web/src/main.ts"),
                ("https://fonts.example.com/css", "ignored"),
            ])
        );
        assert_eq!(
            resolved(&fixture, &[], "web/public/styles/main.css"),
            pairs(&[
                ("base", "web/public/styles/base.css"),
                ("sass:math", "ignored"),
            ])
        );
        assert_eq!(
            resolved(&fixture, &[], "web/src/theme.scss"),
            pairs(&[
                ("tokens", "web/src/_tokens.scss"),
                ("~bootstrap/scss/bootstrap", "ext:bootstrap"),
            ])
        );
    }

    #[test]
    fn resolves_string_references() {
        let fixture = Fixture::new(&[
            ("crates/report/src/html.rs", ""),
            ("crates/report/assets/viewer.html", ""),
            ("config/app.json", ""),
        ]);
        let sources = fixture.sources();
        let resolver = Resolver::new(&sources, &[]);
        assert_eq!(
            resolver.resolve_reference(0, "../assets/viewer.html"),
            Some((1, Confidence::Medium))
        );
        assert_eq!(
            resolver.resolve_reference(0, "config/app.json"),
            Some((2, Confidence::Medium))
        );
        assert_eq!(resolver.resolve_reference(0, "https://x.test/a.json"), None);
        assert_eq!(resolver.resolve_reference(0, "missing.json"), None);
    }

    #[test]
    fn helper_functions() {
        assert_eq!(npm_package_name("@scope/pkg/sub/path"), "@scope/pkg");
        assert_eq!(npm_package_name("@scope/pkg"), "@scope/pkg");
        assert_eq!(npm_package_name("@scope"), "@scope");
        assert_eq!(npm_package_name("lodash/fp"), "lodash");
        assert_eq!(first_segments("a.b.c.d", 3, '.'), "a.b.c");
        assert_eq!(first_segments("a.b", 3, '.'), "a.b");
        assert_eq!(
            to_snake_path("MyApp/UserController"),
            "my_app/user_controller"
        );
        assert_eq!(
            rust_module_path("crates/x/src/a/b.rs", "crates/x/src"),
            vec!["a", "b"]
        );
        assert_eq!(rust_module_path("src/a/mod.rs", "src"), vec!["a"]);
        assert!(rust_module_path("src/lib.rs", "src").is_empty());
        assert!(rust_module_path("tests/cli.rs", "tests").is_empty());
    }
}
