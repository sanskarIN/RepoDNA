#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
"""RepoDNA example analyzer plugin: which source files declare an SPDX license identifier?

Protocol (api 1): RepoDNA writes one JSON request to standard input and reads one JSON
response from standard output. Anything written to standard error is shown to the user
only if the plugin fails. Uses the Python standard library only.
"""

import json
import os
import sys

# How far into each file to look for the identifier.
HEADER_LINES = 20
HEADER_BYTES = 8192
# How many files a finding lists.
LIST_LIMIT = 20
MARKER = "SPDX-License-Identifier:"


def has_identifier(path):
    """Returns True or False, or None when the file cannot be read."""
    try:
        with open(path, "rb") as handle:
            head = handle.read(HEADER_BYTES)
    except OSError:
        return None
    lines = head.decode("utf-8", errors="replace").splitlines()[:HEADER_LINES]
    return any(MARKER in line for line in lines)


def analyze(request):
    root = request.get("repository", {}).get("root")
    if not root:
        return {
            "api": 1,
            "notes": ["Skipped: this plugin needs the repository-files permission."],
        }

    checked = 0
    missing = []
    for entry in request.get("files", []):
        if entry.get("category") != "source":
            continue
        if entry.get("generated") or entry.get("vendored") or entry.get("binary"):
            continue
        relative = entry["path"]
        result = has_identifier(os.path.join(root, *relative.split("/")))
        if result is None:
            continue
        checked += 1
        if not result:
            missing.append(relative)

    response = {"api": 1, "findings": [], "metrics": [], "notes": []}
    if checked:
        response["metrics"].append(
            {
                "id": "spdx-coverage",
                "label": "Source files with an SPDX identifier",
                "value": round((checked - len(missing)) / checked, 4),
                "unit": "ratio",
                "definition": "Share of source files whose first 20 lines contain "
                "'SPDX-License-Identifier:'.",
            }
        )
    if missing:
        shown = missing[:LIST_LIMIT]
        more = len(missing) - len(shown)
        if len(missing) == 1:
            summary = f"The file without an identifier is {missing[0]}."
        else:
            summary = "Files without an identifier include " + ", ".join(shown[:3])
            summary += f" and {len(missing) - 3} more." if len(missing) > 3 else "."
        verb = "has" if len(missing) == 1 else "have"
        response["findings"].append(
            {
                "rule": "missing-spdx-identifier",
                "severity": "info",
                "confidence": "high",
                "title": f"{len(missing)} of {checked} source files {verb} no SPDX license identifier",
                "summary": summary,
                "rationale": "An SPDX identifier states a file's license in a machine-readable "
                "way, which helps license scanners and anyone who copies individual files.",
                "paths": shown,
                "evidence": [{"kind": "file", "path": path, "line": 1} for path in shown[:5]],
                "nextSteps": [
                    "If the project uses per-file identifiers, add a comment such as "
                    "'SPDX-License-Identifier: Apache-2.0' near the top of each file."
                ],
                "limitations": [
                    f"Only the first {HEADER_LINES} lines of each file are checked.",
                    "Many projects rely on a single LICENSE file instead of per-file identifiers.",
                ]
                + ([f"{more} more files are not listed."] if more > 0 else []),
            }
        )
    response["notes"].append(f"Checked {checked} source files.")
    return response


def main():
    try:
        request = json.load(sys.stdin)
    except ValueError as error:
        print(f"invalid request: {error}", file=sys.stderr)
        return 1
    json.dump(analyze(request), sys.stdout)
    return 0


if __name__ == "__main__":
    sys.exit(main())
