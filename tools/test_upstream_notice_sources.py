import hashlib
import json
import tempfile
import unittest
from pathlib import Path

from upstream_notice_sources import collect, git_blob_sha1


class UpstreamNoticeTests(unittest.TestCase):
    def test_curated_plan_has_complete_package_and_git_id_coverage(self):
        plan = json.loads((Path(__file__).parent / "upstream_notice_plan.json").read_text(encoding="utf-8"))
        covered = [package for source in plan["sources"] for package in source["covers"]]
        self.assertEqual(len(covered), 40)
        self.assertEqual(len(set(covered)), 40)
        self.assertEqual(len(plan["unresolved"]), 2)
        for source in plan["sources"]:
            self.assertEqual(len(source["commit"]), 40)
            for expected_blob in source["files"].values():
                self.assertEqual(len(expected_blob), 40)

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
