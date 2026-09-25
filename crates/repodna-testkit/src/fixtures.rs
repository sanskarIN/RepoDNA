//! Fixture repositories for tests, examples, and benchmarks.
//!
//! Each fixture is a small, deterministic repository that demonstrates one situation
//! RepoDNA must handle: a tiny project, a polyglot service, a monorepo, rich or missing Git
//! history, duplicated code, dependency cycles, generated code, missing documentation,
//! suspicious secrets, failing builds and tests, unsupported languages, an archived
//! project, and a large repository. Authors are fictional, dates are fixed, and credential
//! lookalikes are assembled at runtime, so no file in this source looks like a secret.
//!
//! `cargo xtask fixtures` writes all of them to `fixtures/generated/`.

use std::fmt::Write as _;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

/// A fixture repository.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fixture {
    /// Directory name.
    pub name: &'static str,
    /// What it demonstrates.
    pub description: &'static str,
    /// Whether it is a Git repository.
    pub git: bool,
}

/// Every fixture, in a stable order.
pub const FIXTURES: &[Fixture] = &[
    Fixture {
        name: "tiny",
        description: "A tiny single-language project with a README, a license, and tests.",
        git: true,
    },
    Fixture {
        name: "polyglot",
        description: "A service in Go, TypeScript, Python, SQL, and shell with containers and CI.",
        git: true,
    },
    Fixture {
        name: "monorepo",
        description: "An npm workspace of packages that depend on each other.",
        git: true,
    },
    Fixture {
        name: "history",
        description: "Two years of history: contributors, releases, a rename, a restructuring, and a quiet period.",
        git: true,
    },
    Fixture {
        name: "no-git",
        description: "A Rust crate without Git history.",
        git: false,
    },
    Fixture {
        name: "duplicated",
        description: "Handlers that copy the same validation code.",
        git: true,
    },
    Fixture {
        name: "cyclic",
        description: "Modules and files that import each other in loops.",
        git: true,
    },
    Fixture {
        name: "generated",
        description: "Generated protocol code, a minified bundle, and linguist attributes next to handwritten code.",
        git: true,
    },
    Fixture {
        name: "undocumented",
        description: "Source code without a README, license, or comments.",
        git: true,
    },
    Fixture {
        name: "secrets",
        description: "Fake credentials in code and configuration, in test fixtures, and as placeholders.",
        git: true,
    },
    Fixture {
        name: "build-failure",
        description: "A project whose build command fails (seen only when execution is enabled).",
        git: true,
    },
    Fixture {
        name: "test-failure",
        description: "A project whose test suite fails (seen only when execution is enabled).",
        git: true,
    },
    Fixture {
        name: "unsupported",
        description: "Code in languages RepoDNA does not recognize without a plugin (Zig and COBOL).",
        git: true,
    },
    Fixture {
        name: "archived",
        description: "A project that stopped changing years ago, with deprecated tooling.",
        git: true,
    },
    Fixture {
        name: "large",
        description: "A generated repository with thousands of files across several languages.",
        git: true,
    },
];

/// Options for [`build`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FixtureOptions {
    /// Files in the `large` fixture.
    pub large_files: usize,
    /// Commits in the `large` fixture.
    pub large_commits: usize,
}

impl Default for FixtureOptions {
    fn default() -> Self {
        Self {
            large_files: 5_000,
            large_commits: 120,
        }
    }
}

/// Looks up a fixture by name.
pub fn fixture(name: &str) -> Option<&'static Fixture> {
    FIXTURES.iter().find(|fixture| fixture.name == name)
}

/// Writes fixture `name` into `root`, which must not exist yet or be empty.
pub fn build(name: &str, root: &Path, options: &FixtureOptions) -> io::Result<()> {
    let fixture = fixture(name).ok_or_else(|| {
        io::Error::new(io::ErrorKind::NotFound, format!("no fixture named {name}"))
    })?;
    if root.exists() && root.read_dir()?.next().is_some() {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!("{} is not empty", root.display()),
        ));
    }
    std::fs::create_dir_all(root)?;
    let repo = Repo::new(root, fixture.git)?;
    match name {
        "tiny" => tiny(&repo),
        "polyglot" => polyglot(&repo),
        "monorepo" => monorepo(&repo),
        "history" => history(&repo),
        "no-git" => no_git(&repo),
        "duplicated" => duplicated(&repo),
        "cyclic" => cyclic(&repo),
        "generated" => generated(&repo),
        "undocumented" => undocumented(&repo),
        "secrets" => secrets(&repo),
        "build-failure" => build_failure(&repo),
        "test-failure" => test_failure(&repo),
        "unsupported" => unsupported(&repo),
        "archived" => archived(&repo),
        _ => large(&repo, options),
    }
}

/// Fictional people, so no real person appears in fixture history.
const MIRA: (&str, &str) = ("Mira Stone", "mira@example.invalid");
const TOMAS: (&str, &str) = ("Tomas Reyes", "tomas@example.invalid");
const JUN: (&str, &str) = ("Jun Park", "jun@example.invalid");
const LENA: (&str, &str) = ("Lena Fischer", "lena@example.invalid");

/// A fixture being written: files, and commits when it is a Git repository.
struct Repo {
    root: PathBuf,
    git: bool,
}

impl Repo {
    fn new(root: &Path, git: bool) -> io::Result<Self> {
        let repo = Self {
            root: root.to_path_buf(),
            git,
        };
        if git {
            repo.run_git(&["init", "-q", "-b", "main"], &[])?;
        }
        Ok(repo)
    }

    fn write(&self, path: &str, contents: impl AsRef<[u8]>) -> io::Result<&Self> {
        let full = self.root.join(path);
        if let Some(parent) = full.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(full, contents)?;
        Ok(self)
    }

    fn remove(&self, path: &str) -> io::Result<&Self> {
        std::fs::remove_file(self.root.join(path))?;
        Ok(self)
    }

    fn run_git(&self, args: &[&str], env: &[(&str, &str)]) -> io::Result<String> {
        let output = Command::new("git")
            .current_dir(&self.root)
            .args([
                "-c",
                "commit.gpgsign=false",
                "-c",
                "tag.gpgsign=false",
                "-c",
                "core.autocrlf=false",
                "-c",
                "user.useConfigOnly=false",
            ])
            .args(args)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env(
                "GIT_CONFIG_GLOBAL",
                if cfg!(windows) { "NUL" } else { "/dev/null" },
            )
            .env("GIT_TERMINAL_PROMPT", "0")
            .envs(env.iter().copied())
            .output()?;
        if !output.status.success() {
            return Err(io::Error::other(format!(
                "git {} failed: {}",
                args.join(" "),
                String::from_utf8_lossy(&output.stderr).trim()
            )));
        }
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    }

    /// Commits everything with a fixed author and date (RFC 3339).
    fn commit(&self, message: &str, author: (&str, &str), date: &str) -> io::Result<&Self> {
        if !self.git {
            return Ok(self);
        }
        self.run_git(&["add", "-A"], &[])?;
        let env = [
            ("GIT_AUTHOR_NAME", author.0),
            ("GIT_AUTHOR_EMAIL", author.1),
            ("GIT_AUTHOR_DATE", date),
            ("GIT_COMMITTER_NAME", author.0),
            ("GIT_COMMITTER_EMAIL", author.1),
            ("GIT_COMMITTER_DATE", date),
        ];
        self.run_git(&["commit", "-q", "--allow-empty", "-m", message], &env)?;
        Ok(self)
    }

    /// Tags the current commit with an annotated release tag.
    fn tag(&self, name: &str, author: (&str, &str), date: &str) -> io::Result<&Self> {
        if !self.git {
            return Ok(self);
        }
        let env = [
            ("GIT_COMMITTER_NAME", author.0),
            ("GIT_COMMITTER_EMAIL", author.1),
            ("GIT_COMMITTER_DATE", date),
        ];
        self.run_git(&["tag", "-a", name, "-m", &format!("Release {name}")], &env)?;
        Ok(self)
    }
}

const MIT: &str = "MIT License\n\nCopyright (c) 2024 The Tinytool Authors\n\nPermission is hereby granted, free of charge, to any person obtaining a copy\nof this software and associated documentation files (the \"Software\"), to deal\nin the Software without restriction, including without limitation the rights\nto use, copy, modify, merge, publish, distribute, sublicense, and/or sell\ncopies of the Software, and to permit persons to whom the Software is\nfurnished to do so, subject to the following conditions:\n\nThe above copyright notice and this permission notice shall be included in all\ncopies or substantial portions of the Software.\n\nTHE SOFTWARE IS PROVIDED \"AS IS\", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR\nIMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,\nFITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE\nAUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER\nLIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,\nOUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE\nSOFTWARE.\n";

fn tiny(repo: &Repo) -> io::Result<()> {
    repo.write(
        "README.md",
        "# tinytool\n\nCounts words and lines in text files.\n\n## Installation\n\n```sh\npip install .\n```\n\n## Usage\n\n```sh\ntinytool count notes.txt\n```\n",
    )?
    .write("LICENSE", MIT)?
    .write(
        "pyproject.toml",
        "[project]\nname = \"tinytool\"\nversion = \"0.2.0\"\ndescription = \"Counts words and lines in text files.\"\nrequires-python = \">=3.10\"\ndependencies = [\"click>=8.1\"]\n\n[project.scripts]\ntinytool = \"tinytool.cli:main\"\n\n[project.optional-dependencies]\ntest = [\"pytest>=8\"]\n",
    )?
    .write("src/tinytool/__init__.py", "\"\"\"Word and line counting.\"\"\"\n\n__version__ = \"0.2.0\"\n")?
    .write(
        "src/tinytool/core.py",
        "\"\"\"Counting helpers.\"\"\"\n\n\ndef count_words(text: str) -> int:\n    \"\"\"Returns the number of whitespace-separated words.\"\"\"\n    return len(text.split())\n\n\ndef count_lines(text: str) -> int:\n    \"\"\"Returns the number of lines, counting a final line without a newline.\"\"\"\n    if not text:\n        return 0\n    return text.count(\"\\n\") + (0 if text.endswith(\"\\n\") else 1)\n",
    )?;
    repo.commit("Count words and lines", MIRA, "2024-03-02T10:00:00Z")?;
    repo.write(
        "src/tinytool/cli.py",
        "\"\"\"Command-line interface.\"\"\"\n\nimport click\n\nfrom tinytool.core import count_lines, count_words\n\n\n@click.group()\ndef main() -> None:\n    \"\"\"Counts words and lines in text files.\"\"\"\n\n\n@main.command()\n@click.argument(\"path\", type=click.Path(exists=True))\ndef count(path: str) -> None:\n    \"\"\"Prints word and line counts for PATH.\"\"\"\n    with open(path, encoding=\"utf-8\") as handle:\n        text = handle.read()\n    click.echo(f\"{count_words(text)} words, {count_lines(text)} lines\")\n",
    )?;
    repo.commit(
        "Add the command-line interface",
        MIRA,
        "2024-03-09T15:30:00Z",
    )?;
    repo.write(
        "tests/test_core.py",
        "from tinytool.core import count_lines, count_words\n\n\ndef test_counts_words() -> None:\n    assert count_words(\"one two  three\") == 3\n\n\ndef test_counts_lines() -> None:\n    assert count_lines(\"a\\nb\") == 2\n    assert count_lines(\"a\\nb\\n\") == 2\n    assert count_lines(\"\") == 0\n",
    )?;
    repo.commit("Test the counting helpers", MIRA, "2024-03-10T09:15:00Z")?;
    Ok(())
}

fn polyglot(repo: &Repo) -> io::Result<()> {
    repo.write(
        "README.md",
        "# Parcel tracker\n\nTracks parcels from pickup to delivery.\n\n- `server/`: the Go API\n- `web/`: the TypeScript dashboard\n- `scripts/`: Python maintenance scripts\n- `db/`: SQL migrations\n\n## Running locally\n\n```sh\ndocker compose up\n```\n",
    )?
    .write("server/go.mod", "module example.invalid/parcels\n\ngo 1.22\n\nrequire github.com/go-chi/chi/v5 v5.0.12\n")?
    .write(
        "server/cmd/api/main.go",
        "package main\n\nimport (\n\t\"log\"\n\t\"net/http\"\n\n\t\"example.invalid/parcels/internal/api\"\n)\n\nfunc main() {\n\tlog.Fatal(http.ListenAndServe(\":8080\", api.Router()))\n}\n",
    )?
    .write(
        "server/internal/api/router.go",
        "package api\n\nimport (\n\t\"encoding/json\"\n\t\"net/http\"\n\n\t\"github.com/go-chi/chi/v5\"\n\n\t\"example.invalid/parcels/internal/store\"\n)\n\n// Router returns the HTTP routes of the API.\nfunc Router() http.Handler {\n\tr := chi.NewRouter()\n\tr.Get(\"/parcels/{id}\", func(w http.ResponseWriter, req *http.Request) {\n\t\tparcel, ok := store.Find(chi.URLParam(req, \"id\"))\n\t\tif !ok {\n\t\t\thttp.NotFound(w, req)\n\t\t\treturn\n\t\t}\n\t\t_ = json.NewEncoder(w).Encode(parcel)\n\t})\n\treturn r\n}\n",
    )?
    .write(
        "server/internal/store/store.go",
        "package store\n\n// Parcel is a tracked shipment.\ntype Parcel struct {\n\tID     string `json:\"id\"`\n\tStatus string `json:\"status\"`\n}\n\nvar parcels = map[string]Parcel{\"p1\": {ID: \"p1\", Status: \"in transit\"}}\n\n// Find looks up a parcel by identifier.\nfunc Find(id string) (Parcel, bool) {\n\tparcel, ok := parcels[id]\n\treturn parcel, ok\n}\n",
    )?
    .write(
        "server/internal/store/store_test.go",
        "package store\n\nimport \"testing\"\n\nfunc TestFind(t *testing.T) {\n\tif _, ok := Find(\"p1\"); !ok {\n\t\tt.Fatal(\"p1 should exist\")\n\t}\n}\n",
    )?;
    repo.commit("Serve parcels over HTTP", TOMAS, "2024-01-08T09:00:00Z")?;
    repo.write(
        "web/package.json",
        "{\n  \"name\": \"parcel-dashboard\",\n  \"version\": \"0.1.0\",\n  \"private\": true,\n  \"scripts\": {\n    \"build\": \"tsc -p .\",\n    \"test\": \"vitest run\"\n  },\n  \"dependencies\": {\n    \"lit\": \"^3.1.0\"\n  },\n  \"devDependencies\": {\n    \"typescript\": \"^5.4.0\",\n    \"vitest\": \"^1.4.0\"\n  }\n}\n",
    )?
    .write(
        "web/src/api.ts",
        "export interface Parcel {\n  id: string;\n  status: string;\n}\n\nexport async function fetchParcel(id: string): Promise<Parcel> {\n  const response = await fetch(`/parcels/${encodeURIComponent(id)}`);\n  if (!response.ok) {\n    throw new Error(`parcel ${id} not found`);\n  }\n  return (await response.json()) as Parcel;\n}\n",
    )?
    .write(
        "web/src/main.ts",
        "import { fetchParcel } from \"./api\";\n\nconst form = document.querySelector(\"form\");\nform?.addEventListener(\"submit\", async (event) => {\n  event.preventDefault();\n  const id = new FormData(form).get(\"id\");\n  if (typeof id === \"string\") {\n    const parcel = await fetchParcel(id);\n    document.body.dataset.status = parcel.status;\n  }\n});\n",
    )?
    .write("web/src/api.test.ts", "import { describe, expect, it } from \"vitest\";\n\ndescribe(\"api\", () => {\n  it(\"loads\", () => {\n    expect(typeof fetch).toBe(\"function\");\n  });\n});\n")?;
    repo.commit("Add the dashboard", JUN, "2024-02-12T14:00:00Z")?;
    repo.write(
        "db/migrations/001_parcels.sql",
        "CREATE TABLE parcels (\n  id TEXT PRIMARY KEY,\n  status TEXT NOT NULL,\n  updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP\n);\n",
    )?
    .write(
        "scripts/purge_delivered.py",
        "\"\"\"Deletes parcels delivered more than a year ago.\"\"\"\n\nimport sqlite3\nimport sys\n\n\ndef main(path: str) -> int:\n    with sqlite3.connect(path) as db:\n        db.execute(\"DELETE FROM parcels WHERE status = 'delivered' AND updated_at < date('now', '-1 year')\")\n    return 0\n\n\nif __name__ == \"__main__\":\n    sys.exit(main(sys.argv[1]))\n",
    )?
    .write("scripts/deploy.sh", "#!/usr/bin/env sh\nset -eu\ndocker compose build\ndocker compose up -d\n")?
    .write(
        "Dockerfile",
        "FROM golang:1.22 AS build\nWORKDIR /src\nCOPY server/ .\nRUN go build -o /api ./cmd/api\n\nFROM gcr.io/distroless/base-debian12\nCOPY --from=build /api /api\nUSER nonroot\nENTRYPOINT [\"/api\"]\n",
    )?
    .write(
        "docker-compose.yml",
        "services:\n  api:\n    build: .\n    ports:\n      - \"8080:8080\"\n",
    )?
    .write(
        ".github/workflows/ci.yml",
        "name: CI\non: [push, pull_request]\njobs:\n  server:\n    runs-on: ubuntu-latest\n    steps:\n      - uses: actions/checkout@v4\n      - uses: actions/setup-go@v5\n        with:\n          go-version: \"1.22\"\n      - run: go test ./...\n        working-directory: server\n",
    )?
    .write("Makefile", "test:\n\tcd server && go test ./...\n\tcd web && npm test\n")?;
    repo.commit(
        "Add migrations, scripts, containers, and CI",
        TOMAS,
        "2024-03-20T11:45:00Z",
    )?;
    Ok(())
}

fn monorepo(repo: &Repo) -> io::Result<()> {
    repo.write(
        "package.json",
        "{\n  \"name\": \"notes-monorepo\",\n  \"private\": true,\n  \"description\": \"A note-taking app split into packages.\",\n  \"workspaces\": [\"packages/*\", \"apps/*\"],\n  \"scripts\": {\n    \"build\": \"npm run build --workspaces\",\n    \"test\": \"npm test --workspaces\"\n  }\n}\n",
    )?
    .write("README.md", "# Notes\n\nA note-taking app split into packages.\n\n## Usage\n\n```sh\nnpm install\nnpm test\n```\n")?;
    let packages: [(&str, &[&str], &str); 4] = [
        (
            "core",
            &[],
            "export interface Note {\n  id: string;\n  text: string;\n}\n\nexport function createNote(id: string, text: string): Note {\n  return { id, text: text.trim() };\n}\n",
        ),
        (
            "storage",
            &["core"],
            "import { createNote, type Note } from \"@notes/core\";\n\nconst notes = new Map<string, Note>();\n\nexport function save(id: string, text: string): Note {\n  const note = createNote(id, text);\n  notes.set(id, note);\n  return note;\n}\n\nexport function load(id: string): Note | undefined {\n  return notes.get(id);\n}\n",
        ),
        (
            "search",
            &["core", "storage"],
            "import { load } from \"@notes/storage\";\nimport type { Note } from \"@notes/core\";\n\nexport function find(ids: string[], term: string): Note[] {\n  return ids\n    .map((id) => load(id))\n    .filter((note): note is Note => note !== undefined && note.text.includes(term));\n}\n",
        ),
        (
            "ui",
            &["core"],
            "import type { Note } from \"@notes/core\";\n\nexport function render(note: Note): string {\n  return `<article data-id=\"${note.id}\">${note.text}</article>`;\n}\n",
        ),
    ];
    for (name, dependencies, source) in packages {
        let deps: Vec<String> = dependencies
            .iter()
            .map(|dep| format!("    \"@notes/{dep}\": \"1.0.0\""))
            .collect();
        let deps = if deps.is_empty() {
            String::new()
        } else {
            format!(",\n  \"dependencies\": {{\n{}\n  }}", deps.join(",\n"))
        };
        repo.write(
            &format!("packages/{name}/package.json"),
            format!(
                "{{\n  \"name\": \"@notes/{name}\",\n  \"version\": \"1.0.0\",\n  \"main\": \"src/index.ts\",\n  \"scripts\": {{ \"test\": \"vitest run\" }}{deps}\n}}\n"
            ),
        )?
        .write(&format!("packages/{name}/src/index.ts"), source)?
        .write(
            &format!("packages/{name}/src/index.test.ts"),
            "import { describe, expect, it } from \"vitest\";\n\ndescribe(\"package\", () => {\n  it(\"loads\", () => {\n    expect(1 + 1).toBe(2);\n  });\n});\n",
        )?;
    }
    repo.write(
        "apps/web/package.json",
        "{\n  \"name\": \"@notes/web\",\n  \"version\": \"1.0.0\",\n  \"private\": true,\n  \"dependencies\": {\n    \"@notes/search\": \"1.0.0\",\n    \"@notes/ui\": \"1.0.0\"\n  }\n}\n",
    )?
    .write(
        "apps/web/src/main.ts",
        "import { find } from \"@notes/search\";\nimport { render } from \"@notes/ui\";\n\nconst results = find([\"a\", \"b\"], \"todo\");\ndocument.body.innerHTML = results.map(render).join(\"\");\n",
    )?;
    repo.commit("Split the app into packages", LENA, "2024-05-02T08:30:00Z")?;
    Ok(())
}

fn history(repo: &Repo) -> io::Result<()> {
    repo.write("README.md", "# Ledger\n\nA double-entry bookkeeping library.\n")?
        .write("setup.py", "from setuptools import setup\n\nsetup(name=\"ledger\", version=\"0.1.0\", packages=[\"ledger\"])\n")?
        .write("ledger/__init__.py", "\"\"\"Double-entry bookkeeping.\"\"\"\n")?
        .write("ledger/accounts.py", "class Account:\n    def __init__(self, name):\n        self.name = name\n        self.balance = 0\n")?;
    repo.commit("Start the ledger", MIRA, "2022-01-10T09:00:00Z")?;
    repo.write(
        "ledger/journal.py",
        "from ledger.accounts import Account\n\n\ndef post(debit: Account, credit: Account, amount: int) -> None:\n    debit.balance += amount\n    credit.balance -= amount\n",
    )?;
    repo.commit("Post journal entries", MIRA, "2022-01-24T16:20:00Z")?
        .tag("v0.1.0", MIRA, "2022-01-24T16:30:00Z")?;
    repo.write(
        "ledger/reports.py",
        "from ledger.accounts import Account\n\n\ndef trial_balance(accounts: list[Account]) -> int:\n    return sum(account.balance for account in accounts)\n",
    )?;
    repo.commit(
        "Add the trial balance report",
        TOMAS,
        "2022-03-03T11:00:00Z",
    )?;
    repo.write(
        "tests/test_journal.py",
        "from ledger.accounts import Account\nfrom ledger.journal import post\n\n\ndef test_post_balances():\n    cash, sales = Account(\"cash\"), Account(\"sales\")\n    post(cash, sales, 100)\n    assert cash.balance + sales.balance == 0\n",
    )?;
    repo.commit("Test journal posting", TOMAS, "2022-03-15T10:10:00Z")?;
    repo.write(
        ".github/workflows/test.yml",
        "name: Test\non: [push]\njobs:\n  test:\n    runs-on: ubuntu-latest\n    steps:\n      - uses: actions/checkout@v4\n      - run: python -m pytest\n",
    )?;
    repo.commit("Run tests in CI", JUN, "2022-04-02T13:00:00Z")?
        .tag("v0.2.0", JUN, "2022-04-02T13:05:00Z")?;
    for (index, month) in ["05", "06", "07", "08", "09", "10"].into_iter().enumerate() {
        let mut body = String::from("CURRENCIES = {\n");
        for currency in ["EUR", "USD", "JPY", "GBP", "INR", "BRL", "CAD"]
            .iter()
            .take(index + 2)
        {
            let _ = writeln!(body, "    \"{currency}\": 2,");
        }
        body.push_str("}\n");
        repo.write("ledger/currencies.py", body)?;
        let author = if index % 2 == 0 { TOMAS } else { JUN };
        repo.commit(
            &format!("Support {} currencies", index + 2),
            author,
            &format!("2022-{month}-14T10:00:00Z"),
        )?;
    }
    repo.tag("v0.3.0", TOMAS, "2022-10-20T09:00:00Z")?;
    // A restructuring: the package moves under src/.
    repo.run_git(&["mv", "ledger", "src_ledger_tmp"], &[])?;
    std::fs::create_dir_all(repo.root.join("src"))?;
    repo.run_git(&["mv", "src_ledger_tmp", "src/ledger"], &[])?;
    repo.write("setup.py", "from setuptools import find_packages, setup\n\nsetup(name=\"ledger\", version=\"0.4.0\", package_dir={\"\": \"src\"}, packages=find_packages(\"src\"))\n")?;
    repo.commit("Move the package under src/", MIRA, "2022-11-07T15:00:00Z")?;
    // A quiet period of about seven months, then a TypeScript client appears.
    repo.write(
        "clients/ts/package.json",
        "{\n  \"name\": \"ledger-client\",\n  \"version\": \"0.1.0\",\n  \"dependencies\": { \"zod\": \"^3.22.0\" }\n}\n",
    )?
    .write(
        "clients/ts/src/client.ts",
        "export interface Entry {\n  debit: string;\n  credit: string;\n  amount: number;\n}\n\nexport async function post(entry: Entry): Promise<void> {\n  await fetch(\"/entries\", { method: \"POST\", body: JSON.stringify(entry) });\n}\n",
    )?;
    repo.commit("Add a TypeScript client", LENA, "2023-06-19T10:30:00Z")?;
    repo.remove("src/ledger/reports.py")?.write(
        "src/ledger/statements.py",
        "from ledger.accounts import Account\n\n\ndef balance_sheet(accounts: list[Account]) -> dict[str, int]:\n    return {account.name: account.balance for account in accounts}\n",
    )?;
    repo.commit(
        "Replace the trial balance with statements",
        LENA,
        "2023-08-01T09:45:00Z",
    )?
    .tag("v1.0.0", LENA, "2023-08-01T10:00:00Z")?;
    repo.write("CHANGELOG.md", "# Changelog\n\n## 1.0.0\n\n- Statements replace the trial balance.\n- A TypeScript client.\n")?;
    repo.commit("Write the changelog", MIRA, "2024-01-15T12:00:00Z")?;
    Ok(())
}

fn no_git(repo: &Repo) -> io::Result<()> {
    repo.write(
        "Cargo.toml",
        "[package]\nname = \"temperature\"\nversion = \"0.1.0\"\nedition = \"2021\"\ndescription = \"Converts between temperature scales.\"\nlicense = \"MIT\"\n\n[dependencies]\n",
    )?
    .write("README.md", "# temperature\n\nConverts between temperature scales.\n\n## Usage\n\n```sh\ncargo run -- 21.5c\n```\n")?
    .write(
        "src/lib.rs",
        "//! Temperature conversions.\n\n/// Converts Celsius to Fahrenheit.\npub fn to_fahrenheit(celsius: f64) -> f64 {\n    celsius * 9.0 / 5.0 + 32.0\n}\n\n#[cfg(test)]\nmod tests {\n    use super::*;\n\n    #[test]\n    fn converts_freezing_point() {\n        assert_eq!(to_fahrenheit(0.0), 32.0);\n    }\n}\n",
    )?
    .write(
        "src/main.rs",
        "use temperature::to_fahrenheit;\n\nfn main() {\n    let celsius: f64 = std::env::args()\n        .nth(1)\n        .and_then(|value| value.trim_end_matches('c').parse().ok())\n        .unwrap_or(0.0);\n    println!(\"{:.1}F\", to_fahrenheit(celsius));\n}\n",
    )?;
    Ok(())
}

/// A validation routine long enough to count as duplicated code when copied.
fn validation(entity: &str) -> String {
    format!(
        "function validate{entity}(input) {{\n  const errors = [];\n  if (!input || typeof input !== \"object\") {{\n    errors.push(\"the request body must be an object\");\n    return errors;\n  }}\n  if (typeof input.customerId !== \"string\" || input.customerId.length === 0) {{\n    errors.push(\"customerId is required\");\n  }}\n  if (!Number.isFinite(input.amount) || input.amount <= 0) {{\n    errors.push(\"amount must be a positive number\");\n  }}\n  if (typeof input.currency !== \"string\" || !/^[A-Z]{{3}}$/.test(input.currency)) {{\n    errors.push(\"currency must be a three-letter code\");\n  }}\n  if (input.note !== undefined && typeof input.note !== \"string\") {{\n    errors.push(\"note must be text\");\n  }}\n  if (Array.isArray(input.items)) {{\n    for (const item of input.items) {{\n      if (typeof item.sku !== \"string\" || item.quantity < 1) {{\n        errors.push(\"every item needs a sku and a quantity\");\n      }}\n    }}\n  }}\n  return errors;\n}}\n"
    )
}

fn duplicated(repo: &Repo) -> io::Result<()> {
    repo.write("README.md", "# Billing handlers\n\nHTTP handlers for orders, invoices, and refunds.\n")?
        .write("package.json", "{\n  \"name\": \"billing-handlers\",\n  \"version\": \"1.0.0\",\n  \"type\": \"module\"\n}\n")?;
    for (file, entity) in [
        ("src/orders.js", "Order"),
        ("src/invoices.js", "Invoice"),
        ("src/refunds.js", "Refund"),
        ("src/credits.js", "Credit"),
    ] {
        let lower = entity.to_ascii_lowercase();
        repo.write(
            file,
            format!(
                "{}\nexport function handle{entity}(request) {{\n  const errors = validate{entity}(request.body);\n  if (errors.length > 0) {{\n    return {{ status: 400, errors }};\n  }}\n  return {{ status: 201, {lower}: request.body }};\n}}\n",
                validation(entity)
            ),
        )?;
    }
    repo.commit("Add billing handlers", TOMAS, "2024-04-04T10:00:00Z")?;
    Ok(())
}

fn cyclic(repo: &Repo) -> io::Result<()> {
    repo.write("README.md", "# Accounts\n\nAccounts and billing that depend on each other.\n")?
        .write("package.json", "{\n  \"name\": \"accounts\",\n  \"version\": \"1.0.0\",\n  \"type\": \"module\"\n}\n")?
        .write(
            "src/accounts/account.ts",
            "import { invoicesFor } from \"../billing/invoice\";\n\nexport interface Account {\n  id: string;\n}\n\nexport function balance(account: Account): number {\n  return invoicesFor(account.id).reduce((sum, invoice) => sum + invoice.total, 0);\n}\n",
        )?
        .write(
            "src/billing/invoice.ts",
            "import { findAccount } from \"../accounts/lookup\";\n\nexport interface Invoice {\n  accountId: string;\n  total: number;\n}\n\nexport function invoicesFor(accountId: string): Invoice[] {\n  const account = findAccount(accountId);\n  return account ? [{ accountId, total: 10 }] : [];\n}\n",
        )?
        .write(
            "src/accounts/lookup.ts",
            "import { balance, type Account } from \"./account\";\n\nconst accounts: Account[] = [{ id: \"a1\" }];\n\nexport function findAccount(id: string): Account | undefined {\n  return accounts.find((account) => account.id === id);\n}\n\nexport function richest(): Account | undefined {\n  return [...accounts].sort((a, b) => balance(b) - balance(a))[0];\n}\n",
        )?
        .write(
            "src/reports/summary.ts",
            "import { balance } from \"../accounts/account\";\nimport { findAccount } from \"../accounts/lookup\";\n\nexport function summary(id: string): string {\n  const account = findAccount(id);\n  return account ? `${id}: ${balance(account)}` : `${id}: unknown`;\n}\n",
        )?;
    repo.commit("Link accounts and billing", JUN, "2024-06-01T09:00:00Z")?;
    Ok(())
}

fn generated(repo: &Repo) -> io::Result<()> {
    repo.write("README.md", "# Weather API\n\nA service with generated protocol code.\n")?
        .write("go.mod", "module example.invalid/weather\n\ngo 1.22\n")?
        .write(".gitattributes", "api/*.pb.ts linguist-generated\n")?
        .write(
            "server/handler.go",
            "package server\n\nimport \"example.invalid/weather/api\"\n\n// Forecast returns a fixed forecast.\nfunc Forecast(city string) api.Forecast {\n\treturn api.Forecast{City: city, Celsius: 21}\n}\n",
        )?;
    let mut protocol = String::from(
        "// Code generated by protoc-gen-go. DO NOT EDIT.\n// source: weather.proto\n\npackage api\n\n",
    );
    for index in 0..40 {
        let _ = write!(
            protocol,
            "// Field{index} is generated.\nfunc (x *Forecast) GetField{index}() int32 {{\n\tif x != nil {{\n\t\treturn x.Field{index}\n\t}}\n\treturn 0\n}}\n\n"
        );
    }
    protocol.push_str("type Forecast struct {\n\tCity    string\n\tCelsius int32\n");
    for index in 0..40 {
        let _ = writeln!(protocol, "\tField{index} int32");
    }
    protocol.push_str("}\n");
    repo.write("api/weather.pb.go", protocol)?;
    let mut typescript =
        String::from("export interface Forecast {\n  city: string;\n  celsius: number;\n}\n");
    for index in 0..30 {
        let _ = writeln!(typescript, "export const FIELD_{index} = {index};");
    }
    repo.write("api/weather.pb.ts", typescript)?;
    let bundle: String = (0..400)
        .map(|index| format!("function f{index}(a){{return a+{index}}}"))
        .collect::<Vec<_>>()
        .join(";");
    repo.write("web/dist/app.min.js", bundle)?;
    repo.commit("Generate the protocol code", LENA, "2024-07-01T10:00:00Z")?;
    Ok(())
}

fn undocumented(repo: &Repo) -> io::Result<()> {
    repo.write(
        "src/main.c",
        "#include <stdio.h>\n#include \"queue.h\"\n\nint main(void) {\n    struct queue q = {0};\n    push(&q, 4);\n    push(&q, 2);\n    printf(\"%d\\n\", pop(&q));\n    return 0;\n}\n",
    )?
    .write(
        "src/queue.h",
        "#ifndef QUEUE_H\n#define QUEUE_H\n\nstruct queue {\n    int items[64];\n    int head;\n    int tail;\n};\n\nvoid push(struct queue *q, int value);\nint pop(struct queue *q);\n\n#endif\n",
    )?
    .write(
        "src/queue.c",
        "#include \"queue.h\"\n\nvoid push(struct queue *q, int value) {\n    q->items[q->tail++ % 64] = value;\n}\n\nint pop(struct queue *q) {\n    return q->items[q->head++ % 64];\n}\n",
    )?
    .write("Makefile", "queue: src/main.c src/queue.c\n\tcc -o queue src/main.c src/queue.c\n")?;
    repo.commit("queue", JUN, "2024-02-02T20:00:00Z")?;
    Ok(())
}

fn secrets(repo: &Repo) -> io::Result<()> {
    // Assembled here so that this source file contains no credential-shaped literal.
    let aws = ["AK", "IA", "Z7Q2X9W4R5T1Y8U3"].concat();
    let github = ["gh", "p_", "k2Lm9Qx4Tz8Rw1Yv6Bn3Hc7Fd5Js0Pa2Ge4W"].concat();
    let private_key = [
        "-----BEGIN ",
        "RSA PRIVATE KEY-----\nMIIEowIBAAKCAQEAu3Z4q9vFakeKeyMaterialForTests0nly\nQ2F0cyBhcmUgZ3JlYXQgYW5kIHNvIGFyZSBmaXh0dXJlcw\n-----END ",
        "RSA PRIVATE KEY-----\n",
    ]
    .concat();
    let password = ["Tr0ub4", "dor&3xK9!q"].concat();
    repo.write("README.md", "# Deploy tools\n\nScripts that deploy the storefront.\n")?
        .write(
            "deploy/config.py",
            format!("AWS_ACCESS_KEY_ID = \"{aws}\"\nREGION = \"eu-west-1\"\n"),
        )?
        .write(
            "deploy/github.py",
            format!("import requests\n\nTOKEN = \"{github}\"\n\n\ndef release(tag):\n    return requests.post(\"https://api.github.com/repos/example/storefront/releases\", headers={{\"Authorization\": f\"token {{TOKEN}}\"}}, json={{\"tag_name\": tag}})\n"),
        )?
        .write("deploy/server.key", private_key)?
        .write(".env", format!("DATABASE_PASSWORD={password}\n"))?
        .write(
            "settings.example.toml",
            "# Copy to settings.toml and fill in real values.\napi_key = \"your-api-key-here\"\npassword = \"changeme\"\n",
        )?
        .write(
            "tests/fixtures/credentials.json",
            format!("{{\n  \"aws_access_key_id\": \"{aws}\"\n}}\n"),
        )?
        .write(
            "deploy/verify.py",
            "import requests\n\n\ndef check(url):\n    return requests.get(url, verify=False)\n",
        )?;
    repo.commit("Add deployment scripts", TOMAS, "2024-08-08T08:08:00Z")?;
    Ok(())
}

fn build_failure(repo: &Repo) -> io::Result<()> {
    repo.write("README.md", "# Broken build\n\nThe build script exits with an error.\n")?
        .write(
            "package.json",
            "{\n  \"name\": \"broken-build\",\n  \"version\": \"1.0.0\",\n  \"scripts\": {\n    \"build\": \"node build.js\"\n  }\n}\n",
        )?
        .write(
            "build.js",
            "const missing = \"src/entry.js\";\nconsole.error(`build failed: ${missing} does not exist`);\nprocess.exit(1);\n",
        )?
        .write("src/index.js", "export const answer = 42;\n")?;
    repo.commit("Add a build script", MIRA, "2024-09-01T10:00:00Z")?;
    Ok(())
}

fn test_failure(repo: &Repo) -> io::Result<()> {
    repo.write("README.md", "# Failing tests\n\nOne test expects the wrong result.\n")?
        .write("Makefile", "test:\n\tpython3 -m unittest discover -s tests\n")?
        .write("calc.py", "def add(a, b):\n    return a + b\n")?
        .write(
            "tests/test_calc.py",
            "import sys\nimport unittest\n\nsys.path.insert(0, \".\")\n\nfrom calc import add  # noqa: E402\n\n\nclass AddTest(unittest.TestCase):\n    def test_adds(self):\n        self.assertEqual(add(2, 2), 4)\n\n    def test_expects_the_wrong_sum(self):\n        self.assertEqual(add(2, 2), 5)\n\n\nif __name__ == \"__main__\":\n    unittest.main()\n",
        )?;
    repo.commit("Add a calculator with tests", JUN, "2024-09-02T10:00:00Z")?;
    Ok(())
}

fn unsupported(repo: &Repo) -> io::Result<()> {
    repo.write("README.md", "# Legacy payroll\n\nPayroll in COBOL with new tools in Zig.\n")?
        .write(
            "payroll/PAYROLL.cob",
            "       IDENTIFICATION DIVISION.\n       PROGRAM-ID. PAYROLL.\n       DATA DIVISION.\n       WORKING-STORAGE SECTION.\n       01 WS-HOURS PIC 9(3) VALUE 40.\n       01 WS-RATE  PIC 9(3) VALUE 25.\n       01 WS-PAY   PIC 9(5).\n       PROCEDURE DIVISION.\n           MULTIPLY WS-HOURS BY WS-RATE GIVING WS-PAY.\n           DISPLAY WS-PAY.\n           STOP RUN.\n",
        )?
        .write(
            "tools/src/main.zig",
            "const std = @import(\"std\");\n\npub fn main() !void {\n    const stdout = std.io.getStdOut().writer();\n    try stdout.print(\"{d}\\n\", .{40 * 25});\n}\n",
        )?
        .write("tools/build.zig", "const std = @import(\"std\");\n\npub fn build(b: *std.Build) void {\n    _ = b;\n}\n")?;
    repo.commit("Add payroll and tools", LENA, "2024-10-01T10:00:00Z")?;
    Ok(())
}

fn archived(repo: &Repo) -> io::Result<()> {
    repo.write(
        "README.md",
        "# jQuery carousel\n\n**Archived:** this project is no longer maintained.\n\nA carousel plugin for jQuery.\n",
    )?
    .write(
        "bower.json",
        "{\n  \"name\": \"jquery-carousel\",\n  \"version\": \"1.4.2\",\n  \"dependencies\": {\n    \"jquery\": \"~1.11.0\"\n  }\n}\n",
    )?
    .write(
        "package.json",
        "{\n  \"name\": \"jquery-carousel\",\n  \"version\": \"1.4.2\",\n  \"devDependencies\": {\n    \"grunt\": \"~0.4.5\",\n    \"grunt-contrib-uglify\": \"~0.5.0\"\n  }\n}\n",
    )?
    .write("Gruntfile.js", "module.exports = function (grunt) {\n  grunt.initConfig({ uglify: { dist: { files: { \"dist/carousel.min.js\": [\"src/carousel.js\"] } } } });\n  grunt.loadNpmTasks(\"grunt-contrib-uglify\");\n};\n")?
    .write(".travis.yml", "language: node_js\nnode_js:\n  - \"0.10\"\n")?
    .write(
        "src/carousel.js",
        "(function ($) {\n  // TODO: support touch events\n  $.fn.carousel = function (options) {\n    var settings = $.extend({ interval: 5000 }, options);\n    return this.each(function () {\n      var slides = $(this).children();\n      var index = 0;\n      setInterval(function () {\n        slides.eq(index).hide();\n        index = (index + 1) % slides.length;\n        slides.eq(index).show();\n      }, settings.interval);\n    });\n  };\n})(jQuery);\n",
    )?;
    repo.commit("Initial carousel", MIRA, "2014-05-04T18:00:00Z")?
        .tag("v1.0.0", MIRA, "2014-05-04T18:10:00Z")?;
    repo.write(
        "src/carousel.js",
        "(function ($) {\n  // TODO: support touch events\n  // FIXME: the interval keeps running after the element is removed\n  $.fn.carousel = function (options) {\n    var settings = $.extend({ interval: 5000, pauseOnHover: true }, options);\n    return this.each(function () {\n      var slides = $(this).children();\n      var index = 0;\n      var paused = false;\n      $(this).hover(function () { paused = settings.pauseOnHover; }, function () { paused = false; });\n      setInterval(function () {\n        if (paused) { return; }\n        slides.eq(index).hide();\n        index = (index + 1) % slides.length;\n        slides.eq(index).show();\n      }, settings.interval);\n    });\n  };\n})(jQuery);\n",
    )?;
    repo.commit("Pause on hover", TOMAS, "2015-02-11T12:00:00Z")?
        .tag("v1.4.0", TOMAS, "2015-02-11T12:05:00Z")?;
    repo.write(
        "CHANGELOG.md",
        "# Changelog\n\n## 1.4.2\n\n- Fix the fade timing.\n\n## 1.4.0\n\n- Pause on hover.\n",
    )?;
    repo.commit("Fix the fade timing", TOMAS, "2016-07-22T09:00:00Z")?
        .tag("v1.4.2", TOMAS, "2016-07-22T09:05:00Z")?;
    repo.write(
        "DEPRECATED.md",
        "This plugin is deprecated. Use the browser's scroll-snap instead.\n",
    )?;
    repo.commit("Deprecate the plugin", MIRA, "2017-11-30T17:00:00Z")?;
    Ok(())
}

fn large(repo: &Repo, options: &FixtureOptions) -> io::Result<()> {
    const SERVICES: [&str; 8] = [
        "accounts", "billing", "catalog", "delivery", "email", "fraud", "gateway", "history",
    ];
    repo.write(
        "README.md",
        "# Commerce platform (simulated)\n\nA generated repository for testing RepoDNA at scale.\n\n## Usage\n\n```sh\nmake build\n```\n",
    )?
    .write("go.mod", "module example.invalid/commerce\n\ngo 1.22\n")?
    .write("Makefile", "build:\n\tgo build ./...\n\ntest:\n\tgo test ./...\n")?;
    let files = options.large_files.max(SERVICES.len() * 3);
    let commits = options.large_commits.max(1);
    let per_commit = files.div_ceil(commits);
    let mut written = 0;
    let mut commit = 0;
    while written < files {
        let batch_end = (written + per_commit).min(files);
        for index in written..batch_end {
            let service = SERVICES[index % SERVICES.len()];
            let part = index / SERVICES.len();
            let (path, contents) = match index % 3 {
                0 => (
                    format!("services/{service}/internal/part{part:05}.go"),
                    format!(
                        "package internal\n\n// Handle{part} processes one {service} event.\nfunc Handle{part}(input int) int {{\n\tif input < 0 {{\n\t\treturn 0\n\t}}\n\tfor i := 0; i < {limit}; i++ {{\n\t\tinput += i\n\t}}\n\treturn input\n}}\n",
                        limit = part % 17 + 1
                    ),
                ),
                1 => (
                    format!("web/{service}/component{part:05}.ts"),
                    format!(
                        "import {{ format }} from \"../shared/format\";\n\nexport function render{part}(value: number): string {{\n  return value > {part} ? format(value) : \"\";\n}}\n"
                    ),
                ),
                _ => (
                    format!("tools/{service}/task_{part:05}.py"),
                    format!(
                        "\"\"\"Maintenance task {part} for {service}.\"\"\"\n\n\ndef run(items):\n    return [item for item in items if item % {modulus} == 0]\n",
                        modulus = part % 7 + 2
                    ),
                ),
            };
            repo.write(&path, contents)?;
        }
        if written == 0 {
            repo.write(
                "web/shared/format.ts",
                "export function format(value: number): string {\n  return value.toLocaleString(\"en-US\");\n}\n",
            )?;
        }
        written = batch_end;
        let author = [MIRA, TOMAS, JUN, LENA][commit % 4];
        let day = commit % 28 + 1;
        let month = (commit / 28) % 12 + 1;
        let year = 2021 + commit / (28 * 12);
        repo.commit(
            &format!("Add generated modules ({written} files)"),
            author,
            &format!("{year}-{month:02}-{day:02}T10:00:00Z"),
        )?;
        commit += 1;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixture_names_are_unique_and_described() {
        let mut names: Vec<&str> = FIXTURES.iter().map(|f| f.name).collect();
        let count = names.len();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), count);
        assert_eq!(count, 15);
        assert!(FIXTURES.iter().all(|f| !f.description.is_empty()));
        assert!(fixture("tiny").is_some());
        assert!(fixture("nope").is_none());
    }

    #[test]
    fn builds_a_fixture_without_git_and_refuses_to_overwrite() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("no-git");
        build("no-git", &root, &FixtureOptions::default()).unwrap();
        assert!(root.join("Cargo.toml").is_file());
        assert!(!root.join(".git").exists());
        assert!(build("no-git", &root, &FixtureOptions::default()).is_err());
        assert!(build("unknown", &dir.path().join("x"), &FixtureOptions::default()).is_err());
    }

    #[test]
    fn builds_every_git_fixture() {
        if !crate::git_available() {
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let options = FixtureOptions {
            large_files: 60,
            large_commits: 4,
        };
        for fixture in FIXTURES.iter().filter(|f| f.git) {
            let root = dir.path().join(fixture.name);
            build(fixture.name, &root, &options)
                .unwrap_or_else(|error| panic!("{}: {error}", fixture.name));
            assert!(root.join(".git").is_dir(), "{}", fixture.name);
        }
        let tags = Repo {
            root: dir.path().join("history"),
            git: true,
        }
        .run_git(&["tag"], &[])
        .unwrap();
        assert_eq!(tags.lines().count(), 4);
    }
}
