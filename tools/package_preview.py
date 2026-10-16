"""Deterministic ZIP wrapper and provenance for a pinned development preview.

The Rust executables themselves are build outputs, not asserted bit-for-bit
reproducible across compiler or runner images. This records those inputs.
"""

import argparse
import hashlib
import json
import os
import re
import shutil
import subprocess
import zipfile
from pathlib import Path

from dependency_inventory import git_blob_sha256
from font_notices import NOTICE_SHA256, VERSIONS, verified_files
from pe_imports import imports


NOTICES = (
    "LICENSE-MIT",
    "LICENSE-APACHE",
    "NOTICE",
    "ATTRIBUTION.md",
    "UPSTREAM_ATTRIBUTION.md",
)


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def verify_commit(checkout: Path, claimed: str) -> None:
    if not re.fullmatch(r"[0-9a-f]{40}", claimed):
        raise ValueError(f"invalid commit ID for {checkout}: {claimed}")
    actual = subprocess.check_output(
        ["git", "-C", str(checkout), "rev-parse", "--verify", "HEAD"], text=True
    ).strip()
    if actual != claimed:
        raise ValueError(f"commit mismatch for {checkout}: expected {claimed}, found {actual}")


def verify_tracked_checkout_clean(checkout: Path) -> None:
    changes = subprocess.check_output(
        ["git", "-C", str(checkout), "status", "--porcelain", "--untracked-files=no"],
        text=True,
    ).strip()
    if changes:
        raise ValueError(f"tracked checkout differs from HEAD: {checkout}: {changes}")


def make_zip(path: Path, contents: dict[str, bytes]) -> None:
    with zipfile.ZipFile(path, "w", compression=zipfile.ZIP_DEFLATED, compresslevel=9) as output:
        for name in sorted(contents):
            info = zipfile.ZipInfo(name, date_time=(1980, 1, 1, 0, 0, 0))
            info.compress_type = zipfile.ZIP_DEFLATED
            info.external_attr = 0o100644 << 16
            output.writestr(info, contents[name], compress_type=zipfile.ZIP_DEFLATED, compresslevel=9)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--app", type=Path, required=True)
    parser.add_argument("--cli", type=Path, required=True)
    parser.add_argument("--ui", type=Path, required=True)
    parser.add_argument("--source", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--launcher-commit", required=True)
    parser.add_argument("--app-commit", required=True)
    parser.add_argument("--dependency-inventory", type=Path, required=True)
    parser.add_argument("--font-notices", type=Path, required=True)
    args = parser.parse_args()
    verify_commit(Path.cwd(), args.launcher_commit)
    verify_commit(args.source, args.app_commit)
    verify_tracked_checkout_clean(Path.cwd())
    verify_tracked_checkout_clean(args.source)
    inventory = json.loads(args.dependency_inventory.read_text(encoding="utf-8"))
    if (inventory.get("target") != "x86_64-pc-windows-msvc" or
            {item["root"]: item["lock_sha256"] for item in inventory["projects"]} != {
                "pwds-strike-launcher": sha256(Path("Cargo.lock")),
                "gridcraft": sha256(args.source / "Cargo.lock"),
            } or
            {item["root"]: item["lock_git_blob_sha256"] for item in inventory["projects"]} != {
                "pwds-strike-launcher": git_blob_sha256(Path("Cargo.lock")),
                "gridcraft": git_blob_sha256(args.source / "Cargo.lock"),
            }):
        raise ValueError("dependency inventory does not match preview lockfiles and target")
    notice_manifest = json.loads((args.font_notices / "manifest.json").read_text(encoding="utf-8"))
    font_contents = {}
    for group, version in VERSIONS.items():
        if (notice_manifest[group]["version"] != version or
                notice_manifest[group]["files"] != NOTICE_SHA256):
            raise ValueError(f"unexpected {group} font notice manifest")
        font_contents[group] = verified_files(args.font_notices / group)
    args.output.mkdir(parents=True, exist_ok=True)
    inventory_artifact = args.output / "dependency-inventory.json"
    if args.dependency_inventory.resolve() != inventory_artifact.resolve():
        shutil.copyfile(args.dependency_inventory, inventory_artifact)
    font_artifact = args.output / "font-notices"
    if args.font_notices.resolve() != font_artifact.resolve():
        shutil.copytree(args.font_notices, font_artifact)

    binaries = {"gridstrike.exe": args.app, "pwds-strike-launcher.exe": args.cli,
                "pwds-strike-launcher-ui.exe": args.ui}
    binary_audit = {}
    for name, source in binaries.items():
        imported = imports(source)
        all_names = [value.lower() for values in imported.values() for value in values]
        if any(value.startswith(("vcruntime", "msvcp", "gstreamer", "avcodec", "avformat"))
               for value in all_names):
            raise RuntimeError(f"{name} has an unbundled non-OS runtime import: {imported}")
        destination = args.output / name
        shutil.copyfile(source, destination)
        binary_audit[name] = {
            "bytes": destination.stat().st_size,
            "sha256": sha256(destination),
            "pe_imports": imported,
        }

    contents = {"gridstrike.exe": args.app.read_bytes()}
    contents.update({name: (args.source / name).read_bytes() for name in NOTICES})
    contents.update({
        f"THIRD_PARTY/epaint_default_fonts-{VERSIONS['gridstrike']}/{name}": data
        for name, data in font_contents["gridstrike"].items()
    })
    packages = {}
    for version, extra in (("0.5.0", {}),
                           ("0.5.1", {"SYNTHETIC-UPDATE.txt":
                            b"Transaction test only. Same GridStrike 0.5.0 executable; not an app update.\n"})):
        label = "-SYNTHETIC-UPDATE" if extra else ""
        name = f"GridStrike-{version}{label}-windows-x86_64-DEVELOPMENT-PREVIEW.zip"
        package = args.output / name
        make_zip(package, contents | extra)
        packages[version] = {"name": name, "bytes": package.stat().st_size, "sha256": sha256(package)}

    provenance = {
        "status": "DEVELOPMENT PREVIEW; no public release",
        "launcher_commit": args.launcher_commit,
        "gridstrike_commit": args.app_commit,
        "dependency_inventory": {
            "name": inventory_artifact.name,
            "bytes": inventory_artifact.stat().st_size,
            "sha256": sha256(inventory_artifact),
        },
        "font_notice_manifest": {
            "name": "font-notices/manifest.json",
            "bytes": (font_artifact / "manifest.json").stat().st_size,
            "sha256": sha256(font_artifact / "manifest.json"),
        },
        "rustc": subprocess.check_output(["rustc", "--version", "--verbose"], text=True),
        "runner_image": os.environ.get("ImageVersion", "unknown"),
        "zip_metadata": "sorted paths; fixed 1980-01-01 timestamps; deflate level 9",
        "binaries": binary_audit,
        "packages": packages,
        "public_release_gates": [
            "GridStrike icon authorship record and transitive dependency notices need review",
            "launcher distribution license needs to be declared",
            "publisher signing policy and supported Windows versions need approval",
            "GUI startup needs a fresh desktop test with a graphics adapter",
        ],
    }
    (args.output / "provenance.json").write_text(
        json.dumps(provenance, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )
    print(json.dumps({"packages": packages, "binaries": binary_audit}, indent=2))


if __name__ == "__main__":
    main()
