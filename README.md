# PWDS Strike Launcher

**Source foundation, not a downloadable launcher or app installer.** PWDS is the suite publisher brand. Product names remain ClipStrike, DemoStrike, StreamStrike, and so on. No app is marked installable today: [`releases.json`](releases.json) intentionally has no releases, and there is no download, extraction, activation, launch, update, or uninstall executor.

The native Rust UI source opens with prominent **PWDS Strike Launcher** branding, a catalog and a library. Each card shows its product name, purpose, release status, and a clearly marked **Logo pending** slot. The app sources inspected so far did not expose approved first-party logo assets to embed. The controls for install, launch, update and uninstall are disabled until real implementations and release evidence exist. The CLI reads the same catalog and installed-state model:

```text
pwds-strike-launcher catalog
pwds-strike-launcher library
pwds-strike-launcher plan clipstrike
```

`plan` currently returns “No verified release for this platform; coming soon.” A `native-ui` Cargo feature contains the egui/eframe UI, consistent with the existing Rust suite prototypes. No mandatory account, server, paid service, or remote telemetry is part of this source.

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
