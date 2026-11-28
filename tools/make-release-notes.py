#!/usr/bin/env python3
"""Write the GitHub release notes for a version from CHANGELOG.md.

The notes are the version's CHANGELOG section, followed by an install table
for the installers actually present in the release folder and the commands
to verify a download.

Usage:
    make-release-notes.py --version v0.2.0 --repo OWNER/REPO \\
        [--changelog CHANGELOG.md] [--dir artifacts] > notes.md
"""
from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

VERSION = re.compile(r"^v?(\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?)$")

# (filename pattern, system, install command template)
INSTALLERS = [
    (re.compile(r"\.deb$"), "Ubuntu, Debian, Linux Mint", "sudo apt install ./{name}"),
    (re.compile(r"\.rpm$"), "Fedora, openSUSE", "sudo dnf install ./{name}"),
    (re.compile(r"\.AppImage$"), "Any Linux", "chmod +x {name} && ./{name}"),
    (re.compile(r"-setup\.exe$"), "Windows 10 / 11", "run the installer"),
]


def changelog_section(text: str, version: str) -> str:
    """The body of `## <version> …` up to the next `## ` heading."""
    lines = text.splitlines()
    start = None
    for i, line in enumerate(lines):
        if re.match(rf"^## \[?{re.escape(version)}\]?(\s|$)", line):
            start = i + 1
            break
    if start is None:
        raise ValueError(f"CHANGELOG has no section for {version}")
    end = len(lines)
    for j in range(start, len(lines)):
        if lines[j].startswith("## "):
            end = j
            break
    body = "\n".join(lines[start:end]).strip()
    if not body:
        raise ValueError(f"the CHANGELOG section for {version} is empty")
    return body


def install_rows(folder: Path | None) -> list[str]:
    if folder is None or not folder.is_dir():
        return []
    names = sorted(p.name for p in folder.iterdir() if p.is_file())
    rows = []
    for pattern, system, command in INSTALLERS:
        for name in names:
            if pattern.search(name):
                rows.append(f"| {system} | `{name}` | `{command.format(name=name)}` |")
    return rows


def notes(version: str, repo: str, changelog: str, folder: Path | None) -> str:
    if not re.fullmatch(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+", repo):
        raise ValueError(f"not an OWNER/REPO name: {repo!r}")
    parts = [changelog_section(changelog, version), "## Install"]
    rows = install_rows(folder)
    if rows:
        parts.append("| System | File | Install |\n|---|---|---|\n" + "\n".join(rows))
    parts.append(
        "Already using ToolOnize? It updates itself: Settings › Updates, or the "
        "update entry in the sidebar. Installers are not code-signed, so Windows "
        "SmartScreen may show a warning."
    )
    parts.append(
        "## Verify your download\n\n"
        "```bash\n"
        "sha256sum -c SHA256SUMS.txt --ignore-missing\n"
        f"gh attestation verify <file> --repo {repo}\n"
        "```"
    )
    parts.append(
        "---\n\nToolOnize is an open-source product by [CRMonize.com](https://crmonize.com), "
        "licensed MIT OR Apache-2.0."
    )
    return "\n\n".join(parts) + "\n"


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--version", required=True)
    ap.add_argument("--repo", required=True)
    ap.add_argument("--changelog", default="CHANGELOG.md")
    ap.add_argument("--dir", default=None)
    args = ap.parse_args(argv)
    m = VERSION.match(args.version)
    if not m:
        print(f"not a version: {args.version}", file=sys.stderr)
        return 2
    try:
        text = Path(args.changelog).read_text(encoding="utf-8")
        out = notes(m.group(1), args.repo, text, Path(args.dir) if args.dir else None)
    except (OSError, ValueError) as e:
        print(f"error: {e}", file=sys.stderr)
        return 1
    sys.stdout.write(out)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
