# Launcher screenshot provenance

These PNGs are actual native egui screenshot events from the launcher binary built from [draft PR #1](https://github.com/Gerbzz/PWDS-Strike-Launcher/pull/1), commit [`bde73be12529557619ac8abf5cf98e1a2595dc32`](https://github.com/Gerbzz/PWDS-Strike-Launcher/commit/bde73be12529557619ac8abf5cf98e1a2595dc32). The [hosted run 38706849753](https://github.com/Gerbzz/PWDS-Strike-Launcher/actions/runs/38706849753) completed both its standard Windows build and Linux capture jobs successfully. The capture job used Xvfb and Mesa software OpenGL. Both images were downloaded and visually inspected before publication; a prior capture with an oversized logo placeholder was discarded and the draft UI layout corrected.

| File | Contents | Size | SHA-256 |
| --- | --- | ---: | --- |
| `launcher-catalog.png` | Five app cards with purpose and coming-soon availability | 1050 × 760 | `213b23de40672c9addc2e745a1ac8e04abede346fb2cb1ffe10db3b414b866b` |
| `launcher-library.png` | Empty launcher-managed library | 1050 × 760 | `aa4de1b368b46798602f2c2d3091e4a10b2bef03c31aedc8456eba17162843e1` |

The run used the bundled empty release manifest and fresh runner-local installed state. These are development previews from an unmerged draft branch, not screenshots of a published launcher release. They demonstrate the catalog and empty library UI; they do not demonstrate downloading, installing, launching, updating, or uninstalling apps.
