S3CC Splitter v2.4.0

What's new in v2.4.0

- Splitter and Merger modes are available in the same application.
- Results can be filtered and searched, with zero preselection, multiple selection, Shift + click, and Select All / Select None limited to the visible result set.
  - Shift + click range selection was suggested by Jacinda, who also reported a large Select All/export crash while working with hundreds of detected items.
- Preview, themed dialogs, and the responsive EN/PT/ES interface are integrated into the current workflow.
- Merger writes a Veiga Merge Manifest so supported merges can preserve original package names, CASP/OBJD anchors, and the resource ownership needed for reversible operations.
  - Improved original-name recovery was suggested through feedback from greatdaysimmer, based on cases where merged/unmerged CC could lose its original package naming.
- Unmerge can rebuild the original packages from a valid Veiga Merge Manifest.
- Remove from Merged is available for both manifest-backed merges and packages without a valid Veiga Merge Manifest.
- The removal confirmation uses the same X/X presentation in both cases. With a valid manifest, X/X refers to original packages recorded by the manifest. Without a valid manifest, X/X refers to selected logical items versus the logical items detected in the loaded package.
- Packages without a valid Veiga Merge Manifest use dependency-aware removal with shared-resource preservation when possible and require a verified bit-for-bit backup before replacement.
- Merger detects conflicting TGI entries with different data and cancels the merge instead of silently choosing one copy.
- Duplicate detection has been redesigned so matching names alone are no longer treated as proof that two CC items are duplicates.
  - This behavior was improved following real-world test cases and feedback from greatdaysimmer, where different CC items sharing names and common resources were being incorrectly presented as duplicates.
- The result list now distinguishes verified Duplicate content, TGI Conflict entries, and Same Name items.
- Verified duplicates are determined from logical content and related resources, while shared dependencies alone do not make two different CC items duplicates.
- Search by Instance ID is available to make it easier to locate specific CC resources.
  - Instance ID search was suggested by starshiphelix, especially for locating CC identified through external tools.
- Export and large-library handling received further improvements following community testing.
  - quietlyshout reported excessive RAM usage during large exports, duplicate-name export issues, and suggested improvements for a more compact/resizable interface and clearer duplicate handling.
- The portable build workflow covers Windows x64, Windows x86, Windows ARM64, Linux x64, Linux x86, Linux ARM64, macOS Intel, and macOS Apple Silicon.

Current main-branch additions (not yet assigned a new release version)

CLIP and Pose List support

- Ordinary CLIP resources appear in the Clip category by default.
- A CLIP is categorized as Pose only when a PoseList XML explicitly references its CLIP key and the matching CLIP resource is present. Unreferenced CLIPs in a mixed package remain Clip.
- A detected PoseList is presented as a logical Pose item, with its referenced CLIP resources, matching S3SA registrar, and associated icon image resources included when found.
- Package filenames and naming conventions are not used to infer that a resource is a Pose.
- When a merge contains multiple PoseList XMLs, Merger combines their unique `<Pose>` entries into one XML and retains one matching S3SA registrar. Empty template entries from additional lists are skipped, and repeated CLIP keys are deduplicated.
- The original PoseList XML and matching S3SA resources are stored as preserved resources in the Veiga Merge Manifest, allowing Unmerge to reconstruct the original source packages rather than restoring the combined XML into one of them.
- Remove from Merged also removes the consolidated XML and registrar when the combined Pose item is selected, while retaining shared resources still required by remaining content.
- The combined XML/registrar strategy was confirmed in-game with both experimental test packages: the Juniper-registrar variant (TEST_A) and the ROSELIPA-registrar variant (TEST_B). Both displayed all 18 poses and the poses worked in-game. The source includes a regression test for multi-PoseList merge and reversible Unmerge.

External thumbnail-cache fallback

- If an ordinary resource has no usable preview inside its package, Splitter checks the game caches under the actual Documents directory at `Electronic Arts/The Sims 3/Thumbnails`.
- It checks `CASThumbnails.package`, `ObjectThumbnails.package`, and `DownloadsThumbnails.package`, prioritizing the likely cache for the resource type and matching by Instance ID.
- A preview embedded in the loaded package takes priority. If no matching cache entry exists, the preview can remain unavailable.

What this tool does

Splitter mode

Open a .package or .dbc file, scan the supported logical content inside it, filter or search the results, preview supported items, select the content you want, and export the selection as rebuilt .package files with related resources when detected.

The resource list supports content categories, search by name or Instance ID, multiple selection, Shift + click, and Select All / Select None over the currently visible filtered/search result set. Nothing is preselected when a new scan starts.

When package name metadata is available, the Splitter uses it to improve labels and output names.

Duplicate detection

The Splitter distinguishes between three different situations:

- Duplicate: the logical content has been compared and verified as duplicate content.
- TGI Conflict: the same TGI appears with different resource data.
- Same Name: two items use the same internal or displayed name, but have not been identified as duplicate content.

A matching name by itself is never enough to classify content as Duplicate.

The duplicate detector considers the logical item and its detected related resources. Shared dependencies such as BONE, BGEO, or image resources do not by themselves make two otherwise different CC items duplicates.

Package naming metadata, Veiga Merge Manifest metadata, and thumbnails are not used as proof that two logical CC items are duplicates.

If an unusual item cannot be analyzed safely, duplicate analysis does not block the normal package scan.

Merger mode

Merger mode combines two or more supported package sources into a new merged package. The merger:

- deduplicates identical shared TGI resources;
- rejects a TGI conflict when the same key has different data;
- writes a Veiga Merge Manifest into the result;
- preserves the original package names and per-source metadata needed by the supported reversible workflow.

Merged package actions

Remove from Merged is available for supported manifest-backed merges and for packages without a valid Veiga Merge Manifest.

For a valid Veiga Merge Manifest, the tool uses the recorded package ownership and preserves resources still shared by content that remains in the merge. The X/X confirmation represents original packages recorded by the manifest.

Without a valid Veiga Merge Manifest, the same Remove from Merged action removes the selected logical content and associated resources that the Splitter can determine are safe to remove, while preserving detected shared dependencies. In this case, the X/X confirmation represents selected logical items versus the logical items detected in the loaded package. Because original package ownership is unavailable, this removal is not automatically reversible. A bit-for-bit backup of the original is created and verified before replacement.

Unmerge is the only merged-package action that remains exclusive to packages with a valid Veiga Merge Manifest. It rebuilds the recorded original packages using their preserved names.

Important notes

- Unmerge requires a valid Veiga Merge Manifest.
- Remove from Merged is available with or without a Veiga Merge Manifest, but the source of the X/X count differs as described above.
- Without a valid manifest, the tool does not claim to know the original package ownership and cannot automatically reconstruct the original merged inputs.
- A verified bit-for-bit backup is required before non-manifest removal replaces the loaded package.
- Conflicting TGI data causes Merger to stop instead of producing an ambiguous result.
- Same Name does not mean Duplicate.
- Shared resources do not automatically mean that two logical CC items are duplicates.
- Keep backups when reorganizing important custom-content collections.
- The Splitter interface is available in English, Portuguese, and Spanish.

Cross-platform support

Validated application targets:

- Windows x64
- Windows x86
- Windows ARM64
- Linux x64
- Linux x86
- Linux ARM64
- macOS Intel
- macOS Apple Silicon

Credits and thanks

Kitilith / sims3-rs
https://github.com/kitlith/sims3-rs

The package-reading foundation and related Sims 3 DBPF work are credited to Kitilith under the license included with this repository.

Community feedback and testing

Special thanks to:

- Jacinda — Shift + click range-selection suggestion and large Select All/export crash report.
- quietlyshout — large-export RAM feedback, duplicate-name export issues, and interface/duplicate-handling suggestions.
- greatdaysimmer — merged/unmerged naming feedback, real-world Merger/Unmerge testing, and duplicate-detection test cases.
- starshiphelix — Instance ID search suggestion.

Thank you

Thank you to everyone who tests the tool, reports problems, and helps improve compatibility with real The Sims 3 custom-content collections.
