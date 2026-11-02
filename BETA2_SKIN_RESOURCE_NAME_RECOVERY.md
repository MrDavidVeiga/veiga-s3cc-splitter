# Beta 2 v2.1.0 — Recovery of readable skin names from resource metadata

Status: **experimental patch**, not merged into the official `beta-final/v2.1.0`.
This patch is built on the remote GreatDaySimmer integration branch at the Stage 8 specification commit. The separate local Stage 8 implementation must be reconciled before release.

## Naming priority

1. A unique, trusted Veiga Merge Manifest source filename takes precedence and is preserved verbatim.
2. For legacy s3pe exports without a manifest, recover a readable, resource-derived name from the selected skin's own TONE NMAP/KEY name. Normalize technical suffixes and remove a trailing `_custom_<number>` where the pre-existing reader already does this.
3. If the TONE's label is missing or a generic label (e.g. exactly `Default`), consider only local IMG resources referenced by that precise TONE's full TGI. Use a shared label if the referenced images agree on a meaningful name.
4. For a selected IMG-only skin group, derive an agreed shared resource name from the group’s actual IMG NMAP/KEY entries. Do not invent a TONE.
5. If no defensible readable name exists, generate a filename with the complete Type/Group/Instance identity as a neutral fallback.
6. If two selected skins have the same readable filename, preserve the first and append the second's complete TGI to disambiguate, not sequential “bulk” numbers.

The filename represents **name provenance** only. It is not a classification of the skin. The Stage 8 classifier's Default Replacement / Non-default / Mixed / Unverified result must stay separate. Thus a preserved raw label such as `CAPPUCINO_NONDEFAULT_Default` is not evidence of a default replacement. Do not strip arbitrary `Default`, `NONDEFAULT`, `(ND)`, or variant suffixes to manipulate classification.

## Regression coverage

- Stage 7 legacy NMAP-derived name `CAPPUCINO_NONDEFAULT_Default` is preserved as written.
- `Tainted_custom_0` recovers `Tainted`, matching existing display normalization.
- `[to]PamelaSkin_tone_2` remains distinct from variant 1.
- IMG-only default skin group with consistent `KBPrimerSkin_*_IMG` resource labels becomes `KBPrimerSkin.package` without adding a TONE.
- Missing or generic-only metadata gets the complete TGI fallback.
- Two distinct TONEs with the same recovered filename receive deterministic full-TGI disambiguation; manifest-owned names retain priority.

## Integration gate

The user's Stage 8 classifier changes were reported as implemented locally, but had not been pushed to GitHub when this patch branch was created. Reconcile this patch with the latest Stage 8 code, since Stage 8 originally switched legacy names to neutral full-TGI stems. Keep Stage 8's IMG-only groups, mixed/default/non-default detection, exact resource ownership, and verified release identity `Beta 2 v2.1.0`. Do not force-push or overwrite that local implementation.

Run Rust/Node CI, compare Stage 7 package outputs, validate source hashes and round-trip TGIs, and only then promote to `beta-final/v2.1.0`. The official Beta 1 `beta-final/v2.0.0` must remain unchanged.
