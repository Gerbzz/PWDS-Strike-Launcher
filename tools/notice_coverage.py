"""Collect source notice candidates for the pinned Windows preview graphs.

This is source evidence, not a legal clearance or a linked-binary inventory.
Registry bytes come from .crate archives whose SHA-256 matches Cargo.lock.
"""

import argparse
import hashlib
import io
import json
import re
import stat
import tarfile
import tomllib
from pathlib import Path, PurePosixPath


LEGAL_NAME = re.compile(r"^(?:LICEN[CS]E|COPYING|NOTICE|OFL|UNLICENSE|PATENTS)(?:[._-].*)?$", re.I)
MAX_TEXT = 1024 * 1024
MAX_FILES_PER_PACKAGE = 64
WINDOWS_DEVICES = {"CON", "PRN", "AUX", "NUL", *(f"COM{i}" for i in range(1, 10)),
                   *(f"LPT{i}" for i in range(1, 10))}


def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def linked(status) -> bool:
    return (stat.S_ISLNK(status.st_mode) or
            bool(getattr(status, "st_file_attributes", 0) &
                 getattr(stat, "FILE_ATTRIBUTE_REPARSE_POINT", 0)))


def safe_component(part: str) -> None:
    if (part in {"", ".", ".."} or any(char in part for char in "\\/:") or
            part.rstrip(" .") != part or part.split(".", 1)[0].upper() in WINDOWS_DEVICES):
        raise ValueError(f"unsafe notice path component: {part}")


def checked_source_file(workspace_root: Path, item: Path) -> None:
    """Validate every checkout component before opening a source file."""
    root = workspace_root.absolute()
    path = item.absolute()
    try:
        relative = path.relative_to(root)
    except ValueError as exc:
        raise ValueError(f"source outside workspace: {item}") from exc
    if not relative.parts:
        raise ValueError(f"source is workspace directory: {item}")
    if linked(root.lstat()) or not root.is_dir():
        raise ValueError(f"unsafe workspace root: {root}")
    resolved_root = root.resolve(strict=True)
    current = root
    for index, part in enumerate(relative.parts):
        safe_component(part)
        current = current / part
        status = current.lstat()
        if linked(status):
            raise ValueError(f"symlink or reparse point in workspace source: {current}")
        if index < len(relative.parts) - 1 and not stat.S_ISDIR(status.st_mode):
            raise ValueError(f"non-directory workspace source parent: {current}")
        if index == len(relative.parts) - 1 and not stat.S_ISREG(status.st_mode):
            raise ValueError(f"non-regular workspace source: {current}")
        try:
            current.resolve(strict=True).relative_to(resolved_root)
        except ValueError as exc:
            raise ValueError(f"source outside workspace: {current}") from exc


def checked_output_file(output: Path, parts: tuple[str, ...], data: bytes) -> Path:
    """Write inside an output tree only when its components are real directories."""
    for part in parts:
        safe_component(part)
    output.mkdir(parents=True, exist_ok=True)
    status = output.lstat()
    if linked(status) or not stat.S_ISDIR(status.st_mode):
        raise ValueError(f"unsafe output root: {output}")
    resolved_root = output.resolve(strict=True)
    current = output
    for part in parts[:-1]:
        current = current / part
        try:
            status = current.lstat()
        except FileNotFoundError:
            current.mkdir()
            status = current.lstat()
        if linked(status) or not stat.S_ISDIR(status.st_mode):
            raise ValueError(f"symlink or reparse point in output parent: {current}")
        try:
            current.resolve(strict=True).relative_to(resolved_root)
        except ValueError as exc:
            raise ValueError(f"output parent outside collection: {current}") from exc
    destination = current / parts[-1]
    try:
        status = destination.lstat()
    except FileNotFoundError:
        with destination.open("xb") as stream:
            stream.write(data)
    else:
        if linked(status) or not stat.S_ISREG(status.st_mode):
            raise ValueError(f"unsafe existing output file: {destination}")
        try:
            destination.resolve(strict=True).relative_to(resolved_root)
        except ValueError as exc:
            raise ValueError(f"output file outside collection: {destination}") from exc
        if destination.read_bytes() != data:
            raise ValueError(f"conflicting notice bytes: {destination}")
    return destination


def selected_packages(metadata: dict, root_name: str) -> list[dict]:
    packages = {item["id"]: item for item in metadata["packages"]}
    roots = [item["id"] for item in packages.values() if item["name"] == root_name]
    if len(roots) != 1:
        raise ValueError(f"expected one {root_name} root, found {len(roots)}")
    nodes = {item["id"]: item for item in metadata["resolve"]["nodes"]}
    pending, reached = roots, set()
    while pending:
        package_id = pending.pop()
        if package_id in reached:
            continue
        if package_id not in packages or package_id not in nodes:
            raise ValueError(f"incomplete Cargo graph: {package_id}")
        reached.add(package_id)
        pending.extend(dep["pkg"] for dep in nodes[package_id]["deps"]
                       if any(kind.get("kind") != "dev" for kind in dep["dep_kinds"]))
    return sorted((packages[item] for item in reached),
                  key=lambda item: (item["name"], item["version"], item["id"]))


def candidate(relative: PurePosixPath, declared: str | None) -> bool:
    if declared and relative.as_posix() == declared:
        return True
    if LEGAL_NAME.match(relative.name):
        return True
    return any(part.lower() in {"licenses", "licences"} for part in relative.parts[:-1])


def safe_relative(name: str, archive_root: str) -> PurePosixPath | None:
    path = PurePosixPath(name)
    if path.is_absolute() or any(part in {"", ".", ".."} or "\\" in part or ":" in part
                                 for part in path.parts):
        raise ValueError(f"unsafe crate member: {name}")
    if not path.parts or path.parts[0] != archive_root:
        raise ValueError(f"unexpected crate member root: {name}")
    return PurePosixPath(*path.parts[1:]) if len(path.parts) > 1 else None


def registry_texts(package: dict, checksums: dict) -> tuple[str, list[tuple[str, bytes]]]:
    name, version = package["name"], package["version"]
    source = package["source"]
    if not source.startswith("registry+"):
        raise ValueError(f"unsupported registry source: {source}")
    key = (name, version, source)
    if key not in checksums:
        raise ValueError(f"missing lock checksum: {name} {version}")
    source_root = Path(package["manifest_path"]).parent
    archive = (source_root.parent.parent.parent / "cache" /
               source_root.parent.name / f"{name}-{version}.crate")
    archive_bytes = archive.read_bytes()
    archive_hash = digest(archive_bytes)
    if archive_hash != checksums[key]:
        raise ValueError(f"crate checksum mismatch: {name} {version}")
    declared = None
    if package.get("license_file"):
        declared_path = Path(package["license_file"])
        declared = declared_path.relative_to(source_root).as_posix()
    texts = []
    with tarfile.open(fileobj=io.BytesIO(archive_bytes), mode="r:gz") as crate:
        for member in crate.getmembers():
            relative = safe_relative(member.name, f"{name}-{version}")
            if relative is None or not candidate(relative, declared):
                continue
            if not member.isfile():
                raise ValueError(f"notice candidate is not a regular file: {member.name}")
            if member.size > MAX_TEXT:
                raise ValueError(f"oversize notice candidate: {member.name}")
            if len(texts) >= MAX_FILES_PER_PACKAGE:
                raise ValueError(f"too many notice candidates: {name} {version}")
            data = crate.extractfile(member).read()
            if len(data) != member.size:
                raise ValueError(f"short notice read: {member.name}")
            try:
                data.decode("utf-8")
            except UnicodeDecodeError as exc:
                raise ValueError(f"non UTF-8 notice candidate: {member.name}") from exc
            texts.append((relative.as_posix(), data))
    return archive_hash, sorted(texts)


def workspace_texts(package: dict, workspace_root: Path) -> list[tuple[str, bytes]]:
    manifest = Path(package["manifest_path"])
    checked_source_file(workspace_root, manifest)
    files = [item for item in workspace_root.iterdir()
             if candidate(PurePosixPath(item.name), None)]
    declared = package.get("license_file")
    if declared:
        item = Path(declared)
        item.relative_to(workspace_root)
        files.append(item)
    texts = []
    for item in sorted(set(files)):
        checked_source_file(workspace_root, item)
        data = item.read_bytes()
        if len(data) > MAX_TEXT:
            raise ValueError(f"oversize workspace notice: {item}")
        data.decode("utf-8")
        texts.append((item.relative_to(workspace_root).as_posix(), data))
    return texts


def collect(metadata: dict, root_name: str, lockfile: Path, output: Path) -> dict:
    lock_bytes = lockfile.read_bytes()
    checksums = {(item["name"], item["version"], item["source"]): item["checksum"]
                 for item in tomllib.loads(lock_bytes.decode("utf-8"))["package"]
                 if item.get("source", "").startswith("registry+")}
    workspace_root = Path(metadata["workspace_root"])
    entries = []
    for package in selected_packages(metadata, root_name):
        source = package.get("source")
        kind = "workspace" if source is None else "registry"
        if kind == "workspace":
            texts = workspace_texts(package, workspace_root)
            archive_hash = None
        else:
            archive_hash, texts = registry_texts(package, checksums)
        files = []
        for relative, data in texts:
            parts = PurePosixPath(relative).parts
            prefix = ("texts", kind, package["name"], package["version"])
            if kind == "workspace":
                prefix = ("texts", kind, root_name)
            destination = checked_output_file(output, prefix + parts, data)
            files.append({"source_path": relative,
                          "collected_path": destination.relative_to(output).as_posix(),
                          "bytes": len(data), "sha256": digest(data)})
        entries.append({"name": package["name"], "version": package["version"],
                        "source_kind": kind, "license_declaration": package.get("license"),
                        "crate_sha256": archive_hash, "status": "source_text_collected" if files else "metadata_only",
                        "files": files})
    return {"root": root_name, "lock_sha256": digest(lock_bytes),
            "package_count": len(entries),
            "source_text_count": sum(item["status"] == "source_text_collected" for item in entries),
            "metadata_only": [f"{item['name']} {item['version']}" for item in entries
                              if item["status"] == "metadata_only"],
            "packages": entries}


def main() -> None:
    parser = argparse.ArgumentParser()
    for prefix in ("launcher", "app"):
        parser.add_argument(f"--{prefix}-metadata", type=Path, required=True)
        parser.add_argument(f"--{prefix}-lock", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--launcher-commit", required=True)
    parser.add_argument("--app-commit", required=True)
    args = parser.parse_args()
    report = {"status": "DEVELOPMENT PREVIEW; notice review incomplete",
              "scope": "Windows filtered normal/build Cargo graph; may include crates not linked into binaries",
              "target": "x86_64-pc-windows-msvc",
              "launcher_commit": args.launcher_commit, "app_commit": args.app_commit,
              "projects": [
                  collect(json.loads(args.launcher_metadata.read_text(encoding="utf-8")),
                          "pwds-strike-launcher", args.launcher_lock, args.output),
                  collect(json.loads(args.app_metadata.read_text(encoding="utf-8")),
                          "gridcraft", args.app_lock, args.output),
              ]}
    checked_output_file(args.output, ("coverage.json",),
                        (json.dumps(report, indent=2, sort_keys=True) + "\n").encode("utf-8"))
    print(json.dumps({item["root"]: {"packages": item["package_count"],
                                     "with_source_text": item["source_text_count"],
                                     "metadata_only": item["metadata_only"]}
                      for item in report["projects"]}, indent=2))


if __name__ == "__main__":
    main()
