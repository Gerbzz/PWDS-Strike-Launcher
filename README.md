# PWDS Strike Launcher

**by PWDS**

**The Strike apps in one place.**

The launcher brings the Strike family into a single catalog. Browse each app's purpose and release status, and see a library designed to track verified installations when packaged releases exist.

### Highlights

- Browse app cards with purpose and release status.
- See which apps are available and which are still in development.
- Read the same catalog from the native UI or CLI.

## Current status

Source foundation, not a downloadable launcher. The release manifest is empty, so all apps currently show as coming soon. There is no download, install, launch, update, or uninstall executor; those controls remain disabled.

## Browse the catalog

The native Rust UI source has a catalog and library. Each card shows an app's name, purpose, release status, and a **Logo pending** slot where no approved asset is embedded. The CLI reads the same catalog and installed-state model:

```text
pwds-strike-launcher catalog
pwds-strike-launcher library
pwds-strike-launcher plan clipstrike
```

`plan` currently reports that there is no verified platform release. Install, launch, update, and uninstall controls are disabled until verified artifacts and executors exist.

## What was found locally

The five suite shortcuts in `2026-10-09/task/CS2_Creator_Suite_Local` point to local debug or prototype binaries. Its README and status call them local prototypes with no clean-machine installer acceptance. StreamStrike in `2026-10-12/task-2/StreamStrike` is source-only. The public ClipStrike, DemoStrike, AudioStrike and GridStrike repositories are source references; this project has not verified their installer releases or CI artifacts. Other entries have prototype or unverified distribution status. Source repositories and local shortcuts are never converted into install buttons by the launcher.

The catalog is a product list, not a release claim. Stable IDs stay unprefixed for future state compatibility. See [`docs/INSTALL_DESIGN.md`](docs/INSTALL_DESIGN.md) for the release contract and installation security design.

## Current implementation

- A single typed app catalog and versioned installed-state format are shared by CLI and UI. The state reader validates schema, IDs, versions, target, paths, and a 1 MiB input bound. It reads `installed.json` from the platform's local app-state location if present; the launcher does not create or mutate it yet.
- A release manifest is embedded in the binary. Only that bundled origin can produce a `TrustedManifest`. Release validation checks app/repository identity, platform, architecture, numeric version, HTTPS GitHub release URL, pinned SHA-256 and byte length, and a safe relative entrypoint. Planning chooses a newer matching release only.
- `verify_artifact` streams a file through SHA-256 and checks its exact pinned byte count before any future extraction. It does not download or activate the file.
- All catalog entries show “Coming soon” unless a real release is embedded. The library shows only validated launcher receipts. Buttons are disabled because no executor exists.

## Verification and limits

At the first check C: was **below the requested 20 GiB reserve** (20,141,416,448 bytes free). It later rose to 21,961,342,976 bytes free, only about 486 MB above the reserve. No build, dependency fetch, copy of binaries, or UI window launch was attempted. Source formatting and offline Cargo metadata checks passed; compilation and visible UI review remain outstanding. Even a successful source test would not prove a clean-machine installer or secure end-to-end update flow.

The next functional milestone is one properly licensed and packaged Windows release with a pinned artifact, a bounded downloader and safe ZIP extraction into same-volume staging, atomic activation and recovery, then a clean-machine install/update/uninstall test. User documents and settings must stay outside versioned app directories and remain after uninstall by default.
