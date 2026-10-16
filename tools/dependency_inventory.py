"""Record the Windows Cargo dependency graph for preview notice review.

This is an inventory of resolved normal/build edges, not a claim about which
crates' code is linked into the executable or whether notices are complete.
"""

import argparse
import hashlib
import json
from pathlib import Path


def inventory(metadata: dict, root_name: str, lockfile: Path) -> dict:
    packages = {package["id"]: package for package in metadata["packages"]}
    roots = [package for package in packages.values() if package["name"] == root_name]
    if len(roots) != 1:
        raise ValueError(f"expected one package named {root_name}, found {len(roots)}")
    nodes = {node["id"]: node for node in metadata["resolve"]["nodes"]}
    pending = [roots[0]["id"]]
    reached = set()
    while pending:
        package_id = pending.pop()
        if package_id in reached:
            continue
        if package_id not in packages or package_id not in nodes:
            raise ValueError(f"incomplete Cargo metadata graph: {package_id}")
        reached.add(package_id)
        for dependency in nodes[package_id]["deps"]:
            # Cargo's platform-filtered metadata still contains development
            # dependencies. The preview build uses only normal/build edges.
            if any(kind.get("kind") != "dev" for kind in dependency["dep_kinds"]):
                pending.append(dependency["pkg"])

    entries = []
    for package_id in reached:
        package = packages[package_id]
        source = package.get("source")
        entries.append({
            "name": package["name"],
            "version": package["version"],
            "source_kind": "path" if source is None else source.split("+", 1)[0],
            "license": package.get("license"),
            "license_file": (Path(package["license_file"]).name
                             if package.get("license_file") else None),
        })
    entries.sort(key=lambda item: (item["name"], item["version"], item["source_kind"]))
    missing = [f"{item['name']} {item['version']}" for item in entries
               if not item["license"] and not item["license_file"]]
    return {
        "root": root_name,
        "lock_sha256": hashlib.sha256(lockfile.read_bytes()).hexdigest(),
        "package_count": len(entries),
        "missing_license_declarations": missing,
        "packages": entries,
    }


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--launcher-metadata", type=Path, required=True)
    parser.add_argument("--launcher-lock", type=Path, required=True)
    parser.add_argument("--app-metadata", type=Path, required=True)
    parser.add_argument("--app-lock", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    report = {
        "status": "DEVELOPMENT PREVIEW; notice review incomplete",
        "target": "x86_64-pc-windows-msvc",
        "scope": "Cargo metadata normal/build dependency graph for the selected binary roots; platform filtered, may include feature variants not linked into the binaries",
        "projects": [
            inventory(json.loads(args.launcher_metadata.read_text(encoding="utf-8")),
                      "pwds-strike-launcher", args.launcher_lock),
            inventory(json.loads(args.app_metadata.read_text(encoding="utf-8")),
                      "gridcraft", args.app_lock),
        ],
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(json.dumps({item["root"]: {
        "package_count": item["package_count"],
        "missing_license_declarations": item["missing_license_declarations"],
    } for item in report["projects"]}, indent=2))


if __name__ == "__main__":
    main()
