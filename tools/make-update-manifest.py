#!/usr/bin/env python3
"""Build the updater manifest (latest.json) for a release (ADR-019).

Reads the release folder produced by the release workflow: each installer
next to its `.sig` signature (created by `tauri build` with the release
signing key). Writes the JSON the in-app updater reads:

    {"version", "notes", "pub_date", "platforms": {"<target>": {"url", "signature"}}}

Targets follow tauri-plugin-updater: `<os>-<arch>-<installer>` plus the
`<os>-<arch>` fallback (AppImage on Linux, the NSIS installer on Windows).
Usage:
    make-update-manifest.py --dir artifacts --tag v0.2.0 \\
        --base-url https://github.com/OWNER/REPO/releases/download/v0.2.0 [--notes TEXT]
"""
from __future__ import annotations

import argparse
import datetime as dt
import json
import re
import sys
import urllib.parse
from pathlib import Path

KINDS = [
    # (filename pattern, os, installer, also the os-arch fallback)
    (re.compile(r"\.AppImage$"), "linux", "appimage", True),
    (re.compile(r"\.deb$"), "linux", "deb", False),
    (re.compile(r"\.rpm$"), "linux", "rpm", False),
    (re.compile(r"-setup\.exe$"), "windows", "nsis", True),
]
VERSION = re.compile(r"^v?(\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?)$")


def arch_of(name: str) -> str:
    lower = name.lower()
    if "aarch64" in lower or "arm64" in lower:
        return "aarch64"
    return "x86_64"


def build(folder: Path, tag: str, base_url: str, notes: str, now: dt.datetime) -> dict:
    m = VERSION.match(tag)
    if not m:
        raise ValueError(f"not a release tag: {tag!r}")
    if not base_url.startswith("https://"):
        raise ValueError("the download base URL must use https")
    platforms: dict[str, dict[str, str]] = {}
    for file in sorted(folder.iterdir()):
        for pattern, os_name, installer, fallback in KINDS:
            if not pattern.search(file.name):
                continue
            sig_file = file.with_name(file.name + ".sig")
            if not sig_file.is_file():
                raise ValueError(f"{file.name} has no signature ({sig_file.name})")
            signature = sig_file.read_text(encoding="utf-8").strip()
            if not signature or any(c.isspace() for c in signature):
                raise ValueError(f"{sig_file.name} is not a single-line signature")
            entry = {
                "url": f"{base_url.rstrip('/')}/{urllib.parse.quote(file.name)}",
                "signature": signature,
            }
            arch = arch_of(file.name)
            key = f"{os_name}-{arch}-{installer}"
            if key in platforms:
                raise ValueError(f"two files for {key}")
            platforms[key] = entry
            if fallback:
                platforms[f"{os_name}-{arch}"] = entry
    if not platforms:
        raise ValueError("no signed installers found")
    return {
        "version": m.group(1),
        "notes": notes,
        "pub_date": now.astimezone(dt.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
        "platforms": platforms,
    }


def main(argv: list[str]) -> int:
    p = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    p.add_argument("--dir", required=True, type=Path)
    p.add_argument("--tag", required=True)
    p.add_argument("--base-url", required=True)
    p.add_argument("--notes", default="See the release page for what changed.")
    a = p.parse_args(argv)
    try:
        manifest = build(a.dir, a.tag, a.base_url, a.notes, dt.datetime.now(dt.timezone.utc))
    except ValueError as e:
        print(f"make-update-manifest: {e}", file=sys.stderr)
        return 1
    json.dump(manifest, sys.stdout, indent=2)
    print()
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
