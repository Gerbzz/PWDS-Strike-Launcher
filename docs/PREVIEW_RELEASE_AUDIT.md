# GridStrike Windows development preview audit

This audit covers the pinned GridStrike source commit
`a4457c872b9fad0b492c30bf585cccfb4d25cd9e` and the installer development
preview workflow. It does not authorize a public release. `releases.json` remains
empty.

## Package contents and provenance

The ZIP contains `gridstrike.exe`, `LICENSE-MIT`, `LICENSE-APACHE`, `NOTICE`,
`ATTRIBUTION.md`, and `UPSTREAM_ATTRIBUTION.md` from the pinned source, plus
the four byte-pinned original `epaint_default_fonts 0.36.2` font notice texts
and a factual `FONT-ATTRIBUTION.txt` under `THIRD_PARTY/`. The standalone
launcher artifact carries the corresponding `epaint_default_fonts 0.30.0`
texts in `font-notices/launcher/`. The
second ZIP adds `SYNTHETIC-UPDATE.txt` and carries the *same* real executable;
its version `0.5.1` exists only to test update and rollback transactions.
`tools/package_preview.py` sorts ZIP paths and fixes their timestamps. The
workflow records source commits, Rust compiler details, runner image, byte
counts, SHA-256 hashes, and direct and delay-loaded PE DLL imports in
`provenance.json`. `Cargo.lock` is enforced with `--locked` for both projects.
The packager also checks that the launcher and GridStrike checkouts have the
recorded commit IDs and no tracked changes. Fresh hosted checkout and build
order support provenance, but a commit ID check alone cannot prove which
source bytes produced a binary. `dependency-inventory.json` records the Windows
Cargo normal/build dependency graph for each selected binary root, including
package versions, declared license identifiers or license file names, and
lockfile hashes. `lock_sha256` identifies checked-out bytes and can differ
with line endings; `lock_git_blob_sha256` identifies the committed Git blob
bytes for comparison across platforms. The independent consumer checks this
inventory's length and SHA-256 against `provenance.json`. Raw Cargo metadata is not uploaded because
it contains runner-specific local paths.

This inventory is a review aid, not a notice-completeness finding. Cargo's
resolved graph can contain feature variants that the binaries do not link;
conversely, package metadata does not supply the text of each dependency's
license or every attribution obligation. Its `missing_license_declarations`
entries flag absent metadata for manual investigation. The four identified
font texts and copyright attribution are now included; the broader dependency notice review is still
open.
The pinned lockfiles currently resolve 207 packages in the launcher graph and
213 in the GridStrike graph. The launcher's root package is the sole entry
without a license declaration. The graphs also include
`epaint_default_fonts` with separate font license identifiers. These counts
come from the selected Windows roots and are not a count of linked crates.
The executable's bytes are not claimed bit-for-bit reproducible across
different compilers or runner images.

The app source identifies its GridCraft donor and carries that donor's MIT
and Apache licenses and notice. Its workspace and desktop manifest declare
`MIT OR Apache-2.0`. The launcher repository has neither a distribution
license file nor a `license` field in its Cargo manifest; the donor's choice
does not declare the launcher's choice. The pinned GridStrike icon's three
raster files first appear in Neil's commit `a4457c8`. The 256-pixel glyph is
visibly distinct from the donor GridCraft icon, but the repository has no
editable source design or separate authorship record for it. ArtCraft
trademark artwork is absent from the ZIP; the donor notice states that its
marks are excluded from the source license.

`epaint_default_fonts` embeds Hack Regular, Noto Emoji Regular, Ubuntu Light,
and emoji icon fonts in both graphical executables. The crate's `fonts/` directory has
four matching texts: `Hack-Regular.txt` (including Source Foundry and
Bitstream notices), `OFL.txt`, `UFL.txt`, and
`emoji-icon-font-mit-license.txt` (John Slegers). The Noto TTF identifies
Google's 2013 copyright, and Ubuntu Light identifies Canonical's 2011
copyright in their font metadata; the generated attribution file records
these strings. The previous preview ZIP contained none of the four original
text files. The packager now checks their bytes against pinned SHA-256 values
before copying them. This resolves the identified missing
font texts, not the full transitive notice inventory or icon authorship gate.

## Runtime and independent consumer check

The preview build requests static MSVC CRT linkage and fails packaging if a
GridStrike or launcher executable imports an external `VCRUNTIME` or `MSVCP`
DLL. The PE report also lists all direct and delay-loaded imports for review.
This check cannot prove that an app will never load a DLL dynamically. The
pinned GridStrike Cargo manifest uses eframe/WGPU; its version process exits
before opening the graphical editor. The launcher's native library did render
on the Office PC after isolated install, synthetic update, rollback, and
uninstall; [app-only captures](screenshots/README.md#fresh-update-and-rollback-capture-2026-10-16)
record those states. The GridStrike graphical editor still needs a fresh
desktop GUI test on a supported Windows version with a graphics adapter.

The separate Windows consumer job downloads the artifact with no source
checkout or build step. It removes developer directories from `PATH`, asserts
that Cargo, rustc, MSVC `cl`, and GStreamer command line tools cannot be
resolved, checks the pinned ZIP hashes, and runs isolated install, GridStrike
`--version`, launcher process start, synthetic update, rollback, and uninstall.
A user data marker outside version directories must survive. The hosted image
may still contain development tools elsewhere on disk, so this is a
toolchain-independent process check, not proof of a minimal Windows machine.
The runner has no dependable graphics adapter; `--version` is not a GUI test.

## Unsigned preview

The hosted preview executables are unsigned. Microsoft's
[SmartScreen overview](https://learn.microsoft.com/en-us/windows/security/operating-system-security/virus-and-threat-protection/microsoft-defender-smartscreen/)
says downloaded apps and signatures undergo reputation checks and items with
no reputation can trigger a warning. The preview has no established publisher
identity or reputation claim. Publisher signing policy, certificate custody,
supported Windows versions, and GUI behavior remain public-release decisions.
No signing key or paid service is used by this workflow.
