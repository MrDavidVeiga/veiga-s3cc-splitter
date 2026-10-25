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

## Remaining validation gates

1. **Real package round trip**: new Windows candidate exports from both s3pe and Veiga merges, and compares source/export Type/Group/Instance and decoded payload hashes. Do not treat packed-byte size differences as corruption by themselves.
2. **Large merged files**: 460 MB s3pe vs tool resources and DBPF index/compression/content comparison. Files downloaded privately, but container-side full comparison was not completed; no conclusion about game lag.
3. **Default vs non-default**: classify via actual source TONE metadata and original names, not by blindly stripping the substring `Default`.
4. **Controlled The Sims 3 A/B**: install only one merged variant at a time, remove duplicate originals, keep patch/version/save/lot/graphics configuration, clear caches consistently, compare catalog names, thumbnails, load times and stutter.
5. **UI and packaging smoke**: launch x64 candidate, merge/export/restore, confirm backup and rollback and that no output overwrites an input without user consent.

No game performance claim or final release approval is supported until these gates pass. Do not publish third-party CC files.