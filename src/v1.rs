//! matrix256v1 — reference Rust implementation of the filesystem-walk
//! fingerprint. Every regular file under the walk root contributes one
//! (relative-path, size) record to a SHA-256 hash. The walk and
//! serialization logic here must stay in lockstep with the normative spec
//! in `SPEC.md`
//! (<https://github.com/shitwolfymakes/matrix256/blob/main/SPEC.md>).
//! If one changes, the other must too.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use unicode_normalization::UnicodeNormalization;

/// The matrix256 algorithm version this module implements (spec §5).
/// Distinct from a crate or package version; future algorithm versions
/// will be added as sibling submodules with their own `VERSION` constants.
pub const VERSION: &str = "1";

/// A regular file selected for matrix256v1 fingerprinting.
///
/// Returned by [`walk`] for callers that want to inspect or display the
/// entry list. [`fingerprint`] consumes these internally and only the
/// `relative` and `size` fields contribute to the digest.
#[derive(Debug, Clone)]
pub struct Entry {
    /// Absolute host path. Retained for caller inspection; **not** part
    /// of the digest input.
    pub path: PathBuf,
    /// Canonical UTF-8 byte sequence for this file's root-relative path:
    /// '/' separator, NFC-normalized, U+FFFD substitution for invalid
    /// sequences (spec §2.2). This is the byte sequence fed into SHA-256
    /// alongside [`size`](Self::size).
    pub relative: Vec<u8>,
    /// File size in bytes per filesystem metadata (spec §2.3). Never
    /// computed by reading file contents.
    pub size: u64,
}

/// Compute the matrix256v1 digest of the filesystem rooted at `root`.
///
/// Walks the tree, sorts entries by UTF-8 path bytes (spec §2.4), feeds the
/// per-entry serialization (`<path-bytes> 0x00 <size-ascii> 0x0A`, spec §2.5)
/// into SHA-256 (spec §2.6). Returns 64 lowercase hex digits. Returns the
/// underlying `io::Error` if any directory or file metadata can't be read —
/// matrix256v1 is all-or-nothing per spec §3.
pub fn fingerprint<P: AsRef<Path>>(root: P) -> io::Result<String> {
    let entries = walk(root.as_ref())?;
    let mut hasher = Sha256::new();
    for e in &entries {
        hasher.update(&e.relative);
        hasher.update([0x00]);
        hasher.update(e.size.to_string().as_bytes());
        hasher.update([0x0A]);
    }
    // `format!("{:x}", _)` on the GenericArray output yields the 64-char
    // lowercase hex required by spec §2.6.
    Ok(format!("{:x}", hasher.finalize()))
}

/// Collect every regular file under `root`, sorted by UTF-8 path bytes.
///
/// Directories are skipped (their existence is implied by the relative paths
/// of contained files), as are symbolic links (not followed, not emitted)
/// and other non-file entries (devices, sockets, FIFOs). Returns an
/// `io::Error` on any metadata failure — matrix256v1 is all-or-nothing per
/// spec §3.
pub fn walk<P: AsRef<Path>>(root: P) -> io::Result<Vec<Entry>> {
    let mut entries = Vec::new();
    let mut ancestors: Vec<String> = Vec::new();
    scan(root.as_ref(), &mut ancestors, &mut entries)?;
    entries.sort_by(|a, b| a.relative.cmp(&b.relative));
    Ok(entries)
}

/// Walk `current`, accumulating into `out`. `ancestors` is the chain of
/// root-relative path components leading to `current`; each recursive call
/// pushes its component before descending and pops on the way out, so
/// `Entry::relative` can be built from `ancestors` directly without ever
/// computing a relative path from an absolute one (which would invite a
/// `strip_prefix` that could fail at the type level).
fn scan(
    current: &Path,
    ancestors: &mut Vec<String>,
    out: &mut Vec<Entry>,
) -> io::Result<()> {
    for de in fs::read_dir(current)? {
        let de = de?;
        let ft = de.file_type()?;
        // Spec §2.1: symlinks are filtered before any other inspection —
        // neither followed nor emitted, regardless of what they point at.
        if ft.is_symlink() {
            continue;
        }
        let path = de.path();
        let name = de.file_name().to_string_lossy().into_owned();
        if ft.is_dir() {
            ancestors.push(name);
            let result = scan(&path, ancestors, out);
            ancestors.pop();
            result?;
        } else if ft.is_file() {
            let metadata = de.metadata()?;
            ancestors.push(name);
            let relative = build_relative(ancestors);
            ancestors.pop();
            out.push(Entry {
                path,
                relative,
                size: metadata.len(),
            });
        }
    }
    Ok(())
}

/// Build the canonical UTF-8 byte sequence for the file whose root-relative
/// path is `components`: '/'-joined, U+FFFD substitution for invalid
/// sequences (already done at component capture via `to_string_lossy`),
/// NFC-normalized.
///
/// `to_string_lossy` (called in `scan`) provides the U+FFFD substitution
/// required by spec §2.2: "paths that cannot be represented as valid
/// Unicode are encoded as UTF-8 with the Unicode replacement character ...
/// substituted for each invalid code unit." On Unix this substitutes in
/// raw filename bytes; on Windows it substitutes lone UTF-16 surrogates.
///
/// The `.nfc()` step (from the `unicode-normalization` crate) applies spec
/// §2.2's Unicode Normalization Form C requirement. NFC is not in std;
/// see the README for the dep justification.
///
/// Decode-then-join is equivalent to join-then-decode here because the
/// separator '/' is single-byte ASCII (no UTF-8 sequence can cross a
/// component boundary), and NFC distributes over '/' (U+002F has canonical
/// combining class 0 and is in no canonical (de)composition mapping).
fn build_relative(components: &[String]) -> Vec<u8> {
    components.join("/").nfc().collect::<String>().into_bytes()
}
