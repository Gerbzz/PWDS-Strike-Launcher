import io
import tarfile
import tempfile
import unittest
from pathlib import Path

from notice_coverage import collect, digest, safe_relative


class NoticeCoverageTests(unittest.TestCase):
    def fixture(self, root: Path, members: dict[str, bytes], *, checksum: str | None = None):
        registry = root / "registry"
        source = registry / "src" / "test-index" / "example-1.0.0"
        source.mkdir(parents=True)
        manifest = source / "Cargo.toml"
        manifest.write_text("[package]\nname = 'example'\nversion = '1.0.0'\n")
        archive = registry / "cache" / "test-index" / "example-1.0.0.crate"
        archive.parent.mkdir(parents=True)
        with tarfile.open(archive, "w:gz") as crate:
            for name, data in members.items():
                info = tarfile.TarInfo(name)
                info.size = len(data)
                crate.addfile(info, io.BytesIO(data))
        source_id = "registry+https://github.com/rust-lang/crates.io-index"
        lock = root / "Cargo.lock"
        lock.write_text("[[package]]\nname = 'example'\nversion = '1.0.0'\n"
                        f"source = '{source_id}'\nchecksum = '{checksum or digest(archive.read_bytes())}'\n")
        package_id = "example 1.0.0 (registry)"
        metadata = {"workspace_root": str(root),
                    "packages": [{"id": package_id, "name": "example", "version": "1.0.0",
                                  "source": source_id, "manifest_path": str(manifest),
                                  "license": "MIT", "license_file": None}],
                    "resolve": {"nodes": [{"id": package_id, "deps": []}]}}
        return metadata, lock

    def test_collects_only_source_notice_and_records_hash(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            metadata, lock = self.fixture(root, {"example-1.0.0/LICENSE-MIT": b"real notice\n",
                                                  "example-1.0.0/src/lib.rs": b"code"})
            output = root / "output"
            result = collect(metadata, "example", lock, output)
            entry = result["packages"][0]
            self.assertEqual(entry["status"], "source_text_collected")
            self.assertEqual(entry["files"][0]["sha256"], digest(b"real notice\n"))
            self.assertEqual((output / entry["files"][0]["collected_path"]).read_bytes(), b"real notice\n")
            self.assertEqual(len(entry["files"]), 1)

    def test_missing_file_stays_metadata_only(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            metadata, lock = self.fixture(root, {"example-1.0.0/src/lib.rs": b"code"})
            result = collect(metadata, "example", lock, root / "output")
            self.assertEqual(result["metadata_only"], ["example 1.0.0"])

    def test_rejects_corrupted_archive(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            metadata, lock = self.fixture(root, {"example-1.0.0/LICENSE": b"text"},
                                          checksum="0" * 64)
            with self.assertRaisesRegex(ValueError, "checksum mismatch"):
                collect(metadata, "example", lock, root / "output")

    def test_rejects_archive_traversal_and_windows_separators(self):
        for member in ("example-1.0.0/../../LICENSE", "example-1.0.0/..\\..\\LICENSE"):
            with self.subTest(member=member), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                metadata, lock = self.fixture(root, {member: b"text"})
                with self.assertRaisesRegex(ValueError, "unsafe crate member"):
                    collect(metadata, "example", lock, root / "output")
        self.assertIsNone(safe_relative("example-1.0.0", "example-1.0.0"))


if __name__ == "__main__":
    unittest.main()
