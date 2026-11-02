# Beta 2 — Codex acceptance audit on real The Sims 3 CC

**Scope:** Veiga's S3CC Splitter Beta 2 integration, PR #32. This is a LOCAL and **READ-ONLY** audit of third-party assets. Do not modify/recompress/delete/upload source CCs; do not publish them or add them to Git commits or Actions artifacts. Official Beta 2 is not a test destination.

## Operator / Codex requirements

- Windows x64; Rust/Cargo installed; adequate free disk and RAM.
- Use the existing local clone on the experimental branch `integration/beta2-greatdaysimmer-fixes-20261025`; never reset a dirty worktree, switch branch over uncommitted changes, or change the official beta.
- Obtain local file paths from the operator. **Never assume** that a previously downloaded private Drive file is present.
- Read the GitHub Actions regression results for the selected commit and record the SHA tested.
- No automatic installation into the active Sims 3 Mods folder. Do not delete originals or cache files without explicit operator approval.

## 1. Read-only comparison of large merged packages

Inputs (operator will supply local absolute paths):
- `S3PE_MERGED`: GreatDaySimmer non-default skins merged with s3pe; the earlier copy was 463,872,826 bytes.
- `SPLITTER_MERGED`: same collection merged with Veiga's tool; the earlier copy was 460,407,376 bytes.

The **3,465,450-byte difference is not proof of resource loss**. Validate actual source identities, multiplicities, metadata and decoded hashes, never file size alone.

From the experimental repository root, in PowerShell (fill in the operator's local paths):

```powershell
cargo test --locked -p sims3-pack --example compare_merges
if ($LASTEXITCODE -ne 0) { throw 'Comparator regression tests failed' }

# Existing real files supplied by the operator: DO NOT edit or move them.
$s3pe = 'C:\REPLACE_WITH_LOCAL_PATH\s3pe-merged.package'
$splitter = 'C:\REPLACE_WITH_LOCAL_PATH\splitter-merged.package'
$report = 'C:\REPLACE_WITH_OUTPUT_FOLDER\beta2-s3pe-vs-splitter.csv'

if (-not (Test-Path -LiteralPath $s3pe -PathType Leaf)) { throw 's3pe file missing' }
if (-not (Test-Path -LiteralPath $splitter -PathType Leaf)) { throw 'Splitter file missing' }
if (Test-Path -LiteralPath $report) { throw 'Output CSV exists; choose another name' }

Get-FileHash -LiteralPath $s3pe -Algorithm SHA256
Get-FileHash -LiteralPath $splitter -Algorithm SHA256
cargo run --release --locked -p sims3-pack --example compare_merges -- $s3pe $splitter $report
if ($LASTEXITCODE -ne 0) { throw 'Per-TGI audit failed' }
```

Read the CSV produced by `sims3-pack/examples/compare_merges.rs`, including `left_decoded_sha256`, `right_decoded_sha256`, `left_tgi_count` and `right_tgi_count`. The tool never rewrites its inputs and uses `create_new` to avoid overwriting an earlier CSV.

### Classification and required investigation

| Status | Meaning / action |
| --- | --- |
| `same_stored` | Same stored bytes and metadata. |
| `same_decoded_diff_storage` | Decoded contents equal; stored representation differs. NOT automatically corrupt. |
| `same_decoded_metadata_diff` | Decoded contents equal; inspect metadata differences. NOT automatically missing CC. |
| `different_decoded` | Real decoded payload difference; inspect Type/Group/Instance and original CC. |
| `only_s3pe` or `only_splitter` | Missing TGI on one side; check whether the resource is legitimate per-source metadata or required content. |
| `duplicate_tgi` | Repeated identity. Inspect all occurrences; a single hash is intentionally not supplied. |
| `decode_error`, `read_error` | Corrupt, unsupported, or unreadable resource; STOP before accepting that package. |

Specifically inspect:
- Skin TONE `0x0354796A` and _IMG `0x00B2D882` (exact TGI, not similar display names).
- Object OBJD, OBJK, VPXY, MODL, MLOD, GEOM, TXTC, IMG, ICON and THUM.
- Any repeated TGI under which s3pe may apply override semantics.
- Source filenames with `Default` or `NonDefault`: do not infer type from the word alone.

For each abnormal row, record the full TGI and both SHA-256 digests (when available), resource sizes, count, reason and next disposition. A strong SHA-256 match is evidence of equal decoded data, but **does not establish compatibility in-game**.

## 2. Sample individual unmerge/clone checks

Choose samples from each category:
1. Non-default skin originally reported as TONE/KEY-only; verify v4/v6 TONE dependencies and all present referenced _IMG TGIs and ramp PNGs.
2. Two independent skins that share the display name `Default`; verify they export independently.
3. Two separate catalog variants of an object (e.g., wall frame vs floor-leaning frame); neither must disappear as a presumed duplicate.
4. An object with source-owned ICON/THUM resources; verify only the correct object's preview resource(s) are exported.
5. Two recolors with the same TGI but different decoded _IMG bytes; Merger must REFUSE and identify both originals, rather than silently pick a winner.

Run comparison on original package vs reconstructed *full-source* package wherever the reversible manifest proves ownership. Some output metadata may be reserialized for partial legacy extraction; document justified differences instead of hiding them.

Before and after every potentially modifying test, hash the source files. Restore/Unmerge/Merge must operate on **copies** in a separate test directory with sufficient free space; inspect backup/rollback behavior and verify original source hashes match.

## 3. Controlled in-game A/B (requires a person with The Sims 3)

Do not infer game performance from DBPF size or synthetic tests. For each run:
- Same PC, game patch, save, lot, camera path, graphics settings and content collection.
- Only ONE version of the merged set active, with corresponding unmerged originals excluded to avoid duplicates.
- Use a consistent cache-reset procedure approved by the tester for every run.
- Record launch/load times, camera stutter/frametime observations, Build/Buy/CAS names, icon previews and whether content can be placed/used.
- Compare at least the s3pe merge and the current candidate, ideally repeat to reduce one-run variance.
- Do not claim the original lag report is fixed without a controlled result.

## 4. Reporting / acceptance

Record tested commit SHA, tester, date, local operating system, asset *filenames only* (no private uploads), per-status counts, per-TGI discrepancy examples, source file hashes, clone/restore verdicts and game A/B observations.

A Beta 2 release candidate can pass only after:
- Critical required resource loss is ruled out or corrected.
- No false duplicate removal for independent object variants.
- ICON/THUM only associated with their verified owners; missing original icons are NOT invented.
- Known conflicting same-TGI payloads are not silently chosen.
- Original files survive error/cancel/rollback unchanged.
- The reported severe in-game stutter does not recur under comparable conditions.
- Final Windows x64 build and full CI pass on the exact release commit.

**Do not release or merge PR #32 automatically.** If one condition fails, record a reproducible defect and keep the PR in draft.
