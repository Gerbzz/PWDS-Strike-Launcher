# Strike catalog source audit

Checked 2026-10-16 against the 26 public `Gerbzz/*Strike` app repositories and
their default-branch descriptions/READMEs. The launcher has 11 catalog entries.
The category and short description describe current source capabilities, not
an installable binary. The embedded `releases.json` has no releases.

`App.github_repo` in `src/lib.rs` is a release URL authority constraint. It is
not a source link. A public source repository alone does not justify changing
`None` to `Some`: that would broaden which future embedded release URL the
launcher accepts. This audit changes no release authority fields.

| Catalog entry | Source | Checked capability and limit |
| --- | --- | --- |
| ClipStrike | [repo](https://github.com/Gerbzz/ClipStrike) | Organizes media and builds multitrack timeline edits. End-to-end playback and sync remain unverified. |
| DemoStrike | [repo](https://github.com/Gerbzz/DemoStrike) | Reviews supported CS2 demo events and stages supervised captures; external CS2/HLAE tools and media review are needed for footage. |
| StreamStrike | [repo](https://github.com/Gerbzz/StreamStrike) | Stores scenes and source settings. It does not yet preview, record, or stream. |
| AudioStrike | [repo](https://github.com/Gerbzz/AudioStrike) | Edits tracks and sessions; a bounded CLI workflow exported a WAV. The captured UI had playback hardware disabled. |
| GridStrike | [repo](https://github.com/Gerbzz/GridStrike) | Edits workbooks, formulas, and tables, with bounded XLSX and CSV checks. The launcher package is a development preview. |
| NoteStrike | [repo](https://github.com/Gerbzz/NoteStrike) | Writes/searches local Markdown notes with stable links and backlinks. No installer is supported by its source checkpoint. |
| PDFStrike | [repo](https://github.com/Gerbzz/PDFStrike) | Reorders PDF pages, adds review marks and supported form values, and exports a separate PDF. Broad PDF compatibility is unverified. |
| SkinStrike | [repo](https://github.com/Gerbzz/SkinStrike) | Composes layered CS2 finish artwork and exports PNG. Valve-compatible game-ready materials are unverified. |
| MapStrike | [repo](https://github.com/Gerbzz/MapStrike) | Makes box-based map blockouts and exports OBJ. Experimental VMAP source has not passed Hammer/CS2 validation. |
| ModelStrike | [repo](https://github.com/Gerbzz/ModelStrike) | Inspects and edits simple triangle meshes. It is not a Source 2 model compiler. |
| HungerStrike | [repo](https://github.com/Gerbzz/HungerStrike) | Reviews receipt purchases and tracks local kitchen stock. The default branch is a Windows alpha source preview without an installer or binary release. |

The other 15 public Strike app repositories are [MediaStrike](https://github.com/Gerbzz/MediaStrike),
[EffectStrike](https://github.com/Gerbzz/EffectStrike),
[CodeStrike](https://github.com/Gerbzz/CodeStrike),
[ReplayStrike](https://github.com/Gerbzz/ReplayStrike),
[SlideStrike](https://github.com/Gerbzz/SlideStrike),
[WordStrike](https://github.com/Gerbzz/WordStrike),
[GameStrike](https://github.com/Gerbzz/GameStrike),
[DesignStrike](https://github.com/Gerbzz/DesignStrike),
[LightStrike](https://github.com/Gerbzz/LightStrike),
[FigStrike](https://github.com/Gerbzz/FigStrike),
[AnimateStrike](https://github.com/Gerbzz/AnimateStrike),
[VectorStrike](https://github.com/Gerbzz/VectorStrike),
[FontStrike](https://github.com/Gerbzz/FontStrike),
[CADStrike](https://github.com/Gerbzz/CADStrike), and
[PhotoStrike](https://github.com/Gerbzz/PhotoStrike). They have no current
launcher catalog entry. This audit does not infer installer availability from
their source repositories.
