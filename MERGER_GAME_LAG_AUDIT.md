# Merger / The Sims 3 game-lag compatibility audit

Target: **Beta 2 — Versão Beta Final 2.1**, branch `beta-final/v2.1.0`.
Development fix branch: `fix/beta-final-v2.1-merger-game-compat-20261022`.

## User report

GreatDaySimmer reported severe gameplay lag with custom content merged by this tool. After unmerging with Splitter and re-merging the same resources with s3pe, the game ran normally. Some catalog items then appeared with missing names or source package filenames; attribution of the name issue is not established. Treat this as a credible regression report **not yet a controlled s3pe equivalence proof**.

## Verified implementation risks

1. Former Merger stored its private `<VeigaS3CCMergeManifest>` as a resource with TypeID `0x0333406C`, which is a real The Sims 3 XML resource type. This could cause unwanted interaction with game resource consumers. The new experimental output uses opaque TypeID `0x5653474D` (VSGM); readers still recognize **legacy** `0x0333406C` when both group `0x56534733` and instance `0x56454947414D4E46` match.
2. Former Merger wrote package index entries in source iteration order, not TGI order. The experimental build sorts *only the index entries* by (Type, Group, Instance) after writing payloads. Chunk offsets and resource bytes remain unchanged. **No claim is made that index sorting itself solves the game lag**.
3. The merge output intentionally excludes source NMAP and original package-manifest metadata from playable resources, retaining them for Unmerge. CLIP-related NMAP is consolidated. This choice needs comparison against a tested s3pe output and must **not** be changed blindly, especially if catalog names are affected.
4. The new manifest is still embedded in the DBPF archive; a truly s3pe-identical output might require storing reversibility metadata externally. This is a candidate future A/B experiment, not part of the current fix.

## Required validation before merge to canonical branch

- `cd veiga-s3cc-splitter && npm ci && node --test && npm run build`
- `cargo check --locked --workspace && cargo test --locked --workspace`
- Run WinUI visual smoke and Windows x64 portable workflow.
- Verify **legacy** manifests still permit Unmerge and Remove from Merged, and new manifests round-trip.
- Compare each non-metadata TGI and decoded/raw bytes for all sources vs output. Identify any conflict and fail rather than silently dropping divergent data.
- Specifically compare `STBL` (0x220557DA), `OBJD`, `OBJK`, `CASP`, `NMAP`, and XML types and any referenced instances.
- Test a representative mixed set of CAS and Build/Buy CC plus large (>1 GiB) merged-package regression.
- For controlled gameplay A/B test, merge the **same originals** once with s3pe and once with the experimental Merger. Keep only ONE merged package version installed at a time; do not install loose originals alongside it. Control for cache changes and use the same save/lot, graphics options, game patch and mod load order. Record startup/load time, stuttering, FPS/1% lows, catalog entries and name labels.
- Repeat the comparison if the first test suggests a meaningful difference. Do not assert that performance was fixed based solely on a successful DBPF parse, a passing CI job, or anecdotal results from different setups.

## Rollback

The canonical branch remains unchanged while this fix is under test. Git history preserves prior builds. Avoid writing over the user's CC collection. Always generate merged output as a new file and keep original packages for recovery.
