"""Verify a production-shaped UI cannot consume an otherwise valid local fixture."""

import hashlib
import json
import subprocess
import sys
import tempfile
import zipfile
from pathlib import Path


def main() -> None:
    binary = Path(sys.argv[1]).resolve()
    with tempfile.TemporaryDirectory(prefix="pwds-fixture-gate-") as temporary:
        folder = Path(temporary)
        archive = folder / "gridstrike.zip"
        with zipfile.ZipFile(archive, "w") as package:
            package.writestr("gridstrike.exe", b"synthetic production gate fixture")
        payload = archive.read_bytes()
        spec = folder / "fixture.json"
        spec.write_text(
            json.dumps(
                {
                    "schema": 1,
                    "releases": [
                        {
                            "version": "0.5.0",
                            "archive": str(archive),
                            "sha256": hashlib.sha256(payload).hexdigest(),
                            "bytes": len(payload),
                        }
                    ],
                }
            ),
            encoding="utf-8",
        )
        root = folder / "pwds-ui-fixture-production-gate"
        for flag in ("--check-local-fixture", "--local-fixture-install"):
            result = subprocess.run(
                [str(binary), flag, str(spec), str(root)],
                capture_output=True,
                text=True,
                timeout=30,
                check=False,
            )
            assert result.returncode != 0, (flag, result.stdout, result.stderr)
            assert "usage: pwds-strike-launcher-ui" in result.stderr, (
                flag,
                result.stdout,
                result.stderr,
            )
            assert not root.exists(), f"{flag} created an install root"


if __name__ == "__main__":
    main()
