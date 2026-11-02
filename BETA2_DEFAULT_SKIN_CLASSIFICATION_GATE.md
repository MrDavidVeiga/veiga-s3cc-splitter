# Beta 2 v2.1.0 — Default vs Non-default Skintone Detection: release blocker

Status: **AUDIT / IMPLEMENTATION PENDING**. This document does not claim that a classifier or export fix has been implemented.

## Why this must be implemented before the official Beta 2 promotion

GreatDaySimmer reports that old skins exported from merged packages can contain “Default” in the filenames, including strings like `NONDEFAULT_Default` or `(ND)_default`. In the 12-package `Skintones TEST.zip` Stage 7 audit, all six names containing “Default” were already in raw NMAP/TONE metadata; no Splitter-invented word was found. The old report verified resource preservation but did **not** prove actual default/non-default semantics. The user is additionally supplying known-default skin package references to derive the resource-instance evidence.

Do **not** treat the mere presence of the word “Default”, a TONE that references the EA default color ramp, or the absence of a `0x2F7D0004` ramp PNG as proof of a default replacement. A non-default skin can reuse the EA ramp. Similarly, the existence of a TONE and NMAP does not itself settle the classification.

## Ground truth: classify per full resource key, never filename

The reference category must be determined from **the actual Type / Group / Instance** of resources relative to a trusted inventory of the **original EA Sims 3 base/expansion resources** for the game version(s) supported. In particular:
- A default-replacement **texture** typically overrides an EA texture by reusing the EA full IMG TGI (Type `0x00B2D882`, Group, Instance). It may have **no TONE** resource of its own.
- A default-replacement **tone/ramp** may override an EA TONE via the original EA full TONE TGI (Type `0x0354796A`, Group, Instance).
- A **non-default** TONE with a distinct full TGI remains non-default even when its references include the EA ramp, EA textures or the display/internal name `Default`.
- Some skin packages may combine original overrides and custom tones (MIXED). Others cannot be proven without an EA baseline (UNKNOWN).
- Matching only the 64-bit **instance** is NOT sufficient across resource Types/Groups; full TGI is mandatory.
- Seeing a matching TGI between two third-party skin packages does **not** prove that this TGI is an EA original. Shared mod resources and source-family variants can reuse TGIs. First verify the TGI against the vanilla EA reference.

## Required implementation

1. **Read-only baseline audit**: inspect the user's separately identified known-default ZIP and loose package (when accessible), recording for each exact TGI: Type, Group, Instance, decoded SHA-256 and whether it also exists in the trusted EA catalog. Extract candidates for a reviewed baseline. Keep the uploaded skin files unchanged.
2. **Classifier**: return one of `DEFAULT_REPLACEMENT`, `NON_DEFAULT`, `MIXED`, `UNKNOWN` for each logical skin grouping, plus provenance/evidence. A package can contain several logical groups. Only mark DEFINITE default when full-TGI EA comparisons support it. Report missing baseline as UNKNOWN rather than guess. Do not classify non-default merely because no matching EA TGI was observed if the baseline is incomplete.
3. **Recognition without TONE**: ensure standalone or merged IMG-only default replacements appear in the resource results, can be selected as an appropriately scoped logical group and exported with the exact same TGIs/payload hashes. Do not invent a TONE to enable UI scanning.
4. **Ownership**: when a trusted Veiga Merge Manifest proves the source, preserve the exact original filename and resource membership. For a legacy s3pe merge without provenance, use a neutral, collision-resistant filename based on the canonical resource key(s), not a guessed Default/NonDefault suffix. A default replacement comprised of many IMG TGIs must not be split arbitrarily into wrong catalog variants if ownership cannot be recovered; an UNKNOWN group should remain safely extractable as a complete copy rather than silently losing resources.
5. **Dependency integrity**: preserve TONE v4/v6 → IMG and ramp PNG links, exact Type/Group/Instance sets, byte-identical decoded payloads, package-owned metadata as appropriate, and shared dependencies. Do not borrow or mix IMG textures between unrelated skins just because their names or ramps match.
6. **Display**: if and only if proven, show “Default replacement” or “Non-default” in the UI details/badge; show “Unverified” when unknown; **keep a raw source label** separate from the verified classification. Never rename the underlying `*.package` just to make the label prettier.
7. **Tests**: synthetic known-EA-TGI IMG-only default, EA-TONE replacement, custom TONE referencing the EA ramp but non-default, conflicting `NONDEFAULT_Default` internal name, matching 64-bit instance with mismatched Type/Group, MIXED package, UNKNOWN baseline, duplicates across separate packages, legacy nonmanifest merge, manifest round-trip. For each: assert exact reconstructed TGIs and decoded SHA-256, original bytes and names left unchanged. Also rerun all Stage 5/6/7 regression cases.

## Important data-status distinction

The already inspected `Skintones TEST.zip` (12 packages, Stage 7) was a **name/dependency audit**, NOT a verified EA baseline. Do not reinterpret its six `Default` filenames or their TONE instance IDs as official EA TGI IDs. The newly described reference ZIP and loose package are not present among the accessible conversation attachments as of this review; do not claim they have been parsed.

## Release gate

The official `beta-final/v2.1.0` should not be promoted/overwritten before the baseline audit, implementation with regression tests, CI (including Windows x64), and a read-only real-CC round-trip. Maintain `beta-final/v2.0.0` unchanged. The PR #32 work was based on a newer experimental branch that also contains a historical `2.0.0 | Beta 2` identity and must be restored to the canonical `2.1.0 | Beta 2` naming before officialization.

References:
- Official beta 2 branch: `beta-final/v2.1.0`.
- Experimental fixes: `integration/beta2-greatdaysimmer-fixes-20261025`.
- Past evidence: Stage 7 `STAGE7_SKINTONES_NAME_AUDIT.md`.
