import importlib.util
import tempfile
import unittest
from pathlib import Path

spec = importlib.util.spec_from_file_location(
    "make_release_notes",
    Path(__file__).resolve().parents[1] / "make-release-notes.py",
)
assert spec and spec.loader
mrn = importlib.util.module_from_spec(spec)
spec.loader.exec_module(mrn)  # type: ignore[union-attr]

CHANGELOG = """# Changelog

## Unreleased

- not yet

## 0.3.0 — 2027-01-10

### Terminals

- New thing.

## 0.2.0 — 2026-11-28

First release.
"""


class TestReleaseNotes(unittest.TestCase):
    def test_section_stops_at_the_next_version(self):
        body = mrn.changelog_section(CHANGELOG, "0.3.0")
        self.assertIn("### Terminals", body)
        self.assertIn("- New thing.", body)
        self.assertNotIn("First release", body)
        self.assertNotIn("not yet", body)
        self.assertEqual(mrn.changelog_section(CHANGELOG, "0.2.0"), "First release.")

    def test_missing_or_empty_section_is_an_error(self):
        with self.assertRaises(ValueError):
            mrn.changelog_section(CHANGELOG, "9.9.9")
        with self.assertRaises(ValueError):
            mrn.changelog_section("## 1.0.0\n\n## 0.9.0\nx\n", "1.0.0")
        # "0.2.0" must not match "0.2.01" or "10.2.0".
        with self.assertRaises(ValueError):
            mrn.changelog_section("## 0.2.01\nx\n## 10.2.0\ny\n", "0.2.0")

    def test_install_table_lists_only_present_installers(self):
        d = Path(tempfile.mkdtemp())
        for name in ["ToolOnize_0.3.0_amd64.deb", "ToolOnize_0.3.0_amd64.deb.sig",
                     "ToolOnize_0.3.0_x64-setup.exe", "SHA256SUMS.txt", "latest.json"]:
            (d / name).write_text("x")
        out = mrn.notes("0.3.0", "example/toolonize", CHANGELOG, d)
        self.assertIn("| Ubuntu, Debian, Linux Mint | `ToolOnize_0.3.0_amd64.deb` | "
                      "`sudo apt install ./ToolOnize_0.3.0_amd64.deb` |", out)
        self.assertIn("`ToolOnize_0.3.0_x64-setup.exe`", out)
        self.assertNotIn(".sig", out)
        self.assertNotIn("AppImage", out)
        self.assertNotIn(".rpm", out)
        self.assertIn("gh attestation verify <file> --repo example/toolonize", out)
        self.assertIn("CRMOnize", out)
        self.assertIn("| Windows 10 / 11 | `ToolOnize_0.3.0_x64-setup.exe` | Run the installer |", out)

    def test_repo_name_is_validated(self):
        with self.assertRaises(ValueError):
            mrn.notes("0.3.0", "not a repo; rm -rf /", CHANGELOG, None)

    def test_cli_rejects_a_bad_version(self):
        self.assertEqual(mrn.main(["--version", "latest", "--repo", "a/b"]), 2)


if __name__ == "__main__":
    unittest.main()
