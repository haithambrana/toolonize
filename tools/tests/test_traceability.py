import importlib.util
import unittest
from pathlib import Path

spec = importlib.util.spec_from_file_location(
    "check_doc_traceability",
    Path(__file__).resolve().parents[1] / "check-doc-traceability.py",
)
assert spec and spec.loader
cdt = importlib.util.module_from_spec(spec)
spec.loader.exec_module(cdt)  # type: ignore[union-attr]

class TestRangeExpansion(unittest.TestCase):
    def test_simple_range(self):
        ids = cdt.expand_all_tokens("T-PTY-001..013")
        self.assertEqual(len(ids), 13)
        self.assertIn("T-PTY-001", ids)
        self.assertIn("T-PTY-013", ids)

    def test_mixed_range_and_single(self):
        ids = cdt.expand_all_tokens("T-PTY-001..003 and T-UI-001")
        self.assertIn("T-PTY-001", ids)
        self.assertIn("T-PTY-003", ids)
        self.assertIn("T-UI-001", ids)
        self.assertEqual(len(ids), 4)

    def test_complex_prefix_range(self):
        ids = cdt.expand_all_tokens("T-DISC-LNX-DES-001..004")
        self.assertEqual(len(ids), 4)
        self.assertIn("T-DISC-LNX-DES-001", ids)
        self.assertIn("T-DISC-LNX-DES-004", ids)

    def test_no_false_range(self):
        ids = cdt.expand_all_tokens("FR-001 and FR-002")
        self.assertEqual(len(ids), 0)

    def test_malformed_range_raises(self):
        with self.assertRaises(ValueError):
            cdt.expand_all_tokens("T-PTY-013..001")

if __name__ == "__main__":
    unittest.main()
