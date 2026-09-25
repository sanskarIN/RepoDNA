//! Built-in language specifications.
//!
//! Languages with symbol patterns get *lexical* analysis; the rest get line counting or
//! detection only. Patterns run on masked lines (comments removed, string contents
//! blanked), so text inside strings and comments can never create false symbols.

use regex::Regex;
use repodna_core::model::languages::LanguageKind;
use repodna_core::model::structure::SymbolKind;

use crate::spec::{
    BlockComment, BodyStyle, ComplexityRules, ImportExtractor, ImportKind, ImportPattern,
    LanguageSpec, LineComment, StringRule, SymbolPattern, Syntax,
};

fn re(pattern: &str) -> Regex {
    // Built-in expressions are constants covered by `every_builtin_pattern_compiles`.
    Regex::new(pattern)
        .unwrap_or_else(|error| panic!("invalid built-in pattern {pattern}: {error}"))
}

fn import(pattern: &str, group: usize, kind: ImportKind) -> ImportPattern {
    ImportPattern {
        regex: re(pattern),
        group,
        kind,
    }
}

fn symbol(pattern: &str, group: usize, kind: SymbolKind) -> SymbolPattern {
    SymbolPattern {
        regex: re(pattern),
        group,
        kind,
    }
}

fn line(token: &str) -> LineComment {
    LineComment {
        token: token.to_owned(),
        requires_boundary: false,
    }
}

fn line_boundary(token: &str) -> LineComment {
    LineComment {
        token: token.to_owned(),
        requires_boundary: true,
    }
}

fn block(open: &str, close: &str, nested: bool) -> BlockComment {
    BlockComment {
        open: open.to_owned(),
        close: close.to_owned(),
        nested,
    }
}

fn exts(spec: &mut LanguageSpec, extensions: &[&str]) {
    spec.extensions = extensions.iter().map(|e| (*e).to_owned()).collect();
}

fn names(list: &[&str]) -> Vec<String> {
    list.iter().map(|n| (*n).to_owned()).collect()
}

fn c_syntax(nested: bool) -> Syntax {
    Syntax {
        line_comments: vec![line("//")],
        block_comments: vec![block("/*", "*/", nested)],
        strings: vec![StringRule::simple("\""), StringRule::simple("'")],
        ..Syntax::default()
    }
}

fn hash_syntax() -> Syntax {
    Syntax {
        line_comments: vec![line_boundary("#")],
        strings: vec![StringRule::simple("\""), StringRule::simple("'")],
        ..Syntax::default()
    }
}

fn c_family_complexity(extra: &[&str]) -> ComplexityRules {
    let mut keywords = vec!["if", "for", "while", "case", "catch"];
    keywords.extend_from_slice(extra);
    ComplexityRules::new(&keywords, &["&&", "||"], true)
}

fn lexical(
    id: &str,
    name: &str,
    extensions: &[&str],
    syntax: Syntax,
    body: BodyStyle,
) -> LanguageSpec {
    let mut spec = LanguageSpec::new(id, name, LanguageKind::Programming);
    exts(&mut spec, extensions);
    spec.syntax = syntax;
    spec.body = body;
    spec.duplication = true;
    spec
}

fn line_count(
    id: &str,
    name: &str,
    kind: LanguageKind,
    extensions: &[&str],
    syntax: Syntax,
) -> LanguageSpec {
    let mut spec = LanguageSpec::new(id, name, kind);
    exts(&mut spec, extensions);
    spec.syntax = syntax;
    spec
}

fn rust() -> LanguageSpec {
    let mut syntax = c_syntax(true);
    syntax.strings = vec![StringRule::multiline("\"")];
    syntax.rust_char_literals = true;
    syntax.rust_raw_strings = true;
    let mut spec = lexical("rust", "Rust", &["rs"], syntax, BodyStyle::Braces);
    spec.import_extractor = ImportExtractor::Rust;
    let vis = r"^\s*(?:pub(?:\s*\([^)]*\))?\s+)?";
    spec.functions = vec![symbol(
        &format!(
            r#"{vis}(?:default\s+)?(?:const\s+)?(?:async\s+)?(?:unsafe\s+)?(?:extern\s+"[^"]*"\s+)?fn\s+([A-Za-z_][A-Za-z0-9_]*)"#
        ),
        1,
        SymbolKind::Function,
    )];
    spec.types = vec![
        symbol(
            &format!(r"{vis}struct\s+([A-Za-z_]\w*)"),
            1,
            SymbolKind::Struct,
        ),
        symbol(
            &format!(r"{vis}union\s+([A-Za-z_]\w*)"),
            1,
            SymbolKind::Struct,
        ),
        symbol(&format!(r"{vis}enum\s+([A-Za-z_]\w*)"), 1, SymbolKind::Enum),
        symbol(
            &format!(r"{vis}(?:unsafe\s+)?trait\s+([A-Za-z_]\w*)"),
            1,
            SymbolKind::Trait,
        ),
        symbol(
            &format!(r"{vis}type\s+([A-Za-z_]\w*)\s*(?:<[^>]*>)?\s*="),
            1,
            SymbolKind::Type,
        ),
        symbol(
            &format!(r"{vis}mod\s+([A-Za-z_]\w*)\s*\{{"),
            1,
            SymbolKind::Module,
        ),
        symbol(r"^\s*macro_rules!\s*([A-Za-z_]\w*)", 1, SymbolKind::Macro),
        symbol(
            &format!(r"{vis}(?:const|static)\s+(?:mut\s+)?([A-Z_][A-Z0-9_]*)\s*:"),
            1,
            SymbolKind::Constant,
        ),
    ];
    spec.complexity = ComplexityRules::new(&["if", "for", "while"], &["&&", "||", "=>"], false);
    spec
}

fn c() -> LanguageSpec {
    let mut spec = lexical("c", "C", &["c", "h"], c_syntax(false), BodyStyle::Braces);
    spec.imports = vec![
        import(r#"^\s*#\s*include\s*"([^"]+)""#, 1, ImportKind::Include),
        import(r"^\s*#\s*include\s*<([^>]+)>", 1, ImportKind::SystemInclude),
    ];
    spec.functions = vec![symbol(
        r"^(?:static\s+|inline\s+|extern\s+)*[A-Za-z_][\w\s\*]*?[\s\*]\b([A-Za-z_]\w*)\s*\([^;{}]*\)\s*\{?\s*$",
        1,
        SymbolKind::Function,
    )];
    spec.types = vec![
        symbol(
            r"^\s*(?:typedef\s+)?struct\s+([A-Za-z_]\w*)\s*\{",
            1,
            SymbolKind::Struct,
        ),
        symbol(
            r"^\s*(?:typedef\s+)?union\s+([A-Za-z_]\w*)\s*\{",
            1,
            SymbolKind::Struct,
        ),
        symbol(
            r"^\s*(?:typedef\s+)?enum\s+([A-Za-z_]\w*)\s*\{",
            1,
            SymbolKind::Enum,
        ),
        symbol(r"^\s*#\s*define\s+([A-Za-z_]\w*)", 1, SymbolKind::Macro),
    ];
    spec.complexity = c_family_complexity(&[]);
    spec
}

fn cpp() -> LanguageSpec {
    let mut syntax = c_syntax(false);
    syntax.strings.insert(0, StringRule::raw("R\"(", ")\""));
    let mut spec = lexical(
        "cpp",
        "C++",
        &[
            "cpp", "cc", "cxx", "c++", "hpp", "hh", "hxx", "h++", "ipp", "tpp", "inl", "ixx",
            "cppm",
        ],
        syntax,
        BodyStyle::Braces,
    );
    spec.imports = c().imports;
    spec.functions = vec![
        symbol(
            r"^\s*(?:template\s*<[^>]*>\s*)?(?:(?:inline|static|virtual|constexpr|consteval|explicit|friend|extern)\s+)*[A-Za-z_][\w:<>,\*&\s]*?[\s\*&]\b([A-Za-z_~][\w:~]*)\s*\([^;{}]*\)\s*(?:const\s*)?(?:noexcept\s*)?(?:override\s*|final\s*)*(?:->\s*[\w:<>\*&\s]+)?\{?\s*$",
            1,
            SymbolKind::Function,
        ),
        symbol(
            r"^\s*([A-Za-z_]\w*::~?[A-Za-z_]\w*)\s*\([^;{}]*\)\s*(?::[^{;]*)?\{?\s*$",
            1,
            SymbolKind::Method,
        ),
    ];
    spec.types = vec![
        symbol(
            r"^\s*(?:template\s*<[^>]*>\s*)?class\s+([A-Za-z_]\w*)[^;]*$",
            1,
            SymbolKind::Class,
        ),
        symbol(
            r"^\s*(?:template\s*<[^>]*>\s*)?struct\s+([A-Za-z_]\w*)[^;]*$",
            1,
            SymbolKind::Struct,
        ),
        symbol(
            r"^\s*enum\s+(?:class\s+|struct\s+)?([A-Za-z_]\w*)[^;]*$",
            1,
            SymbolKind::Enum,
        ),
        symbol(
            r"^\s*namespace\s+([A-Za-z_][\w:]*)\s*\{",
            1,
            SymbolKind::Module,
        ),
        symbol(r"^\s*#\s*define\s+([A-Za-z_]\w*)", 1, SymbolKind::Macro),
    ];
    spec.complexity = c_family_complexity(&[]);
    spec
}

fn java() -> LanguageSpec {
    let mut syntax = c_syntax(false);
    syntax.strings.insert(0, StringRule::multiline("\"\"\""));
    let mut spec = lexical("java", "Java", &["java"], syntax, BodyStyle::Braces);
    spec.imports = vec![import(
        r"^\s*import\s+(?:static\s+)?([\w.]+(?:\.\*)?)\s*;",
        1,
        ImportKind::Import,
    )];
    spec.package = Some(re(r"^\s*package\s+([\w.]+)\s*;"));
    spec.functions = vec![symbol(
        r"^\s*(?:@\w+(?:\([^)]*\))?\s+)*(?:(?:public|protected|private|static|final|abstract|synchronized|native|default|strictfp)\s+)*(?:<[^>]+>\s+)?(?:[\w<>\[\],.?]+\s+)?\b([a-zA-Z_$][\w$]*)\s*\([^;]*\)\s*(?:throws\s+[\w.,\s]+)?\s*\{?\s*$",
        1,
        SymbolKind::Method,
    )];
    spec.types = vec![
        symbol(
            r"^\s*(?:(?:public|protected|private|static|final|abstract|sealed|non-sealed)\s+)*class\s+([A-Za-z_$][\w$]*)",
            1,
            SymbolKind::Class,
        ),
        symbol(
            r"^\s*(?:(?:public|protected|private|static)\s+)*(?:sealed\s+)?interface\s+([A-Za-z_$][\w$]*)",
            1,
            SymbolKind::Interface,
        ),
        symbol(
            r"^\s*(?:(?:public|protected|private|static)\s+)*enum\s+([A-Za-z_$][\w$]*)",
            1,
            SymbolKind::Enum,
        ),
        symbol(
            r"^\s*(?:(?:public|protected|private|static)\s+)*record\s+([A-Za-z_$][\w$]*)",
            1,
            SymbolKind::Struct,
        ),
        symbol(
            r"^\s*(?:public\s+)?@interface\s+([A-Za-z_$][\w$]*)",
            1,
            SymbolKind::Interface,
        ),
    ];
    spec.complexity = c_family_complexity(&[]);
    spec
}

fn csharp() -> LanguageSpec {
    let mut syntax = c_syntax(false);
    syntax.strings = vec![
        StringRule::raw("\"\"\"", "\"\"\""),
        StringRule::raw("@\"", "\""),
        StringRule::simple("\""),
        StringRule::simple("'"),
    ];
    let mut spec = lexical("csharp", "C#", &["cs", "csx"], syntax, BodyStyle::Braces);
    spec.imports = vec![import(
        r"^\s*(?:global\s+)?using\s+(?:static\s+)?(?:\w+\s*=\s*)?([A-Za-z_][\w.]*)\s*;",
        1,
        ImportKind::Import,
    )];
    spec.package = Some(re(r"^\s*namespace\s+([\w.]+)"));
    spec.functions = vec![symbol(
        r"^\s*(?:\[[^\]]*\]\s*)*(?:(?:public|private|protected|internal|static|virtual|override|abstract|sealed|async|extern|unsafe|new|partial|readonly)\s+)*(?:[\w<>\[\],.?()]+\s+)?\b([A-Za-z_]\w*)\s*(?:<[^>]*>)?\s*\([^;]*\)\s*(?:where\s+[^{]+)?\{?\s*$",
        1,
        SymbolKind::Method,
    )];
    spec.types = vec![
        symbol(
            r"\b(?:record\s+)?class\s+([A-Za-z_]\w*)",
            1,
            SymbolKind::Class,
        ),
        symbol(r"\binterface\s+([A-Za-z_]\w*)", 1, SymbolKind::Interface),
        symbol(
            r"\b(?:record\s+)?struct\s+([A-Za-z_]\w*)",
            1,
            SymbolKind::Struct,
        ),
        symbol(r"\benum\s+([A-Za-z_]\w*)", 1, SymbolKind::Enum),
        symbol(r"\brecord\s+([A-Za-z_]\w*)\s*\(", 1, SymbolKind::Struct),
    ];
    spec.complexity = c_family_complexity(&["foreach"]);
    spec
}

fn kotlin() -> LanguageSpec {
    let mut syntax = c_syntax(true);
    syntax
        .strings
        .insert(0, StringRule::raw("\"\"\"", "\"\"\""));
    let mut spec = lexical(
        "kotlin",
        "Kotlin",
        &["kt", "kts"],
        syntax,
        BodyStyle::Braces,
    );
    spec.imports = vec![import(
        r"^\s*import\s+([\w.]+(?:\.\*)?)",
        1,
        ImportKind::Import,
    )];
    spec.package = Some(re(r"^\s*package\s+([\w.]+)"));
    spec.functions = vec![symbol(
        r"\bfun\s+(?:<[^>]+>\s*)?(?:[\w.]+\.)?([A-Za-z_]\w*)\s*\(",
        1,
        SymbolKind::Function,
    )];
    spec.types = vec![
        symbol(
            r"\b(?:(?:data|sealed|abstract|open|enum|inner|annotation|value|private|internal|public)\s+)*class\s+([A-Za-z_]\w*)",
            1,
            SymbolKind::Class,
        ),
        symbol(
            r"\b(?:fun\s+)?interface\s+([A-Za-z_]\w*)",
            1,
            SymbolKind::Interface,
        ),
        symbol(r"\bobject\s+([A-Za-z_]\w*)", 1, SymbolKind::Class),
        symbol(r"^\s*typealias\s+([A-Za-z_]\w*)", 1, SymbolKind::Type),
    ];
    spec.complexity = ComplexityRules::new(
        &["if", "for", "while", "when", "catch"],
        &["&&", "||", "?:"],
        false,
    );
    spec
}

fn scala() -> LanguageSpec {
    let mut syntax = c_syntax(true);
    syntax
        .strings
        .insert(0, StringRule::raw("\"\"\"", "\"\"\""));
    let mut spec = lexical(
        "scala",
        "Scala",
        &["scala", "sc"],
        syntax,
        BodyStyle::Braces,
    );
    spec.imports = vec![import(r"^\s*import\s+([\w.]+)", 1, ImportKind::Import)];
    spec.package = Some(re(r"^\s*package\s+([\w.]+)"));
    spec.functions = vec![symbol(r"\bdef\s+([A-Za-z_]\w*)", 1, SymbolKind::Function)];
    spec.types = vec![
        symbol(
            r"\b(?:case\s+|abstract\s+|sealed\s+|final\s+|implicit\s+)*class\s+([A-Za-z_]\w*)",
            1,
            SymbolKind::Class,
        ),
        symbol(
            r"\b(?:sealed\s+)?trait\s+([A-Za-z_]\w*)",
            1,
            SymbolKind::Trait,
        ),
        symbol(
            r"\b(?:case\s+)?object\s+([A-Za-z_]\w*)",
            1,
            SymbolKind::Class,
        ),
        symbol(r"\benum\s+([A-Za-z_]\w*)", 1, SymbolKind::Enum),
    ];
    spec.complexity = ComplexityRules::new(
        &["if", "for", "while", "case", "catch"],
        &["&&", "||"],
        false,
    );
    spec
}

fn js_like(id: &str, name: &str, extensions: &[&str]) -> LanguageSpec {
    let mut syntax = c_syntax(false);
    syntax.strings.push(StringRule::multiline("`"));
    let mut spec = lexical(id, name, extensions, syntax, BodyStyle::Braces);
    spec.import_extractor = ImportExtractor::JavaScript;
    spec.imports = vec![
        import(r#"\bfrom\s+["']([^"']+)["']"#, 1, ImportKind::Import),
        import(r#"^\s*import\s+["']([^"']+)["']"#, 1, ImportKind::Import),
        import(
            r#"\brequire\s*\(\s*["']([^"']+)["']\s*\)"#,
            1,
            ImportKind::Import,
        ),
        import(
            r#"\bimport\s*\(\s*["']([^"']+)["']\s*\)"#,
            1,
            ImportKind::Import,
        ),
    ];
    spec.functions = vec![
        symbol(
            r"\bfunction\s*\*?\s*([A-Za-z_$][\w$]*)\s*[<(]",
            1,
            SymbolKind::Function,
        ),
        symbol(
            r"^\s*(?:export\s+)?(?:default\s+)?(?:const|let|var)\s+([A-Za-z_$][\w$]*)\s*(?::[^=]+)?=\s*(?:async\s+)?(?:function\b|(?:<[^>]*>\s*)?\([^)]*\)\s*(?::\s*[^=]+)?=>|[A-Za-z_$][\w$]*\s*=>)",
            1,
            SymbolKind::Function,
        ),
        symbol(
            r"^\s+(?:(?:public|private|protected|static|async|override|readonly|abstract|get|set)\s+)*\*?\b([A-Za-z_$][\w$]*)\s*(?:<[^>]*>)?\s*\([^)]*\)\s*(?::\s*[^{;]+)?\{\s*$",
            1,
            SymbolKind::Method,
        ),
    ];
    spec.types = vec![
        symbol(r"\bclass\s+([A-Za-z_$][\w$]*)", 1, SymbolKind::Class),
        symbol(
            r"^\s*(?:export\s+)?(?:declare\s+)?interface\s+([A-Za-z_$][\w$]*)",
            1,
            SymbolKind::Interface,
        ),
        symbol(
            r"^\s*(?:export\s+)?(?:declare\s+)?type\s+([A-Za-z_$][\w$]*)\s*(?:<[^>]*>)?\s*=",
            1,
            SymbolKind::Type,
        ),
        symbol(
            r"^\s*(?:export\s+)?(?:declare\s+)?(?:const\s+)?enum\s+([A-Za-z_$][\w$]*)",
            1,
            SymbolKind::Enum,
        ),
        symbol(
            r"^\s*(?:export\s+)?(?:declare\s+)?namespace\s+([A-Za-z_$][\w$.]*)",
            1,
            SymbolKind::Module,
        ),
    ];
    spec.complexity = c_family_complexity(&[]);
    spec
}

fn python() -> LanguageSpec {
    let syntax = Syntax {
        line_comments: vec![line("#")],
        strings: vec![
            StringRule::multiline("\"\"\""),
            StringRule::multiline("'''"),
            StringRule::simple("\""),
            StringRule::simple("'"),
        ],
        ..Syntax::default()
    };
    let mut spec = lexical(
        "python",
        "Python",
        &["py", "pyw", "pyi"],
        syntax,
        BodyStyle::Indentation,
    );
    spec.shebangs = names(&["python", "python2", "python3", "pypy", "pypy3"]);
    spec.import_extractor = ImportExtractor::Python;
    spec.functions = vec![symbol(
        r"^\s*(?:async\s+)?def\s+([A-Za-z_]\w*)\s*[(\[]",
        1,
        SymbolKind::Function,
    )];
    spec.types = vec![symbol(r"^\s*class\s+([A-Za-z_]\w*)", 1, SymbolKind::Class)];
    spec.complexity = ComplexityRules::new(
        &["if", "elif", "for", "while", "except", "and", "or", "case"],
        &[],
        false,
    );
    spec
}

fn go() -> LanguageSpec {
    let mut syntax = c_syntax(false);
    syntax.strings.push(StringRule::raw("`", "`"));
    let mut spec = lexical("go", "Go", &["go"], syntax, BodyStyle::Braces);
    spec.import_extractor = ImportExtractor::Go;
    spec.package = Some(re(r"^\s*package\s+([A-Za-z_]\w*)"));
    spec.functions = vec![
        symbol(
            r"^func\s+\([^)]*\)\s*([A-Za-z_]\w*)\s*[\[(]",
            1,
            SymbolKind::Method,
        ),
        symbol(r"^func\s+([A-Za-z_]\w*)\s*[\[(]", 1, SymbolKind::Function),
    ];
    spec.types = vec![
        symbol(
            r"^\s*type\s+([A-Za-z_]\w*)(?:\[[^\]]*\])?\s+struct\b",
            1,
            SymbolKind::Struct,
        ),
        symbol(
            r"^\s*type\s+([A-Za-z_]\w*)(?:\[[^\]]*\])?\s+interface\b",
            1,
            SymbolKind::Interface,
        ),
        symbol(r"^\s*type\s+([A-Za-z_]\w*)\s+[^=\s]", 1, SymbolKind::Type),
    ];
    spec.complexity = ComplexityRules::new(&["if", "for", "case", "select"], &["&&", "||"], false);
    spec
}

fn php() -> LanguageSpec {
    let syntax = Syntax {
        line_comments: vec![line("//"), line("#")],
        block_comments: vec![block("/*", "*/", false)],
        strings: vec![StringRule::multiline("\""), StringRule::multiline("'")],
        ..Syntax::default()
    };
    let mut spec = lexical(
        "php",
        "PHP",
        &["php", "phtml", "php8", "php7"],
        syntax,
        BodyStyle::Braces,
    );
    spec.shebangs = names(&["php"]);
    spec.imports = vec![
        import(
            r"^\s*use\s+(?:function\s+|const\s+)?\\?([A-Za-z_][\w\\]*)",
            1,
            ImportKind::Import,
        ),
        import(
            r#"\b(?:require|include)(?:_once)?\s*\(?\s*(?:__DIR__\s*\.\s*)?["']([^"']+)["']"#,
            1,
            ImportKind::Import,
        ),
    ];
    spec.package = Some(re(r"^\s*namespace\s+([A-Za-z_][\w\\]*)"));
    spec.functions = vec![symbol(
        r"\bfunction\s+&?\s*([A-Za-z_]\w*)\s*\(",
        1,
        SymbolKind::Function,
    )];
    spec.types = vec![
        symbol(
            r"\b(?:(?:abstract|final|readonly)\s+)*class\s+([A-Za-z_]\w*)",
            1,
            SymbolKind::Class,
        ),
        symbol(r"\binterface\s+([A-Za-z_]\w*)", 1, SymbolKind::Interface),
        symbol(r"\btrait\s+([A-Za-z_]\w*)", 1, SymbolKind::Trait),
        symbol(r"^\s*enum\s+([A-Za-z_]\w*)", 1, SymbolKind::Enum),
    ];
    spec.complexity = c_family_complexity(&["elseif", "foreach"]);
    spec
}

fn swift() -> LanguageSpec {
    let mut syntax = c_syntax(true);
    syntax.strings = vec![StringRule::multiline("\"\"\""), StringRule::simple("\"")];
    let mut spec = lexical("swift", "Swift", &["swift"], syntax, BodyStyle::Braces);
    spec.imports = vec![import(
        r"^\s*(?:@testable\s+)?import\s+(?:(?:struct|class|enum|protocol|func|var|typealias)\s+)?([A-Za-z_][\w.]*)",
        1,
        ImportKind::Import,
    )];
    spec.functions = vec![symbol(
        r"\bfunc\s+([A-Za-z_]\w*)\s*(?:<[^>]*>)?\s*\(",
        1,
        SymbolKind::Function,
    )];
    spec.types = vec![
        symbol(
            r"\b(?:final\s+)?class\s+([A-Za-z_]\w*)",
            1,
            SymbolKind::Class,
        ),
        symbol(r"\bstruct\s+([A-Za-z_]\w*)", 1, SymbolKind::Struct),
        symbol(r"\benum\s+([A-Za-z_]\w*)", 1, SymbolKind::Enum),
        symbol(r"\bprotocol\s+([A-Za-z_]\w*)", 1, SymbolKind::Interface),
        symbol(r"\bactor\s+([A-Za-z_]\w*)", 1, SymbolKind::Class),
    ];
    spec.complexity = ComplexityRules::new(
        &["if", "guard", "for", "while", "case", "catch", "repeat"],
        &["&&", "||", "??"],
        true,
    );
    spec
}

fn dart() -> LanguageSpec {
    let mut syntax = c_syntax(true);
    syntax.strings = vec![
        StringRule::multiline("\"\"\""),
        StringRule::multiline("'''"),
        StringRule::simple("\""),
        StringRule::simple("'"),
    ];
    let mut spec = lexical("dart", "Dart", &["dart"], syntax, BodyStyle::Braces);
    spec.imports = vec![import(
        r#"^\s*(?:import|export|part)\s+["']([^"']+)["']"#,
        1,
        ImportKind::Import,
    )];
    spec.functions = vec![symbol(
        r"^\s*(?:(?:static|external|factory|abstract|@override)\s+)*[A-Za-z_][\w<>,?]*(?:\s*<[^>]*>)?\s+([a-z_]\w*)\s*\([^;]*\)\s*(?:async\*?|sync\*)?\s*(?:\{|=>)",
        1,
        SymbolKind::Function,
    )];
    spec.types = vec![
        symbol(
            r"^\s*(?:abstract\s+|sealed\s+|base\s+|final\s+|interface\s+)*class\s+([A-Za-z_]\w*)",
            1,
            SymbolKind::Class,
        ),
        symbol(r"^\s*mixin\s+([A-Za-z_]\w*)", 1, SymbolKind::Trait),
        symbol(r"^\s*enum\s+([A-Za-z_]\w*)", 1, SymbolKind::Enum),
        symbol(r"^\s*extension\s+([A-Za-z_]\w*)", 1, SymbolKind::Module),
        symbol(r"^\s*typedef\s+([A-Za-z_]\w*)", 1, SymbolKind::Type),
    ];
    spec.complexity = c_family_complexity(&[]);
    spec
}

fn ruby() -> LanguageSpec {
    let syntax = Syntax {
        line_comments: vec![line("#")],
        block_comments: vec![block("=begin", "=end", false)],
        strings: vec![StringRule::multiline("\""), StringRule::multiline("'")],
        ..Syntax::default()
    };
    let mut spec = lexical(
        "ruby",
        "Ruby",
        &["rb", "rake", "gemspec", "ru"],
        syntax,
        BodyStyle::EndKeyword,
    );
    spec.filenames = names(&[
        "Rakefile",
        "Gemfile",
        "Guardfile",
        "Podfile",
        "Fastfile",
        "Vagrantfile",
        "Brewfile",
    ]);
    spec.shebangs = names(&["ruby"]);
    spec.imports = vec![
        import(
            r#"^\s*require_relative\s*\(?\s*["']([^"']+)["']"#,
            1,
            ImportKind::Import,
        ),
        import(
            r#"^\s*require\s*\(?\s*["']([^"']+)["']"#,
            1,
            ImportKind::Import,
        ),
    ];
    spec.functions = vec![symbol(
        r"^\s*def\s+(?:self\.)?([A-Za-z_]\w*[?!=]?)",
        1,
        SymbolKind::Function,
    )];
    spec.types = vec![
        symbol(r"^\s*class\s+([A-Z][\w:]*)", 1, SymbolKind::Class),
        symbol(r"^\s*module\s+([A-Z][\w:]*)", 1, SymbolKind::Module),
    ];
    spec.complexity = ComplexityRules::new(
        &[
            "if", "elsif", "unless", "while", "until", "for", "when", "rescue", "and", "or",
        ],
        &["&&", "||"],
        true,
    );
    spec
}

fn shell() -> LanguageSpec {
    let syntax = Syntax {
        line_comments: vec![line_boundary("#")],
        strings: vec![StringRule::multiline("\""), StringRule::raw("'", "'")],
        ..Syntax::default()
    };
    let mut spec = lexical(
        "shell",
        "Shell",
        &["sh", "bash", "zsh", "ksh", "bats", "fish"],
        syntax,
        BodyStyle::Braces,
    );
    spec.filenames = names(&[
        ".bashrc",
        ".bash_profile",
        ".zshrc",
        ".profile",
        "bashrc",
        "zshrc",
    ]);
    spec.shebangs = names(&["sh", "bash", "zsh", "ksh", "dash", "ash", "fish"]);
    spec.imports = vec![import(
        r"^\s*(?:source|\.)\s+([^\s;|&]+)",
        1,
        ImportKind::Import,
    )];
    spec.functions = vec![
        symbol(r"^\s*function\s+([A-Za-z_][\w-]*)", 1, SymbolKind::Function),
        symbol(
            r"^\s*([A-Za-z_][\w-]*)\s*\(\s*\)\s*\{?",
            1,
            SymbolKind::Function,
        ),
    ];
    spec.complexity = ComplexityRules::new(
        &["if", "elif", "for", "while", "until", "case"],
        &["&&", "||"],
        false,
    );
    spec
}

fn powershell() -> LanguageSpec {
    let syntax = Syntax {
        line_comments: vec![line("#")],
        block_comments: vec![block("<#", "#>", false)],
        strings: vec![StringRule::multiline("\""), StringRule::raw("'", "'")],
        ..Syntax::default()
    };
    let mut spec = lexical(
        "powershell",
        "PowerShell",
        &["ps1", "psm1", "psd1"],
        syntax,
        BodyStyle::Braces,
    );
    spec.shebangs = names(&["pwsh", "powershell"]);
    spec.imports = vec![import(
        r"^\s*(?:Import-Module|\.)\s+([^\s;]+)",
        1,
        ImportKind::Import,
    )];
    spec.functions = vec![symbol(
        r"(?i)^\s*function\s+([A-Za-z_][\w-]*)",
        1,
        SymbolKind::Function,
    )];
    spec.complexity = ComplexityRules::new(
        &["if", "elseif", "for", "foreach", "while", "switch", "catch"],
        &["-and", "-or"],
        false,
    );
    spec
}

fn lua() -> LanguageSpec {
    // The scanner tries block comments before line comments, so `--[[` wins over `--`.
    let syntax = Syntax {
        line_comments: vec![line("--")],
        block_comments: vec![block("--[[", "]]", false)],
        strings: vec![
            StringRule::raw("[[", "]]"),
            StringRule::simple("\""),
            StringRule::simple("'"),
        ],
        ..Syntax::default()
    };
    let mut spec = lexical("lua", "Lua", &["lua"], syntax, BodyStyle::EndKeyword);
    spec.shebangs = names(&["lua", "luajit"]);
    spec.imports = vec![import(
        r#"\brequire\s*\(?\s*["']([^"']+)["']"#,
        1,
        ImportKind::Import,
    )];
    spec.functions = vec![
        symbol(
            r"^\s*local\s+function\s+([A-Za-z_]\w*)",
            1,
            SymbolKind::Function,
        ),
        symbol(
            r"\bfunction\s+([A-Za-z_][\w.:]*)\s*\(",
            1,
            SymbolKind::Function,
        ),
    ];
    spec.complexity = ComplexityRules::new(
        &["if", "elseif", "for", "while", "repeat", "and", "or"],
        &[],
        false,
    );
    spec
}

fn perl() -> LanguageSpec {
    let mut spec = lexical(
        "perl",
        "Perl",
        &["pl", "pm", "t"],
        hash_syntax(),
        BodyStyle::Braces,
    );
    spec.shebangs = names(&["perl"]);
    spec.imports = vec![import(
        r"^\s*(?:use|require)\s+([A-Za-z_][\w:]*)",
        1,
        ImportKind::Import,
    )];
    spec.package = Some(re(r"^\s*package\s+([\w:]+)"));
    spec.functions = vec![symbol(r"^\s*sub\s+([A-Za-z_]\w*)", 1, SymbolKind::Function)];
    spec.complexity = ComplexityRules::new(
        &["if", "elsif", "unless", "while", "until", "for", "foreach"],
        &["&&", "||"],
        true,
    );
    spec
}

fn r_lang() -> LanguageSpec {
    let mut spec = lexical("r", "R", &["r", "rmd"], hash_syntax(), BodyStyle::Braces);
    spec.shebangs = names(&["Rscript"]);
    spec.imports = vec![import(
        r#"^\s*(?:library|require)\s*\(\s*["']?([\w.]+)"#,
        1,
        ImportKind::Import,
    )];
    spec.functions = vec![symbol(
        r"^\s*([A-Za-z_.][\w.]*)\s*(?:<-|=)\s*function\s*\(",
        1,
        SymbolKind::Function,
    )];
    spec.complexity = ComplexityRules::new(&["if", "for", "while", "repeat"], &["&&", "||"], false);
    spec
}

fn elixir() -> LanguageSpec {
    let syntax = Syntax {
        line_comments: vec![line("#")],
        strings: vec![StringRule::multiline("\"\"\""), StringRule::multiline("\"")],
        ..Syntax::default()
    };
    let mut spec = lexical(
        "elixir",
        "Elixir",
        &["ex", "exs"],
        syntax,
        BodyStyle::EndKeyword,
    );
    spec.imports = vec![import(
        r"^\s*(?:alias|import|require|use)\s+([A-Z][\w.]*)",
        1,
        ImportKind::Import,
    )];
    spec.functions = vec![symbol(
        r"^\s*defp?\s+([a-z_]\w*[?!]?)",
        1,
        SymbolKind::Function,
    )];
    spec.types = vec![symbol(
        r"^\s*defmodule\s+([A-Z][\w.]*)",
        1,
        SymbolKind::Module,
    )];
    spec.complexity = ComplexityRules::new(
        &[
            "if", "unless", "case", "cond", "with", "rescue", "and", "or",
        ],
        &["&&", "||"],
        false,
    );
    spec
}

fn groovy() -> LanguageSpec {
    let mut syntax = c_syntax(false);
    syntax.strings.insert(0, StringRule::multiline("\"\"\""));
    syntax.strings.insert(1, StringRule::multiline("'''"));
    let mut spec = lexical(
        "groovy",
        "Groovy",
        &["groovy", "gradle", "gvy", "gy"],
        syntax,
        BodyStyle::Braces,
    );
    spec.filenames = names(&["Jenkinsfile"]);
    spec.imports = vec![import(
        r"^\s*import\s+([\w.]+(?:\.\*)?)",
        1,
        ImportKind::Import,
    )];
    spec.package = Some(re(r"^\s*package\s+([\w.]+)"));
    spec.functions = vec![symbol(
        r"\bdef\s+([A-Za-z_]\w*)\s*\(",
        1,
        SymbolKind::Function,
    )];
    spec.types = vec![
        symbol(r"\bclass\s+([A-Za-z_]\w*)", 1, SymbolKind::Class),
        symbol(r"\binterface\s+([A-Za-z_]\w*)", 1, SymbolKind::Interface),
    ];
    spec.complexity = c_family_complexity(&[]);
    spec
}

fn solidity() -> LanguageSpec {
    let mut spec = lexical(
        "solidity",
        "Solidity",
        &["sol"],
        c_syntax(false),
        BodyStyle::Braces,
    );
    spec.imports = vec![import(
        r#"^\s*import\s+(?:[^"']*from\s+)?["']([^"']+)["']"#,
        1,
        ImportKind::Import,
    )];
    spec.functions = vec![symbol(
        r"\bfunction\s+([A-Za-z_]\w*)\s*\(",
        1,
        SymbolKind::Function,
    )];
    spec.types = vec![
        symbol(
            r"^\s*(?:abstract\s+)?contract\s+([A-Za-z_]\w*)",
            1,
            SymbolKind::Class,
        ),
        symbol(r"^\s*interface\s+([A-Za-z_]\w*)", 1, SymbolKind::Interface),
        symbol(r"^\s*library\s+([A-Za-z_]\w*)", 1, SymbolKind::Module),
        symbol(r"^\s*struct\s+([A-Za-z_]\w*)", 1, SymbolKind::Struct),
    ];
    spec.complexity = c_family_complexity(&[]);
    spec
}

fn sql() -> LanguageSpec {
    let syntax = Syntax {
        line_comments: vec![line("--")],
        block_comments: vec![block("/*", "*/", false)],
        strings: vec![StringRule::raw("'", "'")],
        ..Syntax::default()
    };
    let mut spec = LanguageSpec::new("sql", "SQL", LanguageKind::Programming);
    exts(&mut spec, &["sql", "psql", "pgsql", "mysql", "ddl", "dml"]);
    spec.syntax = syntax;
    spec.types = vec![symbol(
        r#"(?i)\bcreate\s+(?:or\s+replace\s+)?(?:(?:temporary|temp|unique|materialized)\s+)*(?:table|view|index|function|procedure|trigger|type|sequence|schema)\s+(?:if\s+not\s+exists\s+)?([\w."`]+)"#,
        1,
        SymbolKind::Schema,
    )];
    spec
}

fn markup_languages() -> Vec<LanguageSpec> {
    let html_comment = || block("<!--", "-->", false);
    let mut html = line_count(
        "html",
        "HTML",
        LanguageKind::Markup,
        &["html", "htm", "xhtml"],
        Syntax {
            block_comments: vec![html_comment()],
            ..Syntax::default()
        },
    );
    html.imports = vec![
        import(
            r#"<script\b[^>]*\bsrc\s*=\s*["']([^"']+)["']"#,
            1,
            ImportKind::Reference,
        ),
        import(
            r#"<link\b[^>]*\bhref\s*=\s*["']([^"']+)["']"#,
            1,
            ImportKind::Reference,
        ),
    ];
    let css_syntax = || Syntax {
        block_comments: vec![block("/*", "*/", false)],
        strings: vec![StringRule::simple("\""), StringRule::simple("'")],
        ..Syntax::default()
    };
    let css_imports = || {
        vec![
            import(
                r#"@import\s+(?:url\(\s*)?["']?([^"')\s;]+)"#,
                1,
                ImportKind::Reference,
            ),
            import(
                r#"@(?:use|forward)\s+["']([^"']+)["']"#,
                1,
                ImportKind::Reference,
            ),
        ]
    };
    let mut css = line_count(
        "css",
        "CSS",
        LanguageKind::Stylesheet,
        &["css"],
        css_syntax(),
    );
    css.imports = css_imports();
    let mut scss_syntax = css_syntax();
    scss_syntax.line_comments = vec![line("//")];
    let mut scss = line_count(
        "scss",
        "SCSS",
        LanguageKind::Stylesheet,
        &["scss", "sass"],
        scss_syntax.clone(),
    );
    scss.imports = css_imports();
    let mut less = line_count(
        "less",
        "Less",
        LanguageKind::Stylesheet,
        &["less"],
        scss_syntax,
    );
    less.imports = css_imports();

    let mut vue = line_count(
        "vue",
        "Vue",
        LanguageKind::Markup,
        &["vue"],
        Syntax {
            block_comments: vec![html_comment(), block("/*", "*/", false)],
            ..Syntax::default()
        },
    );
    vue.imports = js_like("typescript", "TypeScript", &[]).imports;
    let mut svelte = vue.clone();
    svelte.id = "svelte".to_owned();
    svelte.name = "Svelte".to_owned();
    exts(&mut svelte, &["svelte"]);
    let mut astro = vue.clone();
    astro.id = "astro".to_owned();
    astro.name = "Astro".to_owned();
    exts(&mut astro, &["astro"]);

    let templates = line_count(
        "template",
        "Template",
        LanguageKind::Markup,
        &[
            "hbs",
            "handlebars",
            "mustache",
            "ejs",
            "erb",
            "j2",
            "jinja",
            "jinja2",
            "twig",
            "liquid",
            "njk",
        ],
        Syntax {
            block_comments: vec![
                html_comment(),
                block("{#", "#}", false),
                block("{{!", "}}", false),
            ],
            ..Syntax::default()
        },
    );
    vec![html, css, scss, less, vue, svelte, astro, templates]
}

fn data_languages() -> Vec<LanguageSpec> {
    let json_syntax = Syntax {
        line_comments: vec![line("//")],
        block_comments: vec![block("/*", "*/", false)],
        strings: vec![StringRule::simple("\"")],
        ..Syntax::default()
    };
    let mut json = line_count(
        "json",
        "JSON",
        LanguageKind::Data,
        &[
            "json",
            "jsonc",
            "json5",
            "geojson",
            "webmanifest",
            "har",
            "jsonl",
            "ndjson",
        ],
        json_syntax,
    );
    json.filenames = names(&[
        ".babelrc",
        ".eslintrc",
        ".prettierrc",
        "composer.lock",
        "flake.lock",
    ]);

    let mut yaml = line_count(
        "yaml",
        "YAML",
        LanguageKind::Data,
        &["yml", "yaml"],
        hash_syntax(),
    );
    yaml.filenames = names(&[".clang-format", ".clang-tidy"]);

    let mut toml_lang = line_count(
        "toml",
        "TOML",
        LanguageKind::Data,
        &["toml"],
        Syntax {
            line_comments: vec![line("#")],
            strings: vec![
                StringRule::multiline("\"\"\""),
                StringRule::raw("'''", "'''"),
                StringRule::simple("\""),
                StringRule::raw("'", "'"),
            ],
            ..Syntax::default()
        },
    );
    toml_lang.filenames = names(&["Cargo.lock", "poetry.lock", "Pipfile", "uv.lock"]);

    let xml_syntax = || Syntax {
        block_comments: vec![block("<!--", "-->", false)],
        ..Syntax::default()
    };
    let xml = line_count(
        "xml",
        "XML",
        LanguageKind::Data,
        &[
            "xml",
            "xsd",
            "xsl",
            "xslt",
            "plist",
            "csproj",
            "vbproj",
            "fsproj",
            "vcxproj",
            "props",
            "targets",
            "resx",
            "xaml",
            "nuspec",
            "wsdl",
            "rss",
            "atom",
            "storyboard",
            "xib",
        ],
        xml_syntax(),
    );
    let svg = line_count("svg", "SVG", LanguageKind::Data, &["svg"], xml_syntax());

    let mut markdown = line_count(
        "markdown",
        "Markdown",
        LanguageKind::Prose,
        &["md", "markdown", "mdx", "mdown", "mkd"],
        Syntax {
            block_comments: vec![block("<!--", "-->", false)],
            ..Syntax::default()
        },
    );
    markdown.filenames = names(&["README", "CHANGELOG", "CONTRIBUTING", "AUTHORS"]);
    let rst = line_count(
        "restructuredtext",
        "reStructuredText",
        LanguageKind::Prose,
        &["rst"],
        Syntax::default(),
    );
    let asciidoc = line_count(
        "asciidoc",
        "AsciiDoc",
        LanguageKind::Prose,
        &["adoc", "asciidoc"],
        Syntax {
            line_comments: vec![line("//")],
            ..Syntax::default()
        },
    );
    let text = line_count(
        "text",
        "Text",
        LanguageKind::Prose,
        &["txt", "text"],
        Syntax::default(),
    );
    let latex = line_count(
        "latex",
        "TeX",
        LanguageKind::Prose,
        &["tex", "sty", "cls", "bib"],
        Syntax {
            line_comments: vec![line("%")],
            ..Syntax::default()
        },
    );

    let mut ini = line_count(
        "ini",
        "INI",
        LanguageKind::Data,
        &["ini", "cfg", "conf", "properties", "env"],
        Syntax {
            line_comments: vec![line_boundary("#"), line_boundary(";")],
            ..Syntax::default()
        },
    );
    ini.filenames = names(&[
        ".editorconfig",
        ".gitconfig",
        ".npmrc",
        ".env",
        ".env.example",
        ".flake8",
        ".pylintrc",
    ]);
    let csv = line_count(
        "csv",
        "CSV",
        LanguageKind::Data,
        &["csv", "tsv", "psv"],
        Syntax::default(),
    );

    let mut protobuf = line_count(
        "protobuf",
        "Protocol Buffers",
        LanguageKind::Data,
        &["proto"],
        c_syntax(false),
    );
    protobuf.imports = vec![import(
        r#"^\s*import\s+(?:public\s+|weak\s+)?["']([^"']+)["']"#,
        1,
        ImportKind::Import,
    )];
    let graphql = line_count(
        "graphql",
        "GraphQL",
        LanguageKind::Data,
        &["graphql", "gql", "graphqls"],
        hash_syntax(),
    );
    let hcl = line_count(
        "hcl",
        "HCL / Terraform",
        LanguageKind::Data,
        &["tf", "tfvars", "hcl", "nomad"],
        Syntax {
            line_comments: vec![line("#"), line("//")],
            block_comments: vec![block("/*", "*/", false)],
            strings: vec![StringRule::simple("\"")],
            ..Syntax::default()
        },
    );
    let nix = line_count(
        "nix",
        "Nix",
        LanguageKind::Data,
        &["nix"],
        Syntax {
            line_comments: vec![line("#")],
            block_comments: vec![block("/*", "*/", false)],
            strings: vec![StringRule::multiline("''"), StringRule::simple("\"")],
            ..Syntax::default()
        },
    );
    let mut jupyter = LanguageSpec::new("jupyter", "Jupyter Notebook", LanguageKind::Data);
    exts(&mut jupyter, &["ipynb"]);
    vec![
        json, yaml, toml_lang, xml, svg, markdown, rst, asciidoc, text, latex, ini, csv, protobuf,
        graphql, hcl, nix, jupyter,
    ]
}

fn build_languages() -> Vec<LanguageSpec> {
    let mut dockerfile = line_count(
        "dockerfile",
        "Dockerfile",
        LanguageKind::Build,
        &["dockerfile", "containerfile"],
        Syntax {
            line_comments: vec![line_boundary("#")],
            ..Syntax::default()
        },
    );
    dockerfile.filenames = names(&["Dockerfile", "Containerfile", "dockerfile"]);
    dockerfile.filename_prefixes = names(&["Dockerfile.", "Containerfile."]);

    let mut makefile = line_count(
        "makefile",
        "Makefile",
        LanguageKind::Build,
        &["mk", "mak", "make"],
        Syntax {
            line_comments: vec![line_boundary("#")],
            ..Syntax::default()
        },
    );
    makefile.filenames = names(&["Makefile", "makefile", "GNUmakefile", "BSDmakefile"]);
    makefile.imports = vec![import(r"^\s*-?include\s+([^\s]+)", 1, ImportKind::Include)];

    let mut cmake = line_count(
        "cmake",
        "CMake",
        LanguageKind::Build,
        &["cmake"],
        Syntax {
            line_comments: vec![line("#")],
            strings: vec![StringRule::multiline("\"")],
            ..Syntax::default()
        },
    );
    cmake.filenames = names(&["CMakeLists.txt"]);

    let mut starlark = line_count(
        "starlark",
        "Starlark",
        LanguageKind::Build,
        &["bzl", "star", "bazel"],
        hash_syntax(),
    );
    starlark.filenames = names(&[
        "BUILD",
        "WORKSPACE",
        "BUILD.bazel",
        "WORKSPACE.bazel",
        "MODULE.bazel",
        "Tiltfile",
    ]);

    let batch = line_count(
        "batch",
        "Batchfile",
        LanguageKind::Programming,
        &["bat", "cmd"],
        Syntax {
            line_comments: vec![line("::"), line("REM "), line("rem ")],
            ..Syntax::default()
        },
    );
    vec![dockerfile, makefile, cmake, starlark, batch]
}

fn other_programming() -> Vec<LanguageSpec> {
    let mut objc = lexical(
        "objective-c",
        "Objective-C",
        &["m", "mm"],
        c_syntax(false),
        BodyStyle::Braces,
    );
    objc.imports = vec![
        import(
            r#"^\s*#\s*(?:import|include)\s*"([^"]+)""#,
            1,
            ImportKind::Include,
        ),
        import(
            r"^\s*#\s*(?:import|include)\s*<([^>]+)>",
            1,
            ImportKind::SystemInclude,
        ),
        import(r"^\s*@import\s+([\w.]+)", 1, ImportKind::Import),
    ];
    objc.functions = vec![symbol(
        r"^\s*[-+]\s*\([^)]*\)\s*([A-Za-z_]\w*)",
        1,
        SymbolKind::Method,
    )];
    objc.types = vec![
        symbol(r"^\s*@interface\s+([A-Za-z_]\w*)", 1, SymbolKind::Class),
        symbol(r"^\s*@protocol\s+([A-Za-z_]\w*)", 1, SymbolKind::Interface),
    ];
    objc.complexity = c_family_complexity(&[]);

    let mut haskell = line_count(
        "haskell",
        "Haskell",
        LanguageKind::Programming,
        &["hs", "lhs"],
        Syntax {
            line_comments: vec![line("--")],
            block_comments: vec![block("{-", "-}", true)],
            strings: vec![StringRule::simple("\"")],
            ..Syntax::default()
        },
    );
    haskell.imports = vec![import(
        r"^\s*import\s+(?:qualified\s+)?([A-Z][\w.]*)",
        1,
        ImportKind::Import,
    )];
    let erlang = line_count(
        "erlang",
        "Erlang",
        LanguageKind::Programming,
        &["erl", "hrl"],
        Syntax {
            line_comments: vec![line("%")],
            strings: vec![StringRule::simple("\"")],
            ..Syntax::default()
        },
    );
    let clojure = line_count(
        "clojure",
        "Clojure",
        LanguageKind::Programming,
        &["clj", "cljs", "cljc", "edn"],
        Syntax {
            line_comments: vec![line(";")],
            strings: vec![StringRule::multiline("\"")],
            ..Syntax::default()
        },
    );
    let julia = line_count(
        "julia",
        "Julia",
        LanguageKind::Programming,
        &["jl"],
        Syntax {
            line_comments: vec![line("#")],
            block_comments: vec![block("#=", "=#", true)],
            strings: vec![StringRule::multiline("\"\"\""), StringRule::simple("\"")],
            ..Syntax::default()
        },
    );
    let fsharp = line_count(
        "fsharp",
        "F#",
        LanguageKind::Programming,
        &["fs", "fsi", "fsx"],
        Syntax {
            line_comments: vec![line("//")],
            block_comments: vec![block("(*", "*)", true)],
            strings: vec![StringRule::multiline("\"\"\""), StringRule::simple("\"")],
            ..Syntax::default()
        },
    );
    let ocaml = line_count(
        "ocaml",
        "OCaml",
        LanguageKind::Programming,
        &["ml", "mli"],
        Syntax {
            block_comments: vec![block("(*", "*)", true)],
            strings: vec![StringRule::simple("\"")],
            ..Syntax::default()
        },
    );
    let vb = line_count(
        "visual-basic",
        "Visual Basic",
        LanguageKind::Programming,
        &["vb", "bas", "vbs"],
        Syntax {
            line_comments: vec![line("'")],
            strings: vec![StringRule::raw("\"", "\"")],
            ..Syntax::default()
        },
    );
    let assembly = line_count(
        "assembly",
        "Assembly",
        LanguageKind::Programming,
        &["asm", "s", "nasm"],
        Syntax {
            line_comments: vec![line(";"), line_boundary("#")],
            ..Syntax::default()
        },
    );
    let shader = line_count(
        "shader",
        "Shader (GLSL/HLSL)",
        LanguageKind::Programming,
        &[
            "glsl", "vert", "frag", "hlsl", "wgsl", "metal", "comp", "geom",
        ],
        c_syntax(false),
    );
    let fortran = line_count(
        "fortran",
        "Fortran",
        LanguageKind::Programming,
        &["f90", "f95", "f03", "f08"],
        Syntax {
            line_comments: vec![line("!")],
            strings: vec![StringRule::simple("\""), StringRule::simple("'")],
            ..Syntax::default()
        },
    );
    vec![
        objc, haskell, erlang, clojure, julia, fsharp, ocaml, vb, assembly, shader, fortran,
    ]
}

/// Every built-in language specification.
pub fn builtin_languages() -> Vec<LanguageSpec> {
    let mut languages = vec![
        rust(),
        c(),
        cpp(),
        csharp(),
        java(),
        kotlin(),
        scala(),
        js_like("javascript", "JavaScript", &["js", "mjs", "cjs", "jsx"]),
        js_like("typescript", "TypeScript", &["ts", "mts", "cts", "tsx"]),
        python(),
        go(),
        php(),
        swift(),
        dart(),
        ruby(),
        shell(),
        powershell(),
        lua(),
        perl(),
        r_lang(),
        elixir(),
        groovy(),
        solidity(),
        sql(),
    ];
    languages[7].shebangs = names(&["node", "nodejs", "deno", "bun"]);
    languages[8].shebangs = names(&["ts-node", "tsx"]);
    languages.extend(markup_languages());
    languages.extend(data_languages());
    languages.extend(build_languages());
    languages.extend(other_programming());
    languages
}

#[cfg(test)]
mod tests {
    use super::*;
    use repodna_core::model::languages::ParserCapability;
    use std::collections::HashSet;

    #[test]
    fn every_builtin_pattern_compiles_and_ids_are_unique() {
        let languages = builtin_languages();
        let mut ids = HashSet::new();
        for language in &languages {
            assert!(
                ids.insert(language.id.clone()),
                "duplicate id {}",
                language.id
            );
        }
        assert!(languages.len() >= 60, "{} languages", languages.len());
    }

    #[test]
    fn extensions_are_unambiguous() {
        let mut seen = HashSet::new();
        for language in builtin_languages() {
            for extension in &language.extensions {
                assert!(
                    seen.insert(extension.clone()),
                    "extension {extension} is claimed twice"
                );
                assert_eq!(extension, &extension.to_ascii_lowercase());
            }
        }
    }

    #[test]
    fn required_languages_have_lexical_analysis() {
        let languages = builtin_languages();
        let lexical: Vec<_> = languages
            .iter()
            .filter(|l| l.capability() == ParserCapability::Lexical)
            .map(|l| l.id.as_str())
            .collect();
        for id in [
            "rust",
            "c",
            "cpp",
            "csharp",
            "java",
            "kotlin",
            "javascript",
            "typescript",
            "python",
            "go",
            "php",
            "swift",
            "dart",
            "ruby",
            "shell",
            "sql",
        ] {
            assert!(lexical.contains(&id), "{id} should be lexical");
        }
        for id in ["html", "css", "json", "yaml", "toml", "xml", "markdown"] {
            let language = languages.iter().find(|l| l.id == id).unwrap();
            assert_eq!(language.capability(), ParserCapability::LineCount, "{id}");
        }
    }
}
