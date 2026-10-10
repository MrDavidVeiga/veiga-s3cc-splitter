# Changelog

## [2.1.0]

### Remove from Merged
- Expanded Remove from Merged support.
- Added removal planning based on the original packages represented inside a merged package.
- Added X/X confirmation showing how many original contents will actually be removed.
- Shared resources still required by remaining content are preserved.
- Selected resources that must remain because of dependencies are protected.
- Veiga Merge Manifest entries are updated after removal.

### Generic Merged Package Support
- Added safer handling of merged packages without a valid Veiga Merge Manifest.
- Invalid or unsupported manifest data can fall back to the generic merged-package workflow where appropriate.
- Added dependency-aware removal for supported non-manifest merged packages.
- Added bit-for-bit backup creation before destructive merged-package operations.

### Duplicate Detection
- Redesigned duplicate detection.
- Added distinction between Duplicate, TGI Conflict and Same Name.
- Matching filenames alone no longer classify two packages as duplicates.
- Shared dependencies no longer create false duplicate results.
- Real same-TGI/different-data conflicts continue to be rejected.

### Search and Selection
- Added Instance ID search.
- Improved filtered selection behavior.
- Improved Shift + click range handling.
- Improved selection persistence across filtering and search.
- Preserved zero preselection when loading new content.

### Merger and Unmerge
- Improved original filename recovery.
- Improved metadata preservation.
- Improved shared-resource reconstruction.
- Improved handling of output-name collisions.
- Improved reversible merged-package compatibility.

### Interface
- Improved Splitter / Merger controls.
- Improved Revert Merge and Remove from Merged presentation.
- Improved singular/plural messages in English, Portuguese and Spanish.
- Improved Merger action-button alignment.
- Aligned Merger scrollbar behavior with Splitter.
- Improved sidebar, footer and responsive layout.
- Added localized Clear List text.

### Build and Workflow
- Expanded portable build coverage to Windows x64, x86 and ARM64.
- Expanded portable build coverage to Linux x64, x86 and ARM64.
- Added macOS Intel and Apple Silicon portable builds.
- Improved portable artifact validation.
- Updated GitHub artifact actions.
- Added additional frontend and Rust workflow validation.
- Portable packages include project documentation and license files.

### Tests
- Added duplicate-relation regression tests.
- Expanded Remove from Merged coverage.
- Expanded modal, filter and selection tests.
- Expanded reversible merge regression coverage.

## [2.0.0]

### Merger
- Added Merger mode alongside Splitter.
- Added support for combining compatible .package files into a merged package.
- Added the Veiga Merge Manifest for reversible merges.
- Added resource ownership tracking inside merged packages.
- Added preservation of original package names where available.
- Added protection against conflicting resources that share the same TGI but contain different data.

### Revert Merge
- Added Revert Merge for packages containing a valid Veiga Merge Manifest.
- Original packages can be reconstructed from manifest ownership information.
- Shared resources required by multiple original packages are preserved correctly.
- Original package structure and metadata required by the reversible workflow are retained.

### Selection
- Added multiple selection.
- Added Shift + click range selection.
- Added Select All and Select None.
- Select All / Select None operate only on the currently visible filtered/search results.
- Selection outside the current filter remains preserved.
- New scans start with zero selected content.

### Filters and Search
- Added content filtering.
- Added search for loaded resources.
- Filters and search can be combined without breaking selection.

### Preview
- Added and refined preview support for compatible content.
- Preview can follow the focused item without automatically selecting it.

### Interface
- Added Splitter / Merger mode switching.
- Added themed application dialogs.
- Added internal confirmation, information, warning, success and error dialogs.
- Added English, Portuguese and Spanish interface support.
- Improved responsive layout and content-list interaction.

### Reliability
- Added validation for reversible merge data.
- Added safer handling of shared resources.
- Added regression coverage for selection, filtering and merge operations.

## [1.0.0]

### Features
- Initial release of Veiga's S3CC Splitter.
- Added support for opening and inspecting supported The Sims 3 package content.
- Added content selection and package export.
- Added resource preview for supported content.
- Added the original Splitter workflow for separating content into individual packages.

### Interface
- Added the initial desktop interface.
- Added package loading, content list, preview area and export controls.
- Added the original sidebar and application layout.

### Build
- Added initial Windows, Linux and macOS build workflows.
- Added the first portable and release workflows.
