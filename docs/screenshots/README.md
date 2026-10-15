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
