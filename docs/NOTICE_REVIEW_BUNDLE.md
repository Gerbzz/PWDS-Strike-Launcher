# Local notice review evidence, 2026-10-18

This is a source-evidence packet for the development preview, not a decision to
license PWDS Strike Launcher or publish a release. The bundle is in the local,
ignored `target/notice-review-20261018/` directory. Its ZIP is
`target/notice-review-20261018.zip` (1,304,143 bytes, SHA-256
`edf767a379de43ba1c4203311c3223039e4f5e39e76560bd9e722b63f2d66190`).
No app ZIP or executable was downloaded for this review.

The bundle's `file-inventory.json` records every evidence file's byte count and
SHA-256. The existing `tools/notice_coverage.py` checked every selected
registry archive against its exact `Cargo.lock` checksum and collected
candidate source notices for 182/207 launcher packages and 196/213 pinned
GridStrike packages in the Windows x86_64 normal/build Cargo graphs. The
existing `tools/upstream_notice_sources.py` verified 37 additional source files
against pinned Git blob IDs, covering 40 metadata-only entries. The bundle
also preserves the two GridStrike attribution files, original pinned font
notices, and a separate **reference-only** copy of the ArtCraft donor notices.
Those donor files are not asserted to be PWDS distribution notices.

## `hexf-parse 0.2.1`

The [version-specific evidence](HEXF_PARSE_NOTICE_EVIDENCE.md) applies to
this bundle. `canonical-CC0-1.0.txt` is the 7,048-byte plain text fetched from
[Creative Commons](https://creativecommons.org/publicdomain/zero/1.0/legalcode.txt),
SHA-256 `a2010f343487d3f7618affe54f789f5487602331c0a8d03f49e9a7c547cf0499`.
It is labeled **external canonical text**: the locked `hexf-parse 0.2.1` crate
(SHA-256 `dfa686283ad6dd069f105e5ab091b04c62850d3e4cf5d67debad1933f55023df`)
declares `CC0-1.0` but neither it nor its pinned upstream tree supplied this
file. The later upstream `0BSD` notice was not substituted. Whether to ship
this canonical copy is a notice-package decision; the source evidence is now
reviewable without claiming the copy came from the crate.

## Five egui 0.30.0 crate-to-tag comparisons

Each `.crate` SHA-256 was checked against launcher `Cargo.lock`. Git blob SHA-1
of every packaged source file was compared with the recursive tree of the
official [egui `0.30.0` tag](https://github.com/emilk/egui/tree/0.30.0), which
dereferences to `d864655018fcd73f2df629e5df4df04374b89a4b`. Cargo's
normalized `Cargo.toml` and `.cargo_vcs_info.json` were excluded; each
`Cargo.toml.orig` **was** checked against the tagged original manifest.
The egui crate's README matches the tag's repository-root README rather than
the short `crates/egui/README.md` pointer.

| Locked crate | Source blobs matching tag | Other packaged file |
| --- | ---: | --- |
| `egui 0.30.0` | 89 | None |
| `eframe 0.30.0` | 30 | None |
| `egui-wgpu 0.30.0` | 8 | None |
| `egui-winit 0.30.0` | 5 | None |
| `egui_glow 0.30.0` | 11 | Crate-only, Cargo-generated `Cargo.lock` |

There were no differing packaged source blobs and no missing source paths
after the README mapping. The `.cargo_vcs_info.json` commit recorded by these
five crates, `0a3edabd3544c6d339eacd9e606fccee302e4c35`, still returns
404 from the upstream GitHub commit API. Thus the comparison establishes
the exact packaged source-file relationship to the official tag, while the
unresolvable VCS commit remains a provenance discrepancy. The tagged MIT and
Apache license blobs also match those found at other pinned egui commits in
`tools/upstream_notice_plan.json`; the five entries remain labeled
`official-release-tag-proxy` rather than `exact-vcs`.

## ArtCraft reference check

The cloned ArtCraft reference is at commit `3fd2d56999fa00a7b90f159cb686a5fc64347e7e`.
Its own `LICENSE-MIT`, `LICENSE-APACHE`, `NOTICE`, and `ATTRIBUTION.md` were
inspected. The `NOTICE` expressly excludes ArtCraft marks from the code
license. A mechanical comparison of all four PWDS Rust source files against
the 73 ArtCraft Rust files found no substantive six-line exact code block.
The only 24-token matches were routine Rust argument parsing and egui UI
calls. This scan did not identify copied launcher source requiring a donor
notice. It is not proof of independent authorship or a substitute for a
human code-provenance review. PWDS assets contain PWDS branding rather than
ArtCraft marks.

The pinned GridStrike app has a separately documented GridCraft donor and its
license/notice files in the preview package; see [preview audit](PREVIEW_RELEASE_AUDIT.md).
The public `releases.json` remains empty.
