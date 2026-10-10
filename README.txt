Veiga's S3CC Splitter v2.1.0

What's new in v2.1.0

Remove from Merged
- Expanded Remove from Merged support.
- Added removal planning based on the original packages represented inside a merged package.
- Added X/X confirmation showing how many original contents will actually be removed.
- Shared resources still required by remaining content are preserved.
- Selected resources that must remain because of dependencies are protected.
- Veiga Merge Manifest entries are updated after removal.

Generic Merged Package Support
- Added safer handling of merged packages without a valid Veiga Merge Manifest.
- Invalid or unsupported manifest data can fall back to the generic merged-package workflow where appropriate.
- Added dependency-aware removal for supported non-manifest merged packages.
- Added bit-for-bit backup creation before destructive merged-package operations.

Duplicate Detection
- Redesigned duplicate detection.
- Added distinction between Duplicate, TGI Conflict and Same Name.
- Matching filenames alone no longer classify two packages as duplicates.
- Shared dependencies no longer create false duplicate results.
- Real same-TGI/different-data conflicts continue to be rejected.

Search and Selection
- Added Instance ID search.
- Improved filtered selection behavior.
- Improved Shift + click range handling.
- Improved selection persistence across filtering and search.
- Preserved zero preselection when loading new content.

Merger and Unmerge
- Improved original filename recovery.
- Improved metadata preservation.
- Improved shared-resource reconstruction.
- Improved handling of output-name collisions.
- Improved reversible merged-package compatibility.

Interface
- Improved Splitter / Merger controls.
- Improved Revert Merge and Remove from Merged presentation.
- Improved singular/plural messages in English, Portuguese and Spanish.
- Improved Merger action-button alignment.
- Aligned Merger scrollbar behavior with Splitter.
- Improved sidebar, footer and responsive layout.
- Added localized Clear List text.

Build and Workflow
- Expanded portable builds for Windows x64, x86 and ARM64.
- Expanded portable builds for Linux x64, x86 and ARM64.
- Added macOS Intel and Apple Silicon portable builds.
- Improved artifact validation and workflow checks.
- Portable packages include project documentation and license files.

Tests
- Added duplicate-relation regression tests.
- Expanded Remove from Merged coverage.
- Expanded modal, filter and selection tests.
- Expanded reversible merge regression coverage.
