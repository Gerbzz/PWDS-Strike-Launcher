# `hexf-parse 0.2.1` notice evidence

Checked 2026-10-16 for the Windows development preview's locked dependency.
This records source provenance and a missing text file; it does not decide
distribution obligations or change a preview package.

| Evidence | Observation |
| --- | --- |
| Launcher `Cargo.lock` and cached `.crate` | `hexf-parse 0.2.1` archive SHA-256 is `dfa686283ad6dd069f105e5ab091b04c62850d3e4cf5d67debad1933f55023df`, matching the lock checksum. The 4,662-byte archive has exactly `.cargo_vcs_info.json`, `Cargo.toml`, `Cargo.toml.orig`, and `src/lib.rs`; no notice text is bundled. |
| Published crate metadata | `Cargo.toml.orig` declares `license = "CC0-1.0"`, `repository = "https://github.com/lifthrasiir/hexf"`, and no `license_file`. `.cargo_vcs_info.json` pins `4225763d744183d720f575ae96d04161b4d08ea0`. The Rust source contains no license or copyright header. |
| [Pinned upstream package](https://github.com/lifthrasiir/hexf/blob/4225763d744183d720f575ae96d04161b4d08ea0/parse/Cargo.toml) and [workspace](https://github.com/lifthrasiir/hexf/blob/4225763d744183d720f575ae96d04161b4d08ea0/Cargo.toml) | Both declare `CC0-1.0`. A complete recursive filename check of that Git tree found no `LICENSE`, `LICENCE`, `COPYING`, `NOTICE`, `CC0`, or `UNLICENSE` file. A license/CC0/public-domain keyword check of its README found no separate statement. |
| [Later upstream relicense](https://github.com/lifthrasiir/hexf/commit/8a14eb63c3823b0ed5a04a4a200a03ccde648a0d) | In December 2023, both Cargo manifests changed from `CC0-1.0` to `0BSD`. The [repository LICENSE file](https://github.com/lifthrasiir/hexf/commit/41f0018229c1ee3d6fd813b6808d1ad1f506554c) was added afterward for `0BSD`. This later text is not evidence of a notice file in the pinned 0.2.1 source. |

The source-evidence gap is a full notice text file supplied with the pinned
version or present at its upstream commit. A release review must decide how
its `CC0-1.0` declaration should be handled. The canonical
[CC0 1.0 legal code](https://creativecommons.org/publicdomain/zero/1.0/legalcode.en)
exists separately; the pinned crate did not include a copy or declare a
`license_file` path. Do not substitute the repository's later `0BSD` file for
this version based on these facts alone.
