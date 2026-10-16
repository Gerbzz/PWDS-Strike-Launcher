import hashlib
import json
import subprocess
import tempfile
import unittest
from pathlib import Path

from dependency_inventory import inventory
from package_preview import verify_commit


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


if __name__ == "__main__":
    unittest.main()
