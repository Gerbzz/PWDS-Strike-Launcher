import hashlib
import tempfile
import unittest
from pathlib import Path

from upstream_notice_sources import collect, git_blob_sha1


class UpstreamNoticeTests(unittest.TestCase):
    def fixture(self, blob: bytes):
        return {"sources": [{
            "repository": "Example/project",
            "commit": "a" * 40,
            "source_relation": "exact-vcs",
            "covers": ["launcher:example 1.0.0"],
            "files": {"LICENSE-MIT": git_blob_sha1(blob)},
        }], "unresolved": []}

    def test_collects_only_blob_verified_bytes(self):
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary) / "collection"
            data = b"upstream notice\n"
            result = collect(self.fixture(data), output, lambda url: data)
            file = result["files"][0]
            self.assertEqual(file["sha256"], hashlib.sha256(data).hexdigest())
            self.assertEqual((output / file["collected_path"]).read_bytes(), data)
            self.assertEqual(result["covered_package_count"], 1)

    def test_rejects_wrong_blob_and_existing_output(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            output = root / "collection"
            with self.assertRaisesRegex(ValueError, "Git blob"):
                collect(self.fixture(b"right"), output, lambda url: b"wrong")
            with self.assertRaisesRegex(FileExistsError, "collection"):
                collect(self.fixture(b"right"), output, lambda url: b"right")

    def test_rejects_unsafe_repository_or_notice_path(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            fixture = self.fixture(b"text")
            fixture["sources"][0]["repository"] = "../escape"
            with self.assertRaisesRegex(ValueError, "repository"):
                collect(fixture, root / "first", lambda url: b"text")
            fixture = self.fixture(b"text")
            fixture["sources"][0]["files"] = {"../LICENSE": git_blob_sha1(b"text")}
            with self.assertRaisesRegex(ValueError, "path"):
                collect(fixture, root / "second", lambda url: b"text")


if __name__ == "__main__":
    unittest.main()
