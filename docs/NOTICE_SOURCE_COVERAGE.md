# Windows preview notice source coverage

The preview's `dependency-inventory.json` records Cargo license declarations,
but declarations are not license or notice text. This audit collects text files
from locked registry crate archives and the pinned GridStrike workspace. It
does not decide which texts are legally required for a distributed binary.

`tools/notice_coverage.py` selects normal/build edges for the launcher and
GridStrike Windows binary roots, as the preview inventory does. It checks each
crate archive's SHA-256 against its exact `Cargo.lock` checksum before reading
candidate `LICENSE`, `LICENCE`, `COPYING`, `NOTICE`, `OFL`, `UNLICENSE`,
`PATENTS`, and nested `licenses/` files. It records each collected file's
SHA-256, size, and source path. GridStrike workspace license files come from
the pinned source checkout. Packages with no source text are listed as
`metadata_only` even when Cargo declares an SPDX license expression.

An Office PC read-only scan of the existing pinned metadata and Cargo cache on
2026-10-16 found 182 of 207 launcher graph packages with candidate text
(338 files, 1,726,152 bytes), and 196 of 213 GridStrike graph packages
(397 files, 2,067,951 bytes). The 25 launcher and 17 GridStrike
`metadata_only` entries include shared upstream families such as egui and
AccessKit. The launcher workspace package has no distribution license text.
The pinned GridStrike workspace's `LICENSE-MIT` and `LICENSE-APACHE` files
cover its path packages as source evidence. The hosted audit artifact contains
the full package-level manifest and exact collected bytes.

The graphs may include feature variants that are not linked into these binaries.
The collector does not inspect linked object files, resolve license-choice
obligations, determine patent/attribution duties, or review dependencies
outside these Cargo graphs. Its artifact is source review evidence, not a
redistributable notice bundle or a legal clearance. The current development
preview packages are not changed by this audit. A future packaging change
must choose verified notices to include and pass separate release review.
