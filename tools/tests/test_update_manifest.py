import datetime as dt
import importlib.util
import tempfile
import unittest
from pathlib import Path

spec = importlib.util.spec_from_file_location(
    "make_update_manifest",
    Path(__file__).resolve().parents[1] / "make-update-manifest.py",
)
assert spec and spec.loader
mum = importlib.util.module_from_spec(spec)
spec.loader.exec_module(mum)  # type: ignore[union-attr]

BASE = "https://github.com/example/toolonize/releases/download/v0.2.0"
NOW = dt.datetime(2026, 12, 1, 10, 30, tzinfo=dt.timezone.utc)


class TestManifest(unittest.TestCase):
    def folder(self, files: dict[str, str]) -> Path:
        d = Path(tempfile.mkdtemp())
        for name, text in files.items():
            (d / name).write_text(text)
        return d

    def test_all_platforms_and_fallbacks(self):
        d = self.folder({
            "ToolOnize_0.2.0_amd64.AppImage": "x", "ToolOnize_0.2.0_amd64.AppImage.sig": "SIGAPP\n",
            "ToolOnize_0.2.0_amd64.deb": "x", "ToolOnize_0.2.0_amd64.deb.sig": "SIGDEB",
            "ToolOnize-0.2.0-1.x86_64.rpm": "x", "ToolOnize-0.2.0-1.x86_64.rpm.sig": "SIGRPM",
            "ToolOnize_0.2.0_x64-setup.exe": "x", "ToolOnize_0.2.0_x64-setup.exe.sig": "SIGEXE",
            "SHA256SUMS.txt": "x",
        })
        m = mum.build(d, "v0.2.0", BASE, "notes", NOW)
        self.assertEqual(m["version"], "0.2.0")
        self.assertEqual(m["pub_date"], "2026-12-01T10:30:00Z")
        p = m["platforms"]
        self.assertEqual(sorted(p), [
            "linux-x86_64", "linux-x86_64-appimage", "linux-x86_64-deb", "linux-x86_64-rpm",
            "windows-x86_64", "windows-x86_64-nsis",
        ])
        self.assertEqual(p["linux-x86_64"], p["linux-x86_64-appimage"])
        self.assertEqual(p["linux-x86_64-appimage"]["signature"], "SIGAPP")
        self.assertEqual(p["windows-x86_64"]["url"], BASE + "/ToolOnize_0.2.0_x64-setup.exe")

    def test_refuses_unsigned_bad_tags_and_http(self):
        d = self.folder({"ToolOnize_0.2.0_amd64.AppImage": "x"})
        with self.assertRaisesRegex(ValueError, "no signature"):
            mum.build(d, "v0.2.0", BASE, "", NOW)
        d = self.folder({"a.deb": "x", "a.deb.sig": "two words"})
        with self.assertRaisesRegex(ValueError, "single-line"):
            mum.build(d, "v0.2.0", BASE, "", NOW)
        d = self.folder({"a.deb": "x", "a.deb.sig": "S"})
        with self.assertRaisesRegex(ValueError, "release tag"):
            mum.build(d, "latest", BASE, "", NOW)
        with self.assertRaisesRegex(ValueError, "https"):
            mum.build(d, "v0.2.0", "http://example.org", "", NOW)
        with self.assertRaisesRegex(ValueError, "no signed installers"):
            mum.build(self.folder({"notes.txt": "x"}), "v0.2.0", BASE, "", NOW)

    def test_names_are_url_encoded_and_arm_detected(self):
        d = self.folder({"Tool Onize_aarch64.AppImage": "x", "Tool Onize_aarch64.AppImage.sig": "S"})
        p = mum.build(d, "0.2.1-beta.1", BASE, "", NOW)["platforms"]
        self.assertIn("linux-aarch64", p)
        self.assertTrue(p["linux-aarch64"]["url"].endswith("/Tool%20Onize_aarch64.AppImage"))


if __name__ == "__main__":
    unittest.main()
