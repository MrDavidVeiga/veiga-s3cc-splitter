# GreatDaySimmer — Beta 2 integrated validation

Status: experimental, **not merged into official betas**. This file documents technical work completed before asking the user for in-game validation.

## Changes already included

- Confirmed overwrite: explicit consent, unique permanent `.s3ccbak` backup, rollback and original input protection.
- TONE v4/v6: parse true Type/Group/Instance dependencies, preserve present `_IMG` resources, avoid TONE-only silent exports and include swatch PNGs associated with any present referenced image.
- Object export: use the per-source reversible manifest to select the correct `ICON`, never borrow an unrelated item's icon.
- Full-source manual clone: restore original `KEY` and `Package Manifest` only when selected active resources match exactly one source, and never restore global metadata to a partial selection.
- TGI conflict: compare decoded contents, report both file paths, exact Type/Group/Instance and sizes, and cancel rather than silently select a winner.
- Merged DBPF: private opaque VSGM manifest with recognition of legacy XML manifests; deterministic TGI-sorted index with original resource payload offsets preserved.

## Confirmed test coverage

- Windows Rust full suite, TONE reader tests, JS frontend regressions.
- Full/partial source metadata restoration regressions.
- Ramps with a nonfirst image TGI.
- Separate object ICONs, including legacy manifests.
- Safe overwrite rollback and TGI conflict detection.
- Windows x64 experimental binary built as **Actions artifact**, no publication/release.

## What the provided CCs proved (private data, do not redistribute)

- Original non-default skins retained 4 and 10 _IMG dependencies; old exports contained only TONE.
- Native Body Wash had 52 active resources in the source's Veiga manifest. The old export retained 51 source active TGIs and lacked only ICON; its original Package Manifest was preserved in the source's VSGM manifest.
- SuteFlower glasses and SonyaSims/RolloRolls hair variants have genuine divergent THUM/_IMG data under identical TGIs; neither winner may be silently chosen.
- The overwrite scenario is saving a second merge to the same preexisting filename after a native Save confirmation.

## New independent corrections — 2026-10-25

- TONE selection no longer groups unrelated skins by similar/identical display names such as "Default". In s3pe merges without a trusted Veiga manifest, it includes only the exact selected TONE and its Type/Group/Instance dependencies. When a Veiga manifest uniquely establishes ownership, multiple TONEs from that same source can still be exported together.
- For skin clones, preserve the original package filename (including NonDefault/Default words verbatim) only when one unique original source is identified by the Veiga manifest; never infer default status or strip substrings.
- Synthetic regressions now cover two independent skins with the same UI name, multi-TONE ownership within a single original source, and source filename preservation.
- Experimental build/test workflows cancel outdated runs when newer commits are pushed. The official Beta 2 remains unchanged.
- The two private large skin merges have known Drive sizes: s3pe **463,872,826 bytes** and Splitter **460,407,376 bytes**. This particular pair is **3,465,450 bytes smaller with Splitter**, so it must NOT be presented as proof of the separately reported ~80 MB larger merged-output case. File sizes are not sufficient to judge resource correctness or game lag.
- Both files were materialized through authenticated Drive/Library access, but this execution environment did not support processing their complete binary contents. No resource-by-resource conclusion is recorded for these two full merges.

## Merge streaming and private-file comparator

- The Merger now copies stored compressed resource bytes using bounded streaming I/O and persistent per-source read handles. It does not decompress/recompress assets during the copy stage; the source index's `compressed`, `mem_size`, and `unknown2` metadata are carried forward verbatim.
- Prevent 32-bit DBPF resource/index-offset overflow before publishing a merged output.
- Incomplete merges use `.s3cctmp` instead of `.package` and are removed by a scope guard on error; recursive CC discovery will not import them.
- Tests cover exact stored-byte copying, truncated-input failure, 32-bit overflow, and exclusion of abandoned merge temporaries.
- Read-only audit utility: `sims3-pack/examples/compare_merges.rs`. It compares every Type/Group/Instance and emits `only_s3pe`, `only_splitter`, `same_stored`, `same_decoded_diff_storage`, `same_decoded_metadata_diff`, `different_decoded`, `duplicate_tgi`, or error classifications. This separates real resource differences from mere compression/metadata differences. Never uploads or rewrites source CCs, and refuses to overwrite an existing report.
- Future private local invocation (when the files are available to the execution environment):
  `cargo run --release -p sims3-pack --example compare_merges -- "s3pe-merged.package" "splitter-merged.package" "comparison.csv"`
- **Not yet run on the ~460 MB user files:** the local execution/container service was unavailable, and CI runners do not have access to the user's private Drive binaries. Any claim that the two large merges are byte-identical or that in-game lag is fixed would be premature.

## Independent restore, disk-space, and duplicate-scan hardening

- Duplicate detection during Unmerge now indexes Type/Group/Instance keys first and compares the decoded contents one TGI at a time. This avoids storing an entire 460 MB library of decoded assets from each candidate simultaneously; duplicate TGIs are compared as multisets only within their own group.
- Merge, source-restore, and selected-removal temporary paths are now reserved via exclusive file creation (`create_new`); a failed reservation cannot delete or truncate somebody else's file. The same scope-guard cleanup only follows ownership of a successfully reserved temporary. The Remove worker transfers responsibility to its caller after writing completes.
- The generic "Remove selected" reconstruction uses a fresh non-package temporary filename (`.s3cctmp`), cannot delete or overwrite a pre-existing temporary, and cleans up an unfinished output on error. Reversible-source Unmerge also uses protected `.s3cctmp` files.
- Multi-package Unmerge tracks the finalized files created by that invocation and removes only those new outputs on ordinary runtime failure. Pre-existing packages are never included in this rollback.
- Windows preflight checks available destination-disk space before Merge, Remove, or Restore, accounting for temporary output(s), backup(s), and a safety allowance. If free space is below the estimate, the operation is blocked **before modifying originals**. If less than 5 GiB remain but the estimate is satisfied, the success dialog shows an EN/PT/ES warning (with guidance that large jobs can need roughly 2.1 GiB extra).
- Regression coverage includes decoded per-TGI comparison regardless of DBPF index order, refusal to overwrite unrelated temporary files, 5 GiB warning / insufficient-space decisions, and cleanup of partially restored outputs.
- **Limits:** disk-space calculation is a conservative preflight, not a reservation: other processes can consume space while a task is running. Power loss can interrupt multi-file restore between renames, so complete crash-atomicity is not promised. The low-disk success warning only appears if an operation completes.

## Remaining validation gates

1. **Real package round trip**: new Windows candidate exports from both s3pe and Veiga merges, and compares source/export Type/Group/Instance and decoded payload hashes. Do not treat packed-byte size differences as corruption by themselves.
2. **Large merged files**: 460 MB s3pe vs tool resources and DBPF index/compression/content comparison. Files downloaded privately, but container-side full comparison was not completed; no conclusion about game lag.
3. **Default vs non-default**: classify via actual source TONE metadata and original names, not by blindly stripping the substring `Default`.
4. **Controlled The Sims 3 A/B**: install only one merged variant at a time, remove duplicate originals, keep patch/version/save/lot/graphics configuration, clear caches consistently, compare catalog names, thumbnails, load times and stutter.
5. **UI and packaging smoke**: launch x64 candidate, merge/export/restore, confirm backup and rollback and that no output overwrites an input without user consent.

No game performance claim or final release approval is supported until these gates pass. Do not publish third-party CC files.