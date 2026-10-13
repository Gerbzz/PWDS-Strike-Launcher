# Release and installation contract

This is a design gate for a future executor. The current executable **does not install anything**.

## Release authority

`releases.json` is compiled into the launcher, so a remote catalog cannot silently grant installation authority. Publishing a new release requires a reviewed launcher update that embeds a verified release record. A record names one stable app ID, `major.minor.patch` version, OS (`windows`, `linux`, `macos`), CPU architecture (`x86_64`, `aarch64`), an exact GitHub `Gerbzz/<approved repo>/releases/download/<tag>/<file>.zip` HTTPS URL, exact byte count, SHA-256 of the ZIP, and a safe relative entrypoint. The current manifest has zero records. Source repository existence, tags, workflow definitions, or a locally built executable do not satisfy this contract.

Before adding any record, produce and inspect a real, versioned installer payload: deterministic file list and hashes, app executable, runtime dependencies, license and third-party notices, minimum OS, entrypoint, documented user-data location, tested install/launch/update/uninstall behavior, and the CI build identity. Verify the uploaded release asset bytes independently of its workflow log and confirm the URL is public and stable. Record publisher and signing information. Unsigned or unpublished artifacts remain unavailable; signing and a trusted publisher verification policy must be chosen before enabling general distribution. No license or trademark clearance is inferred by this project.

## Safe executor requirements

1. Fetch only the embedded URL over HTTPS. Disable redirects by default or check every redirect against the exact approved origin and expected release path. Enforce timeouts, content-length and an 8 GiB cap, stream to a uniquely created same-volume staging directory, support cancellation between chunks, and never invoke a shell or remote script.
2. Verify downloaded length and SHA-256 against the embedded pin before opening the ZIP. Stop and retain a clearly labeled failed stage for inspection on any mismatch; do not alter the active version or receipt. A hash is an integrity pin, not proof of publisher identity by itself.
3. Inspect every archive entry before extraction. Reject absolute paths, drive or UNC prefixes, `.`/`..`, backslashes, Windows reserved names, path collisions after case folding, symlinks, junctions, hard links, device files, alternate data streams, excessive entry count, decompression ratio and total output size. Extract with create-new semantics beneath a canonical staging root, never following reparse points. Recheck actual paths after creation.
4. Verify the staged file inventory and regular-file entrypoint. Keep the executable and bundled dependencies inside a versioned application directory; keep documents/settings in a separate user-data directory. Do not run installer scripts. On Windows, do not require elevation for a per-user installation.
5. After a successful stage, rename it to a versioned directory on the same volume. Preserve the old active version until a durable new receipt is written and verified. Use an OS-appropriate atomic replacement for the active pointer/receipt, plus a journal or backup so cancellation, crash and power-loss recovery can identify the last good version. Never treat a partially staged folder as installed. Only permit cancellation before the commit boundary; after that, finish or recover the transaction.
6. Launch only the validated entrypoint beneath the selected active version using direct process APIs with an argument array and a fixed working directory. Do not consult PATH for the executable. An update runs the same verified flow and offers rollback to the previous version if first launch fails.
7. Uninstall only files recorded as launcher-owned within the validated app version directory. Preserve user projects, documents, settings and caches by default; require a separate, explicit choice before deleting any user data. Reject symlinks/reparse-point escapes and unknown ownership. A failed uninstall keeps the receipt and reports what remains.

Linux/macOS adapters can implement platform-specific data paths, activation and launch while sharing the catalog, receipt and release checks. Their release entries stay absent until packaged and tested on those systems.

## Receipt boundary

`InstalledState` currently **reads** a versioned `installed.json`; it does not create receipts. Future transaction code must write receipts only after a verified commit, with app ID, version, target, and relative entrypoint matching the installed bundle. A receipt file by itself must not authorize launching arbitrary paths. The launcher should reconcile it with the versioned directory, ownership record and verified file hashes before enabling launch, update or uninstall.

## Logo and branding gate

The launcher displays the PWDS brand and unchanged app product names. Each catalog card has a visible “Logo pending” slot. Replace a slot only with an approved first-party asset and recorded provenance/license for that app; do not borrow third-party artwork or silently synthesize a product logo.
