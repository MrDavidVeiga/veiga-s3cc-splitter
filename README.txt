Veiga's S3CC Splitter v2.0.0

What's new in v2.0.0

Merger
- Added Merger mode alongside Splitter.
- Added support for combining compatible .package files into a merged package.
- Added the Veiga Merge Manifest for reversible merges.
- Added resource ownership tracking inside merged packages.
- Added preservation of original package names where available.
- Added protection against conflicting resources that share the same TGI but contain different data.

Revert Merge
- Added Revert Merge for packages containing a valid Veiga Merge Manifest.
- Original packages can be reconstructed from manifest ownership information.
- Shared resources required by multiple original packages are preserved correctly.
- Original package structure and metadata required by the reversible workflow are retained.

Selection
- Added multiple selection.
- Added Shift + click range selection.
- Added Select All and Select None.
- Select All / Select None operate only on the currently visible filtered/search results.
- Selection outside the current filter remains preserved.
- New scans start with zero selected content.

Filters and Search
- Added content filtering.
- Added search for loaded resources.
- Filters and search can be combined without breaking selection.

Preview
- Added and refined preview support for compatible content.
- Preview can follow the focused item without automatically selecting it.

Interface
- Added Splitter / Merger mode switching.
- Added themed application dialogs.
- Added internal confirmation, information, warning, success and error dialogs.
- Added English, Portuguese and Spanish interface support.
- Improved responsive layout and content-list interaction.

Reliability
- Added validation for reversible merge data.
- Added safer handling of shared resources.
- Added regression coverage for selection, filtering and merge operations.
