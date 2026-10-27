//! Read-only comparison of two The Sims 3 DBPF merged packages.
 //! cargo run --release -p sims3-pack --example compare_merges -- \
 //!   "s3pe.package" "splitter.package" "comparison.csv"
 //!
 //! No CC is copied, rewritten, uploaded or renamed. The CSV is created with
 //! create_new to avoid accidentally overwriting anything.
 use sims3_pack::dbpf::{Package, ResourceEntry};
 use sha2::{Digest, Sha256};
 use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
 use anyhow::Result;
 use std::fs::{File, OpenOptions};
 use std::io::{Read, Seek, SeekFrom, Write};
 use std::path::Path;

 type Key = (u32, u32, u64);

 fn index<'a>(package: &'a Package) -> (HashMap<Key, &'a ResourceEntry>, HashSet<Key>) {
     let mut by_key = HashMap::with_capacity(package.entries.len());
     let mut duplicated = HashSet::new();
     for entry in &package.entries {
         let key = (entry.type_id, entry.group, entry.instance);
         if by_key.insert(key, entry).is_some() {
             duplicated.insert(key);
         }
     }
     (by_key, duplicated)
 }

 // Compare bytes as stored, using a fixed buffer regardless of resource size.
 // Compression can differ across packers even if decoded content is identical.
 fn stored_equal(
     a: &mut File,
     ea: &ResourceEntry,
     b: &mut File,
     eb: &ResourceEntry,
 ) -> std::io::Result<bool> {
     if ea.file_size != eb.file_size {
         return Ok(false);
     }
     a.seek(SeekFrom::Start(ea.chunk_offset as u64))?;
     b.seek(SeekFrom::Start(eb.chunk_offset as u64))?;
     let mut left = [0u8; 64 * 1024];
     let mut right = [0u8; 64 * 1024];
     let mut remaining = ea.file_size as usize;
     while remaining > 0 {
         let size = remaining.min(left.len());
         a.read_exact(&mut left[..size])?;
         b.read_exact(&mut right[..size])?;
         if left[..size] != right[..size] {
             return Ok(false);
         }
         remaining -= size;
     }
     Ok(true)
 }

 fn compare_resource(
     left: &Package,
     a: &ResourceEntry,
     left_handle: &mut File,
     right: &Package,
     b: &ResourceEntry,
     right_handle: &mut File,
 ) -> &'static str {
     let same_metadata = a.file_size == b.file_size
         && a.mem_size == b.mem_size
         && a.compressed == b.compressed
         && a.unknown2 == b.unknown2;
     match stored_equal(left_handle, a, right_handle, b) {
         Ok(true) if same_metadata => return "same_stored",
         Ok(_) => {}
         Err(_) => return "read_error",
     }
     match (left.get_uncompressed_data(a), right.get_uncompressed_data(b)) {
         (Ok(x), Ok(y)) if x == y && same_metadata => "same_decoded_diff_storage",
         (Ok(x), Ok(y)) if x == y => "same_decoded_metadata_diff",
         (Ok(_), Ok(_)) => "different_decoded",
         _ => "decode_error",
     }
 }

 // Decode one resource at a time to avoid keeping both large texture
 // payloads in RAM. Hash the uncompressed resource, not stored RefPack bytes.
 fn decoded_sha256(package: &Package, entry: &ResourceEntry) -> Result<String> {
     Ok(format!("{:x}", Sha256::digest(package.get_uncompressed_data(entry)?)))
 }

 fn run(left_path: &Path, right_path: &Path, report_path: &Path) -> Result<()> {
     let left = Package::load(left_path)?;
     let right = Package::load(right_path)?;
     let left_bytes = std::fs::metadata(left_path)?.len();
     let right_bytes = std::fs::metadata(right_path)?.len();
     let (left_idx, left_duplicates) = index(&left);
     let (right_idx, right_duplicates) = index(&right);
     let mut left_counts = HashMap::<Key, usize>::new();
     let mut right_counts = HashMap::<Key, usize>::new();
     for entry in &left.entries {
         *left_counts.entry((entry.type_id, entry.group, entry.instance)).or_default() += 1;
     }
     for entry in &right.entries {
         *right_counts.entry((entry.type_id, entry.group, entry.instance)).or_default() += 1;
     }
     let keys: BTreeSet<Key> = left_idx.keys().chain(right_idx.keys()).copied().collect();
     let mut left_handle = File::open(left_path)?;
     let mut right_handle = File::open(right_path)?;
     let mut report = OpenOptions::new().write(true).create_new(true).open(report_path)?;
     writeln!(report, "status,type,group,instance,left_stored_size,right_stored_size,left_decoded_size,right_decoded_size,left_compressed,left_unknown2,right_compressed,right_unknown2,left_decoded_sha256,right_decoded_sha256,left_tgi_count,right_tgi_count")?;

     let mut counts = BTreeMap::<&str, usize>::new();
     for key in keys {
         let a = left_idx.get(&key).copied();
         let b = right_idx.get(&key).copied();
         let status = if left_duplicates.contains(&key) || right_duplicates.contains(&key) {
             "duplicate_tgi"
         } else {
             match (a, b) {
                 (Some(a), Some(b)) => compare_resource(
                     &left, a, &mut left_handle, &right, b, &mut right_handle
                 ),
                 (Some(_), None) => "only_s3pe",
                 (None, Some(_)) => "only_splitter",
                 (None, None) => unreachable!(),
             }
         };
         *counts.entry(status).or_default() += 1;
         // For repeated TGIs there is no unique resource to hash: leave both
         // fields blank rather than silently hashing only the last entry.
         let left_hash = if left_duplicates.contains(&key) {
             None
         } else {
             a.and_then(|entry| decoded_sha256(&left, entry).ok())
         };
         let right_hash = if right_duplicates.contains(&key) {
             None
         } else {
             b.and_then(|entry| decoded_sha256(&right, entry).ok())
         };
         writeln!(
             report,
             "{status},0x{:08X},0x{:08X},0x{:016X},{},{},{},{},{},{},{},{},{},{},{},{}",
             key.0, key.1, key.2,
             a.map(|e| e.file_size.to_string()).unwrap_or_default(),
             b.map(|e| e.file_size.to_string()).unwrap_or_default(),
             a.map(|e| e.mem_size.to_string()).unwrap_or_default(),
             b.map(|e| e.mem_size.to_string()).unwrap_or_default(),
             a.map(|e| e.compressed.to_string()).unwrap_or_default(),
             a.map(|e| e.unknown2.to_string()).unwrap_or_default(),
             b.map(|e| e.compressed.to_string()).unwrap_or_default(),
             b.map(|e| e.unknown2.to_string()).unwrap_or_default(),
             left_hash.unwrap_or_default(),
             right_hash.unwrap_or_default(),
             left_counts.get(&key).copied().unwrap_or(0),
             right_counts.get(&key).copied().unwrap_or(0),
         )?;
     }
     report.flush()?;
     println!("s3pe: {} bytes, {} indexed entries", left_bytes, left.entries.len());
     println!("Splitter: {} bytes, {} indexed entries", right_bytes, right.entries.len());
     println!("Difference (Splitter minus s3pe): {} bytes",
         right_bytes as i128 - left_bytes as i128);
     println!("Resource comparison:");
     for (status, count) in counts {
         println!("  {status}: {count}");
     }
     println!("Read-only CSV: {}", report_path.display());
     Ok(())
 }

 fn main() -> Result<()> {
     let args: Vec<_> = std::env::args_os().collect();
     if args.len() != 4 {
         eprintln!(
             "Usage: compare_merges <s3pe-merged.package> <splitter-merged.package> <NEW-report.csv>"
         );
         std::process::exit(2);
     }
     run(Path::new(&args[1]), Path::new(&args[2]), Path::new(&args[3]))
 }

 #[cfg(test)]
 mod tests {
     use super::*;
     use std::time::{SystemTime, UNIX_EPOCH};

     fn fixture(name: &str) -> std::path::PathBuf {
         let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
         std::env::temp_dir().join(format!("dbpf-compare-{}-{stamp}-{name}", std::process::id()))
     }
     fn entry(offset: u32, size: u32) -> ResourceEntry {
         ResourceEntry {
             type_id: 1, group: 2, id1: 0, id2: 3, instance: 3,
             chunk_offset: offset, file_size: size,
             mem_size: size, compressed: 0, unknown2: 0,
         }
     }
     fn sample_package(path: &Path, items: &[(u32, &[u8])]) {
         let mut package = Package::new(path, false).unwrap();
         let mut output = OpenOptions::new().read(true).write(true).open(path).unwrap();
         output.seek(SeekFrom::Start(package.index_position as u64)).unwrap();
         for (instance, bytes) in items {
             let chunk_offset = output.stream_position().unwrap() as u32;
             output.write_all(bytes).unwrap();
             package.entries.push(ResourceEntry {
                 type_id: 0x00B2D882, group: 0, id1: 0, id2: *instance,
                 instance: *instance as u64, chunk_offset,
                 file_size: bytes.len() as u32, mem_size: bytes.len() as u32,
                 compressed: 0, unknown2: 0,
             });
         }
         package.index_position = output.stream_position().unwrap() as u32;
         drop(output);
         package.write_index_and_header().unwrap();
     }

     #[test]
     fn comparator_reports_present_missing_and_truly_changed_resources() {
         let a = fixture("original.package");
         let b = fixture("splitter.package");
         let report = fixture("report.csv");
         sample_package(&a, &[(1, b"same"), (2, b"old"), (3, b"only-a")]);
         sample_package(&b, &[(1, b"same"), (2, b"new"), (4, b"only-b")]);
         run(&a, &b, &report).unwrap();
         let csv = std::fs::read_to_string(&report).unwrap();
         assert!(csv.contains("same_stored,0x00B2D882,0x00000000,0x0000000000000001"));
         assert!(csv.contains("different_decoded,0x00B2D882,0x00000000,0x0000000000000002"));
         assert!(csv.contains("only_s3pe,0x00B2D882,0x00000000,0x0000000000000003"));
         assert!(csv.contains("only_splitter,0x00B2D882,0x00000000,0x0000000000000004"));
         assert!(run(&a, &b, &report).is_err(), "Never overwrite an existing audit report");
         for path in [a, b, report] {
             std::fs::remove_file(path).unwrap();
         }
     }

     #[test]
     fn audit_records_decoded_sha256_for_matching_and_missing_skin_img() {
         let a = fixture("skin-with-img.package");
         let b = fixture("skin-without-img.package");
         let report = fixture("skin-audit.csv");
         sample_package(&a, &[(101, b"tone"), (501, b"skin-texture")]);
         sample_package(&b, &[(101, b"tone")]);
         run(&a, &b, &report).unwrap();
         let csv = std::fs::read_to_string(&report).unwrap();
         let header: Vec<_> = csv.lines().next().unwrap().split(',').collect();
         assert_eq!(header.len(), 16);
         assert!(header.contains(&"left_decoded_sha256"));
         let row = csv.lines().find(|line| line.starts_with("only_s3pe,")
             && line.contains("0x00000000000001F5")).unwrap();
         let cols: Vec<_> = row.split(',').collect();
         assert_eq!(cols[12], format!("{:x}", Sha256::digest(b"skin-texture")));
         assert_eq!(cols[13], "");
         assert_eq!(cols[14], "1");
         assert_eq!(cols[15], "0");
         let equal = csv.lines().find(|line| line.starts_with("same_stored,")).unwrap();
         let fields: Vec<_> = equal.split(',').collect();
         assert_eq!(fields[12], fields[13]);
         for path in [a, b, report] { std::fs::remove_file(path).unwrap(); }
     }

     #[test]
     fn audit_marks_same_tgi_multiple_payloads_without_choosing_a_hash() {
         let a = fixture("two-versions-under-one-tgi.package");
         let b = fixture("one-version.package");
         let report = fixture("two-versions.csv");
         sample_package(&a, &[(42, b"version-wall"), (42, b"version-floor")]);
         sample_package(&b, &[(42, b"version-wall")]);
         run(&a, &b, &report).unwrap();
         let csv = std::fs::read_to_string(&report).unwrap();
         let row = csv.lines().find(|line| line.starts_with("duplicate_tgi")).unwrap();
         let fields: Vec<_> = row.split(',').collect();
         assert_eq!(fields[12], "");
         assert_eq!(fields[14], "2");
         assert_eq!(fields[15], "1");
         for path in [a, b, report] { std::fs::remove_file(path).unwrap(); }
     }

     #[test]
     fn refpack_and_uncompressed_payloads_have_same_decoded_hash() {
         let compressed = fixture("refpack-compressed.package");
         let plain = fixture("refpack-plain.package");
         let report = fixture("refpack-vs-plain.csv");
         // RefPack: 3-byte size, literal ending command, decoded "cat".
         let bytes_compressed = [0u8, 0xFB, 0, 0, 3, 0xFF, b'c', b'a', b't'];
         let write_one = |path: &Path, data: &[u8]| {
             let mut package = Package::new(path, false).unwrap();
             let mut file = OpenOptions::new().read(true).write(true).open(path).unwrap();
             file.seek(SeekFrom::Start(package.index_position as u64)).unwrap();
             let offset = file.stream_position().unwrap() as u32;
             file.write_all(data).unwrap();
             package.entries.push(ResourceEntry {
                 type_id: 0x00B2_D882, group: 0, id1: 0, id2: 0xA1,
                 instance: 0xA1, chunk_offset: offset,
                 file_size: data.len() as u32, mem_size: 3,
                 compressed: if data.len() != 3 { 0xFFFF } else { 0 },
                 unknown2: 0,
             });
             package.index_position = file.stream_position().unwrap() as u32;
             drop(file);
             package.write_index_and_header().unwrap();
         };
         write_one(&compressed, &bytes_compressed);
         write_one(&plain, b"cat");
         run(&compressed, &plain, &report).unwrap();
         let csv = std::fs::read_to_string(&report).unwrap();
         let row = csv.lines().find(|r| r.starts_with("same_decoded_metadata_diff,"))
             .expect("Compression-only difference must not be classified as changed content");
         let cols: Vec<_> = row.split(',').collect();
         assert_eq!(cols[12], cols[13]);
         assert_eq!(cols[12], format!("{:x}", Sha256::digest(b"cat")));
         for path in [compressed, plain, report] {
             std::fs::remove_file(path).unwrap();
         }
     }

     #[test]
     fn buffered_stored_comparison_respects_offsets_and_differences() {
         let a = fixture("a");
         let b = fixture("b");
         std::fs::write(&a, b"XXabc123YY").unwrap();
         std::fs::write(&b, b"ZZabc123QQ").unwrap();
         let mut fa = File::open(&a).unwrap();
         let mut fb = File::open(&b).unwrap();
         assert!(stored_equal(&mut fa, &entry(2, 6), &mut fb, &entry(2, 6)).unwrap());
         assert!(!stored_equal(&mut fa, &entry(2, 6), &mut fb, &entry(0, 6)).unwrap());
         assert!(!stored_equal(&mut fa, &entry(2, 6), &mut fb, &entry(2, 5)).unwrap());
         std::fs::remove_file(a).unwrap();
         std::fs::remove_file(b).unwrap();
     }
 }
