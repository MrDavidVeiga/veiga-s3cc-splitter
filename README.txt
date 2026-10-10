Veiga's S3CC Splitter v2.2.0

What's new in v2.2.0

CLIP and Pose Support
- Added native CLIP resource support.
- Added Pose List detection.
- Added logical Pose identification based on PoseList relationships.
- CLIPs referenced by PoseLists are identified as Pose content.
- Unrelated CLIPs remain available as regular Clip content.
- Added Pose preview support.
- Added handling for related XML, S3SA, CLIP and icon resources.

Reversible Pose Merging
- Added reversible merging for Pose content.
- Added support for combining multiple PoseLists into one compatible merged PoseList.
- Unique Pose entries are combined during merge.
- Duplicate CLIP references are avoided.
- Additional empty PoseList template entries are skipped.
- Original PoseList XML resources are preserved in the Veiga Merge Manifest.
- Original S3SA registrar resources are preserved for reconstruction.
- Unmerge restores the original PoseList packages and their original resources.

Remove from Merged
- Remove from Merged now handles resources derived from combined PoseLists.
- Pose resources required by remaining content are preserved.
- Remerging correctly restores sources represented through preserved manifest resources.

Preview
- Added external The Sims 3 thumbnail-cache fallback.
- Preview can use thumbnails from CASThumbnails.package, ObjectThumbnails.package and DownloadsThumbnails.package.
- Embedded package previews continue to take priority.
- Thumbnail lookup uses the corresponding resource Instance information.

Smart Unmerge
- Added duplicate detection across separate Unmerge operations using the same destination folder.
- Reconstructed packages are compared against existing packages by TGI and uncompressed resource payload.
- Exact duplicate content is not written a second time.
- Duplicate detection works even when filenames differ.
- Same filename with different content is never silently overwritten.
- Filename collisions with different content receive a unique name such as Package (2).package.
- Unreadable unrelated packages in the destination do not block Unmerge.
- Unmerge completion results report rebuilt packages and already-existing duplicates separately.

Reliability
- Improved preserved-resource reconstruction.
- Improved multi-PoseList handling.
- Improved remerge behavior.
- Improved detection of resources represented only through preserved manifest data.
- Improved output collision protection.

Tests
- Added CLIP/Pose identification regression tests.
- Added reversible Pose merge tests.
- Added multiple-PoseList merge and Unmerge tests.
- Added preserved PoseList/S3SA restoration tests.
- Added thumbnail-cache fallback tests.
- Added cross-Unmerge duplicate-detection tests.
