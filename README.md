<p align="center">
  <img src="assets/branding/PWDS-logo.png" alt="PWDS publisher mark" width="120">
</p>
<p align="center"><strong>Published by PWDS</strong></p>
<h1 align="center">PWDS Strike Launcher</h1>

The launcher brings the Strike family into a single catalog. Browse each app's purpose and release status, then install, launch, update, roll back, or uninstall a pinned release when one is available.

**Status:** installer development preview. The embedded release manifest is empty, so no public Strike app installation is enabled. No downloadable public launcher or accepted clean-machine installer is offered here yet.

## Rust direction and migration status

- **Target:** Build the Rust suite launcher on the selected Craft Launcher foundation, carrying forward safe installation, receipt verification, update, rollback, and uninstall behavior.
- **Current implementation:** Published `main` is still the first-party Rust CLI and eframe/egui launcher described by [`Cargo.toml`](Cargo.toml); Craft Launcher adoption has not been integrated. The bundled public release manifest remains empty.
- **Remaining migration:** Verify and integrate the selected donor foundation without losing existing installer safeguards, then populate only verified release artifacts and test clean-machine installation. The current custom preview is not the completed foundation migration or an installable public suite.

## Native UI preview

These are inspected captures of the real Rust/egui launcher window from [merged PR #1](https://github.com/Gerbzz/PWDS-Strike-Launcher/pull/1), source commit [`bde73be`](https://github.com/Gerbzz/PWDS-Strike-Launcher/commit/bde73be12529557619ac8abf5cf98e1a2595dc32). They were taken on 2026-10-15 on a Linux virtual display with software OpenGL. These images are development previews, not a downloadable release.

![Native launcher catalog showing five coming-soon app cards](docs/screenshots/launcher-catalog.png)

![Native launcher library showing its empty state](docs/screenshots/launcher-library.png)

The bundled release manifest and capture runner's installed state were empty. The catalog shows unavailable installation controls, and the library shows no launcher-managed apps. See the [capture provenance](docs/screenshots/README.md) for the build run and file hashes.

### Installer development preview

The current Rust installer was also exercised with a real GridStrike 0.5.0 package built on a [standard Windows runner](https://github.com/Gerbzz/PWDS-Strike-Launcher/actions/runs/38778010775). These existing native Office PC captures show the isolated development-preview library before and after uninstall. They do not represent a public release.

![Native launcher library with verified GridStrike 0.5.0 installed](docs/screenshots/installer-gridstrike-preview-installed.png)

![Native launcher library after GridStrike preview uninstall](docs/screenshots/installer-gridstrike-preview-uninstalled.png)

See the [installer capture provenance](docs/screenshots/README.md#installer-development-preview) for the exact package hash, source commit, and test boundary.
The [synthetic update](docs/screenshots/installer-gridstrike-preview-synthetic-update.png)
and [rollback](docs/screenshots/installer-gridstrike-preview-rolled-back.png)
states were also captured on the Office PC with the prebuilt native launcher;
their [provenance and limits](docs/screenshots/README.md#fresh-update-and-rollback-capture-2026-10-16)
are recorded separately.

## Features

- Browse app cards with purpose and release status.
- Check each app's current development and release status.
- Read the same catalog from the native UI or CLI.

### Browse the catalog

The native Rust UI source has a catalog and library. Each card shows an app's name, purpose, release status, and a **Logo pending** slot where no approved asset is embedded. The CLI reads the same catalog and installed-state model:

```text
pwds-strike-launcher catalog
pwds-strike-launcher library
pwds-strike-launcher plan clipstrike
```

`plan` currently reports that there is no verified platform release. Install and update remain disabled until a reviewed artifact is pinned in `releases.json`. Launcher-owned installations show Launch, Rollback, and Uninstall controls after receipt and file verification.

### Installer behavior

- A single typed app catalog and versioned installed-state format are shared by CLI and UI. Receipts include a SHA-256 inventory for every installed file; older receipts without inventory cannot authorize launch or uninstall.
- A release manifest is embedded in the binary. Only that bundled origin can produce a `TrustedManifest`. Release validation checks app/repository identity, platform, architecture, numeric version, HTTPS GitHub release URL, pinned SHA-256 and byte length, and a safe relative entrypoint. Planning chooses a newer matching release only.
- The executor downloads only embedded release URLs, checks GitHub asset redirects, applies size and SHA-256 pins, rejects unsafe ZIP entries, and extracts into a same-volume staging directory. It keeps the prior version during update and can recover an interrupted receipt swap or roll back to the previous verified version.
- Launch uses the verified entrypoint directly. Uninstall removes only inventoried version directories and preserves user documents and settings. Native operations run in a worker thread with phase/byte progress, cancellation before activation, and error reporting.
- All catalog entries show "Coming soon" while the release manifest is empty. The `developer-preview` feature permits an explicit local GridStrike package and isolated install root for CI testing; it is not included in the default build.

## Build and install

Install a Rust toolchain supporting edition 2024 and the native graphics/windowing libraries for your OS. The native preview workflow builds the UI with:

```sh
git clone https://github.com/Gerbzz/PWDS-Strike-Launcher.git
cd PWDS-Strike-Launcher
cargo build --locked --features native-ui --bin pwds-strike-launcher-ui
cargo run --locked --features native-ui --bin pwds-strike-launcher-ui
```

These commands build and open the source application. They do not produce a signed or packaged installer. The default build excludes the opt-in `developer-preview` and `ui-local-fixture` features; the ordinary UI opens with no arguments. See the [native preview workflow](.github/workflows/native-preview.yml) and [capture guide](docs/NATIVE_CAPTURE.md) for the Linux runner setup.

### Release readiness

Current entries are source or development builds, with no accepted clean-machine installer or verified public release artifact. The catalog is a product list, not a release claim: a source link or local prototype shortcut never enables public installation. The embedded release manifest is empty; stable app IDs remain unprefixed for library-state compatibility. See [installation design](docs/INSTALL_DESIGN.md) for the verified-artifact contract.

## Validation status

The repository's installer integration tests use synthetic archives and isolated roots for corruption, traversal, cancellation, interrupted receipt swap, existing install, update, rollback, and uninstall. The `installer-development-preview.yml` workflow builds GridStrike from a pinned source commit on a standard Windows runner, packages it as a clearly labeled development preview, exercises install/launch/uninstall in an isolated runner root, and captures the native library. Its run artifact is evidence for that preview only.

Before enabling a public release, verify the uploaded asset bytes independently, complete its license/runtime notice audit, determine publisher signing policy and minimum OS, and repeat the workflow on a clean machine. User documents and settings must remain outside versioned app directories.

## Credits and licenses

PWDS publishes the launcher. The masthead uses the existing [official PWDS company mark](https://github.com/Gerbzz/PWDS-Website/blob/main/public/prowebdevsolutions-logo.png); it is publisher artwork, and catalog cards retain **Logo pending** where no approved app asset is embedded.

The launcher has no declared distribution license yet. Its dependency notice review and the separate GridStrike preview package notices are recorded in the [preview release audit](docs/PREVIEW_RELEASE_AUDIT.md). Those package notices do not assign a license to the launcher.
