"""Collect pinned upstream source texts omitted from published crate archives."""

import hashlib
import json
import re
import urllib.request
from pathlib import Path


REPOSITORY = re.compile(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+\Z")
SHA1 = re.compile(r"[0-9a-f]{40}\Z")
FILENAME = re.compile(r"[A-Za-z0-9][A-Za-z0-9._-]*\Z")
MAX_TEXT = 1024 * 1024


def git_blob_sha1(data: bytes) -> str:
    return hashlib.sha1(f"blob {len(data)}\0".encode() + data).hexdigest()


def fetch_raw(url: str) -> bytes:
    request = urllib.request.Request(url, headers={"User-Agent": "PWDS-notice-source-audit"})
    with urllib.request.urlopen(request, timeout=20) as response:
        data = response.read(MAX_TEXT + 1)
    if len(data) > MAX_TEXT:
        raise ValueError(f"oversize upstream notice: {url}")
    return data


def collect(plan: dict, output: Path, fetcher=fetch_raw) -> dict:
    # A fresh private tree prevents preplanted linked output components.
    output.mkdir(exist_ok=False)
    files = []
    covered = set()
    for source in plan["sources"]:
        repository, commit = source["repository"], source["commit"]
        if (not REPOSITORY.fullmatch(repository) or
                any(part in {".", ".."} or part.endswith(".") for part in repository.split("/"))):
            raise ValueError(f"unsafe repository: {repository}")
        if not SHA1.fullmatch(commit):
            raise ValueError(f"invalid source commit: {commit}")
        if source["source_relation"] not in {"exact-vcs", "official-release-tag-proxy"}:
            raise ValueError("unknown source relation")
        for package in source["covers"]:
            if package in covered:
                raise ValueError(f"duplicate package coverage: {package}")
            covered.add(package)
        for name, expected_blob in source["files"].items():
            if not FILENAME.fullmatch(name) or name in {".", ".."}:
                raise ValueError(f"unsafe notice path: {name}")
            if not SHA1.fullmatch(expected_blob):
                raise ValueError(f"invalid Git blob: {expected_blob}")
            url = f"https://raw.githubusercontent.com/{repository}/{commit}/{name}"
            data = fetcher(url)
            if len(data) > MAX_TEXT:
                raise ValueError(f"oversize upstream notice: {url}")
            data.decode("utf-8")
            if git_blob_sha1(data) != expected_blob:
                raise ValueError(f"Git blob mismatch: {url}")
            relative = Path("sources") / repository / commit / name
            destination = output / relative
            destination.parent.mkdir(parents=True, exist_ok=True)
            if destination.exists() and destination.read_bytes() != data:
                raise ValueError(f"conflicting upstream notice: {destination}")
            destination.write_bytes(data)
            files.append({"repository": repository, "commit": commit, "source_relation": source["source_relation"],
                          "source_vcs_commit": source.get("source_vcs_commit"),
                          "release_tag": source.get("release_tag"),
                          "name": name, "git_blob_sha1": expected_blob, "sha256": hashlib.sha256(data).hexdigest(),
                          "bytes": len(data), "collected_path": relative.as_posix(), "covers": source["covers"]})
    unresolved = plan["unresolved"]
    if any(item["package"] in covered for item in unresolved):
        raise ValueError("covered package also marked unresolved")
    report = {"status": "DEVELOPMENT PREVIEW; upstream source evidence only; legal review incomplete",
              "covered_package_count": len(covered), "covered_packages": sorted(covered),
              "unresolved": unresolved, "files": files}
    (output / "coverage.json").write_text(json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    return report


if __name__ == "__main__":
    import argparse

    parser = argparse.ArgumentParser()
    parser.add_argument("--plan", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    result = collect(json.loads(args.plan.read_text(encoding="utf-8")), args.output)
    print(json.dumps({"covered_package_count": result["covered_package_count"],
                      "unresolved": result["unresolved"], "file_count": len(result["files"])}, indent=2))
