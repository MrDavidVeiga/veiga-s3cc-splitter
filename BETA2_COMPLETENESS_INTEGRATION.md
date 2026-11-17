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


## Additional performance and safety hardening

- **Untrusted CC metadata:** resource names/details are rendered using DOM `textContent` and text nodes, not `innerHTML`. The duplicated names, original titles and Unicode characters remain visible literally.
- **Virtualized list:** avoids re-creating the rendered window when scrolling within the same buffered range; selection membership uses a cached Set while preserving array order for export and Shift-select.
- **Full duplicate/TGI verification on demand:** above the initial 8,000-item fast-load budget, the user can explicitly verify the full set of logically grouped resources via the results-header button. The list remains interactive. The backend performs the original exact TGI/hash comparisons in a blocking worker and emits periodic progress/cancellation checkpoints. This check is **read-only**, and the initial skipped status must never be described as verified.
- **Cancellation isolation:** duplicate verification uses a separate generation and cancellation flag from main scanning and Merger tasks. File changes/new scans invalidate old results and work. The UI waits for cancellation acknowledgment before allowing another verification.
- **Preview coalescing:** a short 45-ms delay discards rapidly superseded selection requests before image decoding, with an additional generation check on queued Rust workers. This does not interrupt image decode already in progress.
- **Artifact quota:** the prior Windows x64 build compiled successfully but upload failed because the GitHub Actions artifact-storage quota was exceeded. Automated PR pushes now run the test matrix without uploading a new executable each time. The optional Windows x64 artifact job is manual (`workflow_dispatch`) with three-day retention and higher compression. **The account storage quota must be freed before a manual artifact can be uploaded**; this is not a code-build defect and this branch cannot delete account artifacts through available permissions.

### Recommended repeatable benchmark

On a *copy* of the input packages, record: package bytes, logical resource count, scanned-list time, peak process memory, time to first preview, rapid-selection responsiveness, time to cancel optional relation verification, merge/unmerge wall time and resulting SHA-256/TGI/decoded-hash differences. Repeat on the same hardware for Beta 1 v2.0.0, Beta 2 before integration, and this PR head. Avoid attributing changes in in-game lag to DBPF byte-size differences alone; use the same The Sims 3 save, CC set and cache protocol.

### Acceptance gates

1. GitHub Actions JS, Linux Rust and Windows Rust tests must all pass on the *same final head SHA*.
2. Manual large-package and cancellation tests must show no wrong list/preview, lost original package, or false `no duplicates` status.
3. Real CC round-trip and in-game A/B validation remain mandatory for release; synthetic tests alone cannot certify gameplay correctness.
4. The downloadable Windows x64 artifact is not confirmed until `actions/upload-artifact` succeeds and the SHA-256 file is accessible. The other seven targets require separate builds.
