# GridStrike Windows development preview audit

This audit covers the pinned GridStrike source commit
`a4457c872b9fad0b492c30bf585cccfb4d25cd9e` and the installer development
preview workflow. It does not authorize a public release. `releases.json` remains
empty.

## Package contents and provenance

The ZIP contains `gridstrike.exe`, `LICENSE-MIT`, `LICENSE-APACHE`, `NOTICE`,
`ATTRIBUTION.md`, and `UPSTREAM_ATTRIBUTION.md` from the pinned source. The
second ZIP adds `SYNTHETIC-UPDATE.txt` and carries the *same* real executable;
its version `0.5.1` exists only to test update and rollback transactions.
`tools/package_preview.py` sorts ZIP paths and fixes their timestamps. The
workflow records source commits, Rust compiler details, runner image, byte
counts, SHA-256 hashes, and direct and delay-loaded PE DLL imports in
`provenance.json`. `Cargo.lock` is enforced with `--locked` for both projects.
The packager also checks that the launcher and GridStrike checkouts actually
have the recorded commit IDs. `dependency-inventory.json` records the Windows
Cargo normal/build dependency graph for each selected binary root, including
package versions, declared license identifiers or license file names, and
lockfile hashes. The independent consumer checks this inventory's length and
SHA-256 against `provenance.json`. Raw Cargo metadata is not uploaded because
it contains runner-specific local paths.

This inventory is a review aid, not a notice-completeness finding. Cargo's
resolved graph can contain feature variants that the binaries do not link;
conversely, package metadata does not supply the text of each dependency's
license or every attribution obligation. Its `missing_license_declarations`
entries flag absent metadata for manual investigation. Dependency license
texts and applicable notices have not yet been assembled into the ZIPs.
The pinned lockfiles currently resolve 207 packages in the launcher graph and
213 in the GridStrike graph. The launcher's root package is the sole entry
without a license declaration. The graphs also include
`epaint_default_fonts` with separate font license identifiers, which makes
their notice text a specific item to check before distribution. These counts
come from the selected Windows roots and are not a count of linked crates.
The executable's bytes are not claimed bit-for-bit reproducible across
different compilers or runner images.

The app source identifies its GridCraft donor and carries that donor's MIT
and Apache licenses and notice. Its local `ATTRIBUTION.md` says the adapted
GridStrike icon's separate authorship record still needs checking. It also
says a complete transitive dependency license and notice inventory remains
a release task. The launcher repository currently has no declared
distribution license file. Those items require resolution before public
download. ArtCraft trademark artwork is absent from the ZIP; the donor
notice explains that ArtCraft marks are excluded from the open source grant.

## Runtime and independent consumer check

The preview build requests static MSVC CRT linkage and fails packaging if a
GridStrike or launcher executable imports an external `VCRUNTIME` or `MSVCP`
DLL. The PE report also lists all direct and delay-loaded imports for review.
This check cannot prove that an app will never load a DLL dynamically. The
pinned GridStrike Cargo manifest uses eframe/WGPU; its version process exits
before opening the graphical editor. A graphics adapter and supported Windows
version still need a fresh desktop GUI test.

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
