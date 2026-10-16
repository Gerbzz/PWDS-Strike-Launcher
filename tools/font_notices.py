"""Copy byte-pinned egui font notices from the locked Cargo dependencies.

These four files cover identified embedded fonts only. They are not a complete
third-party notice audit for either executable.
"""

import argparse
import hashlib
import json
from pathlib import Path


VERSIONS = {"launcher": "0.30.0", "gridstrike": "0.36.2"}
SOURCE_NOTICE_SHA256 = {
    "Hack-Regular.txt": "47c0cccbeec7e8614548cc485588b28149e7874188df5f41b36efebcee285c87",
    "OFL.txt": "6a73f9541c2de74158c0e7cf6b0a58ef774f5a780bf191f2d7ec9cc53efe2bf2",
    "UFL.txt": "2f0015108d68627bd788d313f529c21ff4da2c2c42a5e1f3883acc83480f9002",
    "emoji-icon-font-mit-license.txt": "b9d2c1d909aa149996fd4c91dcb92b2362a04431640c1d200959da94caf8cde1",
}
ATTRIBUTION = (
    "Embedded font attribution from the pinned epaint_default_fonts source.\n"
    "See the adjacent original notice files for full terms.\n"
    "Hack Regular: Copyright 2018 Source Foundry Authors; "
    "Copyright 2003 Bitstream, Inc.\n"
    "Noto Emoji Regular: Copyright 2013 Google Inc. All Rights Reserved.\n"
    "Ubuntu Light: Copyright 2011 Canonical Ltd. "
    "Licensed under the Ubuntu Font Licence 1.0.\n"
    "emoji-icon-font: Copyright (c) 2014 John Slegers.\n"
).encode("utf-8")
NOTICE_SHA256 = SOURCE_NOTICE_SHA256 | {
    "FONT-ATTRIBUTION.txt": hashlib.sha256(ATTRIBUTION).hexdigest()
}


def verified_files(directory: Path, expected: dict[str, str] = NOTICE_SHA256) -> dict[str, bytes]:
    actual_names = {path.name for path in directory.iterdir() if path.is_file()}
    if actual_names != set(expected):
        raise ValueError(f"font notice file set differs in {directory}: {sorted(actual_names)}")
    contents = {name: (directory / name).read_bytes() for name in expected}
    for name, data in contents.items():
        if hashlib.sha256(data).hexdigest() != expected[name]:
            raise ValueError(f"font notice bytes differ: {name}")
    return contents


def collect(metadata: dict, version: str, destination: Path,
            expected: dict[str, str] = SOURCE_NOTICE_SHA256) -> dict:
    packages = [package for package in metadata["packages"]
                if package["name"] == "epaint_default_fonts" and package["version"] == version]
    if len(packages) != 1 or not (packages[0].get("source") or "").startswith("registry+"):
        raise ValueError(f"expected one registry epaint_default_fonts {version} package")
    package = packages[0]
    source = Path(package["manifest_path"]).parent / "fonts"
    destination.mkdir(parents=True, exist_ok=False)
    for name in expected:
        (destination / name).write_bytes((source / name).read_bytes())
    (destination / "FONT-ATTRIBUTION.txt").write_bytes(ATTRIBUTION)
    all_expected = expected | {"FONT-ATTRIBUTION.txt": hashlib.sha256(ATTRIBUTION).hexdigest()}
    verified_files(destination, all_expected)
    return {
        "crate": "epaint_default_fonts",
        "version": version,
        "license_expression": package["license"],
        "files": all_expected,
    }


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--launcher-metadata", type=Path, required=True)
    parser.add_argument("--app-metadata", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=False)
    report = {}
    for group, metadata_path in (("launcher", args.launcher_metadata),
                                 ("gridstrike", args.app_metadata)):
        report[group] = collect(json.loads(metadata_path.read_text(encoding="utf-8")),
                                VERSIONS[group], args.output / group)
    (args.output / "manifest.json").write_text(
        json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )
    print(json.dumps({group: entry["version"] for group, entry in report.items()}))


if __name__ == "__main__":
    main()
