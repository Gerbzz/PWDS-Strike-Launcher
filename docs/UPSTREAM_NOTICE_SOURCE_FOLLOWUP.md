# Upstream notice source follow-up

The [Windows preview notice source audit](https://github.com/Gerbzz/PWDS-Strike-Launcher/pull/7)
found 42 package entries with license metadata but no candidate notice text in
their published crate archives or checked-out workspace root. The pinned
`tools/upstream_notice_plan.json` maps all 42 entries: 40 to upstream source
license files and two to unresolved decisions. It was compared against the
same selected Windows Cargo metadata: no entries were omitted or added.

For 35 entries, the source repository commit comes from the crate's
`.cargo_vcs_info.json`. GitHub's pinned Git tree identified the license file
and Git blob ID. The hosted collector fetches each small raw file at that
commit, recomputes the Git blob SHA-1, and records its SHA-256 and size in an
artifact manifest. No GitHub credential or local binary build is needed.

Five egui 0.30 entries have a crate VCS commit (`0a3edabd...`) that GitHub's
tree API does not currently resolve. Their candidate texts use the official
`0.30.0` release tag at `d864655...`; the plan labels those entries
`official-release-tag-proxy` and records the unresolved VCS commit. This is
source evidence, **not** proof that those five crates exactly match that tag.

Two entries remain unresolved:

- `hexf-parse 0.2.1` declares `CC0-1.0`, but its pinned upstream Git tree and
  locked crate archive have no license or notice text file. The
  [version-specific evidence](HEXF_PARSE_NOTICE_EVIDENCE.md) distinguishes it
  from the repository's later `0BSD` license file.
- `pwds-strike-launcher 0.1.0` has no approved distribution license choice.

The collected files are candidates for notice review. This audit makes no
claim about linked binary contents, which texts distribution requires,
license-choice obligations, or legal clearance. It does not alter the
development preview packages or the empty public release manifest.
