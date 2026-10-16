# Launcher screenshot provenance

The original catalog and empty-library PNGs are actual native egui screenshot events from the launcher binary built from [merged PR #1](https://github.com/Gerbzz/PWDS-Strike-Launcher/pull/1), commit [`bde73be12529557619ac8abf5cf98e1a2595dc32`](https://github.com/Gerbzz/PWDS-Strike-Launcher/commit/bde73be12529557619ac8abf5cf98e1a2595dc32). The [hosted run 38706849753](https://github.com/Gerbzz/PWDS-Strike-Launcher/actions/runs/38706849753) completed both its standard Windows build and Linux capture jobs successfully. The capture job used Xvfb and Mesa software OpenGL. Both images were downloaded and visually inspected before publication; a prior capture with an oversized logo placeholder was discarded and the UI layout corrected.

| File | Contents | Size | SHA-256 |
| --- | --- | ---: | --- |
| `launcher-catalog.png` | Five app cards with purpose and coming-soon availability | 1050 × 760 | `213b23de40672c9addc2e745a1ac8e04abede346fb2cb1ffe10db3b414b866b` |
| `launcher-library.png` | Empty launcher-managed library | 1050 × 760 | `aa4de1b368b46798602f2c2d3091e4a10b2bef03c31aedc8456eba17162843e1` |

The run used the bundled empty release manifest and fresh runner-local installed state. These are development previews, not screenshots of a published launcher release. They demonstrate the catalog and empty library UI; they do not demonstrate downloading, installing, launching, updating, or uninstalling apps.

## Installer development preview

The following app-only PNGs were captured on Neil's Office PC with the native `--capture library` path of the launcher executable built by [hosted Windows run 38778010775](https://github.com/Gerbzz/PWDS-Strike-Launcher/actions/runs/38778010775), launcher commit [`d5121dd`](https://github.com/Gerbzz/PWDS-Strike-Launcher/commit/d5121ddcfdd16abacf4a9bbb38895c5353d4a8ae). They were visually inspected before being added here. That run built GridStrike from source commit `a4457c872b9fad0b492c30bf585cccfb4d25cd9e` on a standard runner and exercised isolated install, `--version` execution, and uninstall. The runner had no suitable graphics adapter, so the native screenshots were captured locally using its prebuilt launcher UI binary. No local application or launcher build was needed.

| File | Contents | Size | SHA-256 |
| --- | --- | ---: | --- |
| `installer-gridstrike-preview-installed.png` | Verified GridStrike 0.5.0 receipt in the native library | 1050 x 760 | `b4b0f261b22ad39083e171903eb19f223adf81fbb6898cc721845e8de3e45771` |
| `installer-gridstrike-preview-uninstalled.png` | Empty library after uninstall | 1050 x 760 | `5b232aa6b8b47b8b021d8dd320ef5c84ab7372e3b43b2475d193378b94f0a51c` |

The development-preview ZIP was 13,680,853 bytes with SHA-256 `bcc72a7cfb67d44723ab120d107c2b59780cdc735e03a1b844b5f22b986e87d7`, matching the runner's `provenance.txt`. On the Office PC it was installed in a new isolated workspace root; the launcher-managed `preview-launch-version` command returned `GridStrike 0.5.0`, and uninstall removed the inventoried app version while preserving a user-data marker in that root. The bundled public release manifest remains empty. These captures do not claim public distribution readiness.

## Fresh update and rollback capture, 2026-10-16

On Neil's Office PC, the already built launcher CLI and native UI binaries
from [hosted run 38791571471](https://github.com/Gerbzz/PWDS-Strike-Launcher/actions/runs/38791571471)
were checked against its `provenance.json` before use. Their source commit is
`0fbc876ef4ab7fff168ae73b3fb2beb9433749fc`; the pinned GridStrike source
is `a4457c872b9fad0b492c30bf585cccfb4d25cd9e`. The real 0.5.0 preview
ZIP was 13,201,172 bytes, SHA-256
`2212614e39012e3ba8d8d6e0b9c829325ba3c4c29e3d5899a84e74672a9691b8`.
The 0.5.1 ZIP, SHA-256
`0622f928d8186872c91e317856c58a41c2751d4f1d54c6d47ca0c2bf9b9daca0`,
is a synthetic transaction fixture with the same GridStrike executable.

The CLI installed 0.5.0 in a new isolated `target/office-audit-20261016`
root, started the installed app's `--version` process, activated the synthetic
0.5.1 update, rolled back to 0.5.0, and uninstalled both inventoried versions.
The `user-project.txt` marker outside the version directories survived. After
each transaction, `pwds-strike-launcher-ui.exe --capture library NEW.png`
rendered the native library with `PWDS_LAUNCHER_PREVIEW_ROOT` set to that
isolated root. These were app-only screenshots, visually inspected here:

| State | File | SHA-256 |
| --- | --- | --- |
| Installed 0.5.0 | `installer-gridstrike-preview-installed.png` | `b4b0f261b22ad39083e171903eb19f223adf81fbb6898cc721845e8de3e45771` |
| Synthetic update active at 0.5.1 | `installer-gridstrike-preview-synthetic-update.png` | `e1e57625cb150e0aaa1c1efae2bf7629258cc1fa8f2de9deb85f4930e259270b` |
| Rolled back to 0.5.0 | `installer-gridstrike-preview-rolled-back.png` | `df729624ca7f7aa97c1a0301eabdba69d82ec705626f531f2e03d98cdb95f4e9` |
| Uninstalled | `installer-gridstrike-preview-uninstalled.png` | `5b232aa6b8b47b8b021d8dd320ef5c84ab7372e3b43b2475d193378b94f0a51c` |

The fresh installed and uninstalled PNGs were byte-identical to the existing
files, so only the two new states were added. These captures show native UI
rendering of verified receipts. The transactions were invoked through the CLI,
not by clicking the controls, and the GridStrike graphical editor was not
opened. The release manifest remains empty.
