import hashlib
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from dependency_inventory import git_blob_sha256, inventory
from font_notices import collect, verified_files
from package_preview import verify_commit, verify_tracked_checkout_clean


class PreviewAuditTests(unittest.TestCase):
    def test_inventory_follows_normal_and_build_edges_but_not_dev_edges(self):
        metadata = {
            "packages": [
                {"id": "root", "name": "app", "version": "1", "source": None,
                 "license": "MIT", "license_file": None},
                {"id": "normal", "name": "normal", "version": "2", "source": "registry+crates.io",
                 "license": "Apache-2.0", "license_file": None},
                {"id": "build", "name": "build", "version": "3", "source": "registry+crates.io",
                 "license": None, "license_file": "/cache/LICENSE"},
                {"id": "dev", "name": "dev", "version": "4", "source": "registry+crates.io",
                 "license": None, "license_file": None},
            ],
            "resolve": {"nodes": [
                {"id": "root", "deps": [
                    {"pkg": "normal", "dep_kinds": [{"kind": None}]},
                    {"pkg": "build", "dep_kinds": [{"kind": "build"}]},
                    {"pkg": "dev", "dep_kinds": [{"kind": "dev"}]},
                ]},
                {"id": "normal", "deps": []},
                {"id": "build", "deps": []},
                {"id": "dev", "deps": []},
            ]},
        }
        with tempfile.TemporaryDirectory() as temporary:
            lockfile = Path(temporary) / "Cargo.lock"
            lockfile.write_text("pinned", encoding="utf-8")
            result = inventory(metadata, "app", lockfile)
        self.assertEqual([item["name"] for item in result["packages"]],
                         ["app", "build", "normal"])
        self.assertEqual(result["missing_license_declarations"], [])
        self.assertEqual(result["lock_sha256"], hashlib.sha256(b"pinned").hexdigest())

    def test_packager_rejects_forged_commit(self):
        checkout = Path(__file__).resolve().parent.parent
        actual = subprocess.check_output(
            ["git", "-C", str(checkout), "rev-parse", "HEAD"], text=True
        ).strip()
        verify_commit(checkout, actual)
        with self.assertRaisesRegex(ValueError, "commit mismatch"):
            verify_commit(checkout, "0" * 40 if actual != "0" * 40 else "1" * 40)

    def test_font_notice_copy_checks_exact_bytes(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            fonts = root / "crate" / "fonts"
            fonts.mkdir(parents=True)
            (fonts / "OFL.txt").write_bytes(b"test font notice")
            expected = {"OFL.txt": hashlib.sha256(b"test font notice").hexdigest()}
            metadata = {"packages": [{
                "name": "epaint_default_fonts", "version": "0.36.2",
                "source": "registry+https://example.test", "license": "OFL-1.1",
                "manifest_path": str(root / "crate" / "Cargo.toml"),
            }]}
            destination = root / "collected"
            result = collect(metadata, "0.36.2", destination, expected)
            self.assertEqual(result["files"]["OFL.txt"], expected["OFL.txt"])
            self.assertIn("FONT-ATTRIBUTION.txt", result["files"])
            (destination / "OFL.txt").write_bytes(b"changed")
            with self.assertRaisesRegex(ValueError, "font notice bytes differ"):
                verified_files(destination, result["files"])

    def test_packager_rejects_modified_tracked_checkout(self):
        with patch("package_preview.subprocess.check_output", return_value=""):
            verify_tracked_checkout_clean(Path("."))
        with patch("package_preview.subprocess.check_output", return_value=" M Cargo.lock"):
            with self.assertRaisesRegex(ValueError, "tracked checkout differs"):
                verify_tracked_checkout_clean(Path("."))

    def test_lock_git_hash_uses_committed_bytes_across_line_endings(self):
        with tempfile.TemporaryDirectory() as temporary:
            lockfile = Path(temporary) / "Cargo.lock"
            lockfile.write_bytes(b"line\r\n")
            with patch("dependency_inventory.subprocess.check_output", return_value=b"line\n"):
                committed = git_blob_sha256(lockfile)
            self.assertEqual(committed, hashlib.sha256(b"line\n").hexdigest())
            self.assertNotEqual(committed, hashlib.sha256(lockfile.read_bytes()).hexdigest())


if __name__ == "__main__":
    unittest.main()
