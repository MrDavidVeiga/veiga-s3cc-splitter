# Beta 2 completeness integration — Beta 1 v2.0.0 parity

**Status:** experimental isolated integration, not a release or an official-beta replacement.
**Date:** 2026-11-16.
**Base:** `fix/beta2-skin-resource-name-recovery-20261102` (PR #33, built on PR #32).
**Compared Beta 1:** `beta-final/v2.0.0` (including merged PR #34).
**Working branch:** `integration/beta2-complete-beta1-parity-20261116`.

## Precisely what was integrated from Beta 1

1. **Nonblocking first preview:** resource list is rendered as soon as scanning returns. `get_resource_preview_async` runs the *existing* Beta 2 cached/PoseList-aware preview on a blocking worker. Preview decoding no longer keeps the scan overlay displayed.
2. **Stale requests:** preview requests carry a frontend request token and a source-path snapshot. A newer selection, a new scan, clearing the list or selecting a new source invalidates outstanding previews.
3. **Scan cancellation:** canceled or superseded scans do not replace the current resource list and should not show the usual read-error dialog. Beta 2's `cancel_package_operation` remains the cancellation endpoint.
4. **Optional duplicate-analysis budget:** above 8,000 items, optional hashing is skipped to keep the list responsive (Beta 1 behavior); the UI explicitly reports that this is **not** a clean duplicate report. For large, non-manifest packages at or below the item budget, Beta 2's deferred relation analysis is retained. Large manifest-backed scans also skip optional relation hashing.
5. **PT/EN/ES progress:** scan stages and cancellation feedback are translated on the frontend, preserving Beta 2's existing backend event identifiers and callbacks.
6. **Manager geometry:** Beta 1's social footer anchoring and language-selector spacing is retained in the Beta 2 visual layer.

## Beta 2 features deliberately retained

- CLIP/PoseList detection, consolidation and source-preserving Unmerge.
- TONE v4/v6 parsing, IMG/ramp dependencies, IMG-only skin detection and source-name recovery.
- Trusted complete-TGI-based skin category classification and conservative `Unverified` semantics.
- Per-source ICON/THUM ownership, reversible VSGM/legacy manifest support.
- Safe output overwrite with permanent `.s3ccbak` backup/rollback and input protection.
- Streamed DBPF stored-byte copy, index bounds, TGI conflicts with decoded-hash evidence.
- External thumbnail-cache fallback, in-memory package preview cache.
- Deferred relation analysis (when within budget), original themes, filters, selection and Merger actions.

## Validation performed / required

**Automated verification:** `.github/workflows/beta2-complete-parity-validation.yml` runs Node tests and frontend bundle, Linux Rust workspace/lib/comparator tests, Windows Rust workspace/comparator/TONE/Tauri tests with the proven Common Controls v6 executable manifest. The workflow must pass at the exact **final** head SHA before promoting.

**Manual source and UI checks still required:**
- Open an actual large merged file (notably the earlier `FullBuild_p04.package` testcase), cancel during names/relations, reopen, and confirm responsive UI/preview.
- Verify PT/EN/ES stage progress and cancellation without an error popup; rapidly select different resources and switch files while previews are loading.
- For >8,000 resources, verify the list opens and the UI states that duplicate/conflict checks were **not completed**; for moderate large packages, verify later relation badges appear when analysis finishes.
- Merge → export → remove five inputs → unmerge on **copies**, preserve source SHA-256 and all required resource TGIs/decoded hashes.
- Repeat GreatDaySimmer's 19-skin and three-object fixtures, IMG-only/default-classification edge cases, ICON/THUM ownership, and a mixed CLIP/PoseList merge/restoration.
- Compare a fresh merged output against the same inputs merged by s3pe using the read-only `sims3-pack/examples/compare_merges.rs` utility. Investigate `different_decoded`, missing keys or conflicting TGIs.
- In-game The Sims 3 controlled A/B tests, identical CC set, same save, cache protocol and settings; catalog names, thumbnails, poses, skin appearance, stutter/frametimes must be reviewed.
- Verify all eight distribution targets before labeling an official Beta 2 release; this branch's CI is **not** an eight-target release matrix.

## Release blockers / non-claims

- No claim that gameplay lag or catalog-name issues have been fixed until in-game A/B confirms it.
- Classification as Default/Non-default needs a trusted EA full-TGI baseline; missing evidence remains `Unverified`.
- Historical Beta 2 version identifiers differ across branches; this branch keeps its selected experimental base's v2.1.0 identity. Do not silently relabel production versions.
- The Windows test-suite entrypoint issue seen in PR #33 is a test-harness problem and is addressed in the separate workflow using a validated manifest embedding sequence. A successful CI run is required to confirm the new workflow works.
- The official Beta 1, official Beta 2, PR #32 and PR #33 are deliberately unchanged. No automatic merge/release.
