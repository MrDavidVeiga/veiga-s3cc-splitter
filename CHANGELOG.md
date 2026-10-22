# Changelog — Veiga's S3CC Splitter

## Beta 2 — Versão Beta Final 2.1 (v2.1.0)

**Canonical development branch:** beta-final/v2.1.0  
**Lineage:** former Beta 3 (previously labeled 2.2.0 / “Beta 3”), now officially **Beta 2 v2.1.0**.  
**Documentation consolidated:** 2026-10-22 (this is NOT an invented release date).

This retrospective changelog covers features present in the canonical branch at documentation time. It does not imply that the code changes were all made on this date, that all platforms have passed final acceptance, or that experimental changes proposed in other branches were released. “Beta Final” is the name of the active beta line, not a claim of production readiness.

### Improved — Manager WinUI 3 layout parity (2026-10-22)

- Unified the **language / EN-PT-ES abbreviation / theme** topbar geometry with the latest S3CC Manager reference (`MrDavidVeiga/S3CC-Manager`, `fix/canonical-category-folders-20261019`), including spacing, 14 px labels, Fluent control heights and 12/16 px icons.
- Matched the Manager **content filter ComboBox and name/Instance ID SearchBox** typography, colors, dimensions, dropdown radius and focus styling.
- Reused the Manager's **results list shell** (`manager-list-card` and `package-list` classes) with 18 px outer radius and 28 px fixed list-header spacing, without replacing the Splitter's resource virtualizer.
- Restored the Manager's **WinUI 3 native Fluent scrollbar** (12 px stable channel; 4 px resting thumb inset, 2 px on hover, 1 px when pressed) for resources, Merger inputs, preview and filter dropdown.
- Matched the main **content panel (outer)** border, corner radius, fill and padding beside the sidebar.
- Restyled **info / warning / error / success / confirmation dialogs** with Manager-style state icons, localized text layout, themed surfaces and centered 120 px action buttons, retaining the Splitter's existing modal keyboard behavior.
- Added UI parity regression tests; frontend test/build validation passed on both branches, and the Beta Final 2.1 WinUI browser visual smoke passed after correcting the initial 140 px modal-button mismatch to 120 px.
- **Scope:** visual, HTML and CSS parity only. No change to resource merge/export engines or claims that Merger gameplay lag has been resolved. See PRs [#19](https://github.com/MrDavidVeiga/veiga-s3cc-splitter/pull/19) and [#20](https://github.com/MrDavidVeiga/veiga-s3cc-splitter/pull/20).

### Added — Core Splitter, Merger, and reversible operations

- **Splitter** and **Merger** in the same desktop application, with distinct operating modes.
- Open, scan, and inspect supported .package/.dbc files for The Sims 3, with logical content grouping and resource details.
- Filter by content category, search by name or **Instance ID**, and select an item, a range (**Shift + click**), or the visible filtered result set.
- New scans begin with no resource selected; **Select All / Select None** operate only on currently visible results.
- Export selected content with associated resources where recognized, plus supported options for thumbnail inclusion, compression, and clone packages.
- Create merged packages from multiple inputs or supported dragged folders while avoiding identical TGI duplicates and **rejecting conflicting TGI data**.
- **Veiga Merge Manifest** retains original source filenames, recorded ownership, selected CASP/OBJD anchors, and per-source metadata needed for supported **Unmerge**.
- **Unmerge / Revert** reconstructs component packages from valid manifest-backed merges, including original names where recorded.
- **Remove from Merged** works for manifest-backed merges and also offers dependency-aware removal for packages without a valid Veiga manifest, with required backup protection.
- Differentiate **Duplicate**, **TGI Conflict**, and **Same Name** using logical resource relationships rather than relying only on names or shared dependencies.
- Present removal confirmation ratios (X/X) according to either manifest-backed source-package counts or non-manifest logical-item counts.

### Added — CLIP, poses, and PoseList support

- Recognize ordinary **CLIP** entries as **Clip**, not automatically as poses.
- Mark content as **Pose** only when a matching **PoseList XML explicitly references a CLIP resource** in the package.
- Treat a detected PoseList as a logical pose item and include linked CLIP resources, the matching S3SA registrar, and associated icon resources when present.
- Do **not** infer pose classification from a package filename or naming convention.
- When merging multiple PoseLists, combine unique **Pose** entries into one XML, retain a matching S3SA registrar, skip extraneous empty templates and deduplicate repeated CLIP keys.
- Preserve original PoseList XML and associated S3SA data in the Veiga Merge Manifest so Unmerge can restore original source packages instead of redistributing the consolidated XML.
- Support removal of the combined PoseList XML and registrar when a consolidated pose item is selected, retaining dependencies still needed by other content.
- The branch README reports *two experimental in-game checks* (Juniper registrar TEST_A and ROSELIPA registrar TEST_B), both showing 18 usable poses. This does not establish universal compatibility across all pose packs.

**Suggestion credit: GreatDaySimmer.** The original suggestion to **add pose support** came from **GreatDaySimmer**. Project implementation and subsequent test outcomes are separate from this suggestion credit; do not assign all pose engineering or gameplay tests to a person without a specific source.

### Added — Resource thumbnail fallback

- Prefer an embedded thumbnail linked to the selected resource whenever it is usable.
- When a normal resource lacks a package thumbnail, search supported game thumbnail-cache packages under the actual Documents/The Sims 3/Thumbnails directory: **CASThumbnails.package**, **ObjectThumbnails.package**, and **DownloadsThumbnails.package**.
- Try the most relevant cache first and identify candidates by Instance ID. If neither embedded nor external thumbnails match, the preview is unavailable or shows the interface's established llama fallback where applicable; no unrelated image should be treated as a match.

### Improved — Performance and user interface

- Use more efficient indexed access and memory-mapped package handling for scanning/preview where implemented, with virtualized resource lists and guarded expensive relationship analysis.
- Improve large-package scanning, export, and memory behavior, informed by real CC-library reports; progress messages remain visible during lengthy processing.
- WinUI 3-inspired themes and visual parity with the companion S3CC Manager (including localized theme labels, relevant indicators and EA App blue accent).
- Responsive English/Portuguese/Spanish interface, with **Theme / Tema** and **Idioma / Language** and PT/EN/ES codes sized consistently at **14 px** while retaining intended font-weight differences.
- Center status text and progress bar on the Splitter work surface **without a backing card**; display an animated progress marquee while the backend remains busy rather than only when it updates progress milestones.
- Center the Merger's empty-state message to align with the Splitter's initial message; restore the regular top-aligned list when sources are present.
- Standardize compact centered **Cancel / Remove / OK** dialog buttons around **120 px** and preserve usable layouts on narrow windows.
- Prioritize real thumbnail previews and keep the original llama image when an in-package visual fallback is necessary.
- Maintain themed dialogs, clearer duplicate/conflict indicators, content list actions, and keyboard interaction.

### Fixed or guarded — Integrity and reversibility

- **Duplicate detection:** avoid treating shared resources or the same displayed name as proof of duplicated CC; indicate unverified same-name cases separately.
- **TGI conflicts:** cancel ambiguous merges instead of silently choosing conflicting payload data.
- **Source ownership and names:** retain original package-name metadata for supported Unmerge workflows, following tester feedback about names changing or being lost.
- **Removing merged content:** preserve resources still shared by retained items and use a verified bit-for-bit backup before replacing a non-manifest source.
- **PoseList reversal:** retain original XML/registrar resources needed for source reconstruction even when a consolidated, playable merged PoseList is produced.

### Known issues / open investigations

- **Merger gameplay lag — OPEN.** GreatDaySimmer reported that the game stuttered significantly with CC merged through this tool, while the same original CCs re-merged in **s3pe** appeared to run normally. The precise cause is not yet demonstrated by a controlled same-input A/B test. Do **not** mark this issue “Fixed” yet.
- **Catalog names — UNCONFIRMED CAUSE.** GreatDaySimmer observed some Build/Buy items without names or displaying source package filenames after unmerging through Splitter and merging with s3pe. The origin of this naming behavior has not been established.
- **Experimental Merger compatibility changes — NOT IN THIS BRANCH.** Pull request **#18** proposes moving the private reversibility manifest away from the game's XML resource TypeID and sorting generated package index entries. As long as the PR is not merged, those proposals belong under investigation, **not** under released fixes. See https://github.com/MrDavidVeiga/veiga-s3cc-splitter/pull/18.
- **Unmerge scope:** requires a valid Veiga Merge Manifest; non-manifest removal can preserve detectable shared resources but cannot reconstruct the exact original packages automatically.
- **Performance and content compatibility:** passing automated checks or opening a DBPF file does not by itself establish smooth gameplay or proper catalog labels. Keep original CC packages and test one merged variant at a time.

### Community contributions — specific credit for ideas, reports and tests

- **GreatDaySimmer (greatdaysimmer):**
  - **Suggested adding pose support** — including the impetus for the CLIP/PoseList work in this beta.
  - Provided reports and test cases about original merged/unmerged package filenames and duplicate classification.
  - Shared real-world Merger/Unmerge testing.
  - Reported **Merger-associated lag compared with s3pe** and possibly related but as-yet-unattributed Build/Buy catalog-name differences. These are important **open bug reports**, not completed fixes.
- **Jacinda:** suggested **Shift + click** range selection and reported an export/Select All crash with hundreds of items.
- **quietlyshout:** reported large-export RAM usage and duplicate-name export problems; suggested more compact/resizable presentation and clearer duplicate indicators.
- **starshiphelix:** suggested **Instance ID** search for locating specific CC found through external tools.
- **Kitilith / sims3-rs:** upstream credit for the Sims 3 DBPF-reading foundation, separate from end-user feedback.

Names above credit the **specific suggestion, bug report, or test**, not exclusive implementation authorship. No contributor is assigned an unverified change.

### Builds, versioning, and history

- **v2.1.0 (Beta 2 / Versão Beta Final 2.1)** replaces the historical **Beta 3** label; its canonical branch is **beta-final/v2.1.0**.
- **v2.0.0 (Beta 1 / Versão Beta Final 2.0)** is the separate, canonical unified Beta 1 + former Beta 2 line; see its own CHANGELOG.md.
- **v1.0.0 (historical initial release):** desktop Splitter package opening, resource selection and export, supported previews, and early portable/release workflows.
- The source contains GitHub workflows covering supported Windows, Linux, and macOS target architectures. Availability of a workflow is not a promise of successful builds on all platforms.
- Older build/v2.1.0 and build/v2.2.0 branches remain for historical reference. Future 2.1 improvements should start at **beta-final/v2.1.0** and remain isolated until reviewed/tested.

### Documentation policy

- The branch README.txt describes capabilities and testing context; BETA_FINAL.md records canonical branch names.
- Add each accepted change to this changelog with one of these statuses: **Added**, **Improved**, **Fixed**, **Known issue**, or **Experimental / pending**.
- Credit community members for suggestions, reports, or testing where identified by a source. Record the bug report separately from its eventual verified fix. Do not backdate undated historical changes.
