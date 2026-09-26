S3CC Splitter v2.2.0

What's new in v2.2.0

- Splitter and Merger modes are available in the same application.
- Results can be filtered and searched, with zero preselection, multiple selection, Shift + click, and Select All / Select None limited to the visible result set.
- Preview, themed dialogs, and the responsive EN/PT/ES interface are integrated into the current workflow.
- Merger writes a Veiga Merge Manifest so supported merges can preserve original package names, CASP/OBJD anchors, and the resource ownership needed for reversible operations.
- Unmerge can rebuild the original packages from a valid Veiga Merge Manifest.
- Remove from Merged is available for both manifest-backed merges and packages without a valid Veiga Merge Manifest.
- The removal confirmation uses the same X/X presentation in both cases. With a valid manifest, X/X refers to original packages recorded by the manifest. Without a valid manifest, X/X refers to selected logical items versus the logical items detected in the loaded package.
- Packages without a valid Veiga Merge Manifest use dependency-aware removal with shared-resource preservation when possible and require a verified bit-for-bit backup before replacement.
- Merger detects conflicting TGI entries with different data and cancels the merge instead of silently choosing one copy.

What this tool does

Splitter mode

Open a .package or .dbc file, scan the supported logical content inside it, filter or search the results, preview supported items, select the content you want, and export the selection as rebuilt .package files with related resources when detected.

The resource list supports content categories, search by name or Instance ID, multiple selection, Shift + click, and Select All / Select None over the currently visible filtered/search result set. Nothing is preselected when a new scan starts.

When package name metadata is available, the Splitter uses it to improve labels and output names.

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
- Keep backups when reorganizing important custom-content collections.
- The Splitter interface is available in English, Portuguese, and Spanish.

Cross-platform support

Validated application targets:

- Windows
- Linux
- macOS Intel
- macOS Apple Silicon

Credits and thanks

Kitilith / sims3-rs
https://github.com/kitlith/sims3-rs

The package-reading foundation and related Sims 3 DBPF work are credited to Kitilith under the license included with this repository.

Thank you

Thank you to everyone who tests the tool, reports problems, and helps improve compatibility with real The Sims 3 custom-content collections.
