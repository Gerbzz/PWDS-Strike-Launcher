# Isolated native UI install fixture

This opt-in test path exists only with `native-ui,ui-local-fixture`. Default and
`native-ui,developer-preview` builds do not accept these commands. It never
modifies the embedded `releases.json` or creates a `TrustedManifest` from a
local file. It is for a coordinated desktop test, not a public app installer.

The fixture JSON has schema 1 and one or two **GridStrike-only** releases,
ordered as real preview `0.5.0` then optional synthetic update `0.5.1`:

```json
{
  "schema": 1,
  "releases": [
    {
      "version": "0.5.0",
      "archive": "ABSOLUTE_PATH_TO_REVIEWED_GRIDSTRIKE_PREVIEW_ZIP",
      "sha256": "64_HEX_DIGITS_FROM_HOSTED_PROVENANCE",
      "bytes": 0
    }
  ]
}
```

Replace every placeholder, including `bytes`, with the reviewed artifact's
actual values. The SHA-256 must come from its hosted provenance rather than
being accepted merely because it matches an unreviewed local file. The
fixture loader checks each package's size and hash before a window opens;
the normal `InstallRoot::install_archive` path checks them again and applies
the regular ZIP path, inventory, receipt, update, and rollback rules. No
fixture URL or signing identity is invented.

The root argument must name a new directory beneath the system temp directory
whose final component starts `pwds-ui-fixture-`. A created root gets a marker
so a later invocation can reopen **that** fixture root for update or rollback.
An existing unmarked directory and a root outside temp are rejected. Uninstall
preserves unrelated user data in the root.

The hosted `UI local fixture validation` workflow builds a Windows test binary
as a short-lived CI artifact; use that binary for a desktop test rather than
building locally. These commands are available only in that binary:

```text
pwds-strike-launcher-ui.exe --check-local-fixture SPEC.json NEW_TEMP_ROOT
pwds-strike-launcher-ui.exe --local-fixture-install SPEC.json NEW_TEMP_ROOT
pwds-strike-launcher-ui.exe --local-fixture SPEC.json NEW_TEMP_ROOT
```

`--check-local-fixture` verifies inputs without creating install state or
opening a window. `--local-fixture-install` exercises the same fixture planner
and normal install transaction headlessly for CI. `--local-fixture` opens the
native UI with the GridStrike Install/Update button enabled. That final
command takes desktop focus; coordinate access before using it on Neil's
Office PC. Capture only the app window after the actual button click and
record the binary/source hash, package hash, isolated root, and resulting
receipt. The synthetic `0.5.1` package must remain labeled as a transaction
fixture, not an app release.
