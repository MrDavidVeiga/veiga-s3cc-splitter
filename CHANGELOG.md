# Changelog — Veiga's S3CC Splitter

## Beta 1 — Versão Beta Final 2.0 (v2.0.0)

**Canonical development branch:** beta-final/v2.0.0  
**Lineage:** former Beta 1 (Splitter v2.0.0) consolidated with former Beta 2 (v2.1.0), reidentified as Beta 1 v2.0.0.  
**Documentation consolidated:** 2026-10-22 (this is NOT an invented release date).

This retrospective changelog describes the capabilities in the canonical branch, distinguishing functionality, improvements, corrections, limitations, and credited user feedback. Individual development dates and the authorship of uncited features are not inferred. “Beta Final” names the current development line; it does not certify a fully tested public release.

### Improved — Manager WinUI 3 layout parity (2026-10-22)

- **Manager header consistency (2026-10-22):** renamed the localized Splitter welcome heading to **Packages para Separar** (PT/ES; **Packages to Split** in EN) and matched the Merger heading's primary text color and weight. Recentered the Theme icon/label and language control in the topbar; aligned their 16px/14px geometry and the 12px regular EN/ES/PT language menu with the latest Manager. The selected language option is hidden from the flyout and exposed through the trigger. PR #21.


- Unified the **language / EN-PT-ES abbreviation / theme** topbar geometry with the latest S3CC Manager reference (`MrDavidVeiga/S3CC-Manager`, `fix/canonical-category-folders-20261019`), including spacing, 14 px labels, Fluent control heights and 12/16 px icons.
- Matched the Manager **content filter ComboBox and name/Instance ID SearchBox** typography, colors, dimensions, dropdown radius and focus styling.
- Reused the Manager's **results list shell** (`manager-list-card` and `package-list` classes) with 18 px outer radius and 28 px fixed list-header spacing, without replacing the Splitter's resource virtualizer.
- Restored the Manager's **WinUI 3 native Fluent scrollbar** (12 px stable channel; 4 px resting thumb inset, 2 px on hover, 1 px when pressed) for resources, Merger inputs, preview and filter dropdown.
- Matched the main **content panel (outer)** border, corner radius, fill and padding beside the sidebar.
- Restyled **info / warning / error / success / confirmation dialogs** with Manager-style state icons, localized text layout, themed surfaces and centered 120 px action buttons, retaining the Splitter's existing modal keyboard behavior.
- Added UI parity regression tests; frontend test/build validation passed on both branches, and the Beta Final 2.1 WinUI browser visual smoke passed after correcting the initial 140 px modal-button mismatch to 120 px.
- **Scope:** visual, HTML and CSS parity only. No change to resource merge/export engines or claims that Merger gameplay lag has been resolved. See PRs [#19](https://github.com/MrDavidVeiga/veiga-s3cc-splitter/pull/19) and [#20](https://github.com/MrDavidVeiga/veiga-s3cc-splitter/pull/20).

### Added — Splitter and Merger

- Unified **Splitter** and **Merger** modes in one desktop application. The former Beta 2's Merger was incorporated into the unified Beta 1; the version remains **2.0.0**, not 2.1.0.
- Open or drag supported The Sims 3 .package and .dbc files; scan and present recognized logical items and underlying resources.
- Filter the content list by supported categories; search by item name or **Instance ID**.
- Multiple selection, **Shift + click** interval selection, and **Select All / Select None** acting on the currently visible filtered set; a newly scanned list begins without preselected content.
- Resource detail and thumbnail preview; prefer the resource's available embedded thumbnail and retain the original llama as the missing-preview fallback.
- Export selected items into .package files with related resources when detected. Export options include thumbnails, compression, and clone behavior where applicable.
- Merge two or more supported source packages into a new output package; deduplicate genuinely identical shared Type/Group/Instance (TGI) resources.
- Reject conflicting resources when the same TGI has different decoded content instead of silently choosing one payload.
- Record a **Veiga Merge Manifest** for supported reversible merges, retaining original filenames, content ownership, anchors (such as CASP/OBJD), and relevant per-source metadata.
- **Unmerge / Revert** supported manifest-backed merges into named source packages.
- **Remove from Merged** both for manifest-backed and non-manifest packages. With a valid manifest, use recorded source ownership; without one, identify safely removable logical content and preserve detected shared dependencies.
- Differentiate **Duplicate**, **TGI Conflict**, and **Same Name** instead of treating repeated labels alone as duplicates.
- Use the confirmation ratio (X/X) according to the available evidence: original source packages when the manifest is valid, logical scanned items otherwise.

### Improved — Performance, usability, and visuals

- More efficient handling of large result sets, including virtualized/scrollable resource lists and reduced unnecessary list rendering.
- Export/large-library and memory-consumption work informed by community reports; do not infer fixed performance on all file sizes from code changes alone.
- Modernized WinUI 3-inspired interface with responsive pane layout, content controls, and styled dialogs.
- Language switching and localized labels in **English, Portuguese, and Spanish**.
- Theme selection including System, Veiga Light/Dark, Mean Girls, EA App, and WinUI Light/Dark; associated icons/colors follow the intended Manager visual parity.
- Harmonized **Idioma / Language** label and PT/EN/ES abbreviations with the Theme label size (**14 px**), keeping established font-weight differences.
- Centered Splitter processing messages, with no background card behind the status text or progress bar.
- Animated indeterminate loading marquee during long-running stages, without relying solely on backend milestone updates.
- Compact, centered action/dialog controls (notably Cancel, Remove, and OK), with **120 px** target widths.
- Centered the Merger's initial “no packages selected” state horizontally and vertically, matching the Splitter empty state; the normal results layout returns when inputs are present.
- Prioritize the genuine package-resource thumbnail in the preview, retaining the original llama graphic when a preview is not available.

### Fixed or guarded — Data safety

- Conflicting TGI payloads stop the merge rather than silently replacing one source.
- Removal from a non-manifest merged package includes **verified bit-for-bit backup** protection before replacing the loaded package.
- Manifest-aware removal preserves shared resources needed by retained packages.
- Duplicate classification no longer relies solely on matching filenames/names or common dependencies; separate cases remain visually distinguishable.
- Handling of original package labels for reversible operations was refined following tests of merged/unmerged names.

### Known issues and limitations (not claims of a fix)

- **Merger game performance — OPEN:** a community tester reported serious in-game lag from a package merged with this tool compared with s3pe. Which historical/current beta is affected, and the exact cause, have not yet been established by a controlled same-input game test. Treat Merger output as experimental for large CC libraries.
- **Build/Buy names:** a tester observed missing catalog names or source filenames in some items after a Splitter + s3pe workflow; it is not established whether the tool, original CC metadata, or the test workflow caused it.
- Unmerge requires a valid Veiga Merge Manifest. **Remove from Merged** without such a manifest cannot promise lossless reconstruction of the original component packages.
- A valid DBPF parse or an automated build does **not** prove gameplay stability. Retain original CC packages, keep backups, and test new merges separately.
- Newer **PoseList/CLIP-specialized support** and external The Sims 3 thumbnail-cache fallback are documented in Beta Final **2.1**, not asserted as features of this branch.

### Community contributions — documented attribution

- **Jacinda:** suggested interval selection by **Shift + click** and reported a large Select All/export crash with hundreds of CC items. This feedback informed selection and large-library work.
- **GreatDaySimmer (greatdaysimmer):** shared feedback on restoring source package names after merge/unmerge and test cases where distinct CC content had been misidentified as duplicates; also contributed real-world Merger/Unmerge testing.
- **quietlyshout:** reported excessive RAM during large exports, duplicate-filename export problems, and suggested a more compact/resizable interface and clearer duplicate display.
- **starshiphelix:** suggested search by **Instance ID**.
- **Kitilith / sims3-rs:** credited for the underlying Sims 3 DBPF/package-reading work (upstream technical foundation, not a community feature suggestion).

These credits refer to the **specific ideas, reports, or testing** above, not to sole implementation authorship. Other changes are recorded without invented attribution. **GreatDaySimmer also suggested support for poses**, which is credited under Beta Final 2.1 where that functionality is documented.

### Build and historical context

- The project includes portable/release workflows for Windows, Linux, and macOS architectures; platform workflows are not a guarantee that all jobs currently pass.
- **v1.0.0 (original stable baseline):** package opening and inspection, resource selection and export, supported previews, original Splitter-only layout, and initial desktop/portable build workflows. See the historical build/v1.0.0 CHANGELOG.md.
- **Beta Final 2.0** is the canonical v2.0.0 working branch. Older build/v2.0.0 and build/v2.1.0 branches are retained as history, not as the current development source.

### Sources and maintenance

- Branch README.txt documents the existing behavior and community credits.
- BETA_FINAL.md identifies the canonical branches.
- Update this changelog whenever a change is **merged into this branch**, and distinguish **implemented**, **tested**, **experimental**, and **reported/unresolved**. Do not copy pending patches from another branch into “Fixed” prematurely.
