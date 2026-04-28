// Tier-1 conformance runner for matrix256v1 (Rust implementation).
//
// Constructs each synthetic fixture from the matrix256 spec repo's
// CONFORMANCE_FIXTURES.md in a fresh temporary directory, runs
// `matrix256::v1::fingerprint` against it, and compares the produced digest
// to the canonical value published in the spec repo's
// conformance_fixtures.json.
//
// Unlike the Python and JavaScript siblings, which read the JSON at runtime,
// this Rust runner uses Rust's built-in test framework (one `#[test]` per
// fixture) with the expected digests inlined as constants — avoiding the
// need for a JSON parser dependency. The constants below MUST stay in sync
// with conformance_fixtures.json in the spec repo. If the spec repo
// regenerates fixtures, update the constants here.
//
// Fixtures whose platform requirements aren't met on the current host
// (case-sensitive filesystem, symlink support, byte-preserving filesystem,
// 200-byte component names, non-UTF-8 filenames) are early-returned with an
// `eprintln!` skip note rather than failed — Cargo's test framework has no
// distinct "skip" status, so successful no-op is the closest available.
//
// Stdlib only — matches the crate's no-deps policy.
//
// Run:
//   cargo test --test conformance
//   cargo test --test conformance fixture_05
//   cargo test --test conformance -- --ignored    # also runs the NFC fixture

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use matrix256::v1;

// --- Temp directory helper ----------------------------------------------
//
// Rust's std exposes `env::temp_dir` but no `mkdtemp`. We compose unique
// names from PID + nanosecond timestamp + a process-local atomic counter,
// retry on collision, and clean up via Drop so failed assertions don't
// leak fixture directories.

static SEQ: AtomicU64 = AtomicU64::new(0);

struct TmpDir(PathBuf);

impl TmpDir {
    fn new(prefix: &str) -> std::io::Result<Self> {
        let base = std::env::temp_dir();
        let pid = std::process::id();
        for _ in 0..32 {
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0);
            let n = SEQ.fetch_add(1, Ordering::Relaxed);
            let name = format!("{prefix}{pid}_{nanos}_{n}");
            let path = base.join(name);
            match fs::create_dir(&path) {
                Ok(()) => return Ok(TmpDir(path)),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(e),
            }
        }
        Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            "exhausted temp dir name attempts",
        ))
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TmpDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn write_file(path: &Path, bytes: &[u8]) {
    let mut f = fs::File::create(path).unwrap_or_else(|e| panic!("create {path:?}: {e}"));
    f.write_all(bytes)
        .unwrap_or_else(|e| panic!("write {path:?}: {e}"));
}

fn assert_digest(tmp: &TmpDir, expected: &str) {
    let produced = v1::fingerprint(tmp.path()).expect("fingerprint must succeed");
    assert_eq!(
        produced, expected,
        "digest mismatch in {:?}",
        tmp.path()
    );
}

// --- Platform capability probes ----------------------------------------

fn probe_case_sensitive() -> bool {
    let Ok(probe) = TmpDir::new("m256_probe_case_") else {
        return false;
    };
    write_file(&probe.path().join("A"), b"");
    if fs::write(probe.path().join("a"), b"").is_err() {
        return false;
    }
    let mut names: Vec<String> = match fs::read_dir(probe.path()) {
        Ok(it) => it
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect(),
        Err(_) => return false,
    };
    names.sort();
    names == vec!["A".to_string(), "a".to_string()]
}

#[cfg(unix)]
fn try_symlink(target: &str, link: &Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(target, link)
}

#[cfg(windows)]
fn try_symlink(target: &str, link: &Path) -> std::io::Result<()> {
    // On Windows a "file" symlink needs the target's type known up front.
    // Tests that use this point to either an existing file or a missing
    // target; in both cases symlink_file matches the spec's "not followed,
    // not emitted" treatment.
    std::os::windows::fs::symlink_file(target, link)
}

#[cfg(not(any(unix, windows)))]
fn try_symlink(_target: &str, _link: &Path) -> std::io::Result<()> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "symlinks unsupported on this platform",
    ))
}

// --- Fixtures -----------------------------------------------------------
//
// Expected digests pasted from
//   matrix256/conformance_fixtures.json
// in the spec repo. Keep IDs and digests aligned with that file.

#[test]
fn fixture_01_empty_directory() {
    let tmp = TmpDir::new("m256_fix01_").unwrap();
    assert_digest(
        &tmp,
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
    );
}

#[test]
fn fixture_02_single_zero_byte_file() {
    let tmp = TmpDir::new("m256_fix02_").unwrap();
    write_file(&tmp.path().join("a"), b"");
    assert_digest(
        &tmp,
        "576ada568edb673473287643d06ca9b763d81b712a080388fbf445bf580dab3d",
    );
}

#[test]
fn fixture_03_single_small_ascii_file() {
    let tmp = TmpDir::new("m256_fix03_").unwrap();
    write_file(&tmp.path().join("hello.txt"), b"hello\n");
    assert_digest(
        &tmp,
        "00c8e12fff1075e74071d424a34ec9e89e2ffc96c5c4ec6a5bf7a3b5941b3324",
    );
}

#[test]
fn fixture_04_two_files_at_root() {
    let tmp = TmpDir::new("m256_fix04_").unwrap();
    write_file(&tmp.path().join("a"), b"");
    write_file(&tmp.path().join("b"), b"");
    assert_digest(
        &tmp,
        "a7cde029efe3b62bb536d2eead4b0900409eea281230c0e1146dd0db645a2042",
    );
}

#[test]
fn fixture_05_case_sensitive_sort() {
    if !probe_case_sensitive() {
        eprintln!("[ skip ] fixture 05 — filesystem is case-insensitive");
        return;
    }
    let tmp = TmpDir::new("m256_fix05_").unwrap();
    write_file(&tmp.path().join("A"), b"");
    write_file(&tmp.path().join("a"), b"");
    assert_digest(
        &tmp,
        "e99dec2b961d71942f740d942301fdb9e1268eeca6b21161dfaf5b7c253ed660",
    );
}

#[test]
fn fixture_06_slash_vs_dash_sort() {
    let tmp = TmpDir::new("m256_fix06_").unwrap();
    write_file(&tmp.path().join("a-b"), b"");
    fs::create_dir(tmp.path().join("a")).unwrap();
    write_file(&tmp.path().join("a").join("b"), b"");
    assert_digest(
        &tmp,
        "82d1301cbc45799e538f19a52840b9ff5a9ca797d80c5e52b4d98c4750d2b5e3",
    );
}

#[test]
fn fixture_07_nested_directories() {
    let tmp = TmpDir::new("m256_fix07_").unwrap();
    let nested = tmp.path().join("dir1").join("dir2");
    fs::create_dir_all(&nested).unwrap();
    write_file(&nested.join("file.txt"), b"");
    assert_digest(
        &tmp,
        "8f2c64be52e682809a97f2e370a2638c10e3c3f9071eaa0bda3f7fc4c6c6eccb",
    );
}

#[test]
fn fixture_08_sibling_full_path_sort() {
    let tmp = TmpDir::new("m256_fix08_").unwrap();
    fs::create_dir(tmp.path().join("a")).unwrap();
    write_file(&tmp.path().join("a").join("z"), b"");
    fs::create_dir(tmp.path().join("b")).unwrap();
    write_file(&tmp.path().join("b").join("a"), b"");
    assert_digest(
        &tmp,
        "ab44545fa7095c239cd8e9fa36eff237b1cc8e32c5126e98591b24250aa11871",
    );
}

#[test]
fn fixture_09_only_empty_subdir() {
    let tmp = TmpDir::new("m256_fix09_").unwrap();
    fs::create_dir(tmp.path().join("empty")).unwrap();
    assert_digest(
        &tmp,
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
    );
}

#[test]
fn fixture_10_file_plus_empty_subdir() {
    let tmp = TmpDir::new("m256_fix10_").unwrap();
    write_file(&tmp.path().join("hello.txt"), b"hello\n");
    fs::create_dir(tmp.path().join("empty")).unwrap();
    assert_digest(
        &tmp,
        "00c8e12fff1075e74071d424a34ec9e89e2ffc96c5c4ec6a5bf7a3b5941b3324",
    );
}

#[test]
fn fixture_11_only_a_symlink() {
    let tmp = TmpDir::new("m256_fix11_").unwrap();
    if let Err(e) = try_symlink("nonexistent", &tmp.path().join("link")) {
        eprintln!("[ skip ] fixture 11 — symlinks not supported ({e})");
        return;
    }
    assert_digest(
        &tmp,
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
    );
}

#[test]
fn fixture_12_symlink_alongside_file() {
    let tmp = TmpDir::new("m256_fix12_").unwrap();
    write_file(&tmp.path().join("real.txt"), b"x");
    if let Err(e) = try_symlink("real.txt", &tmp.path().join("link")) {
        eprintln!("[ skip ] fixture 12 — symlinks not supported ({e})");
        return;
    }
    assert_digest(
        &tmp,
        "1f99a83be1c9ac0d243b7937f15908a03ede98ffa24c18fcf6100fca66506df4",
    );
}

#[test]
fn fixture_13_latin_diacritics_nfc() {
    // The literal "café.txt" below is saved by editors as NFC bytes
    // (U+00E9 — single composed code point), which is what a byte-NFC
    // filesystem will store and read back. No NFC pass needed for the
    // hash to match; the source bytes are already canonical.
    let tmp = TmpDir::new("m256_fix13_").unwrap();
    write_file(&tmp.path().join("café.txt"), b"");
    assert_digest(
        &tmp,
        "afd2f606ae4f4e4d644cbb28ab2f1c5d46d6f98130304efd9941db17d6a91dcd",
    );
}

#[test]
fn fixture_14_latin_diacritics_nfd() {
    // Filename built as 'cafe' + U+0301 (combining acute) — NFD form. The
    // expected digest is the NFC-byte hash, matching fixture 13. Tests
    // that the canonicalization step normalizes NFD → NFC before hashing.
    // Skipped on filesystems that auto-NFC-normalize at write time
    // (e.g. APFS).
    let tmp = TmpDir::new("m256_fix14_").unwrap();
    let mut name = String::from("cafe");
    name.push('\u{0301}');
    name.push_str(".txt");
    let written = tmp.path().join(&name);
    write_file(&written, b"");
    let listed: Vec<String> = fs::read_dir(tmp.path())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    if listed != vec![name.clone()] {
        eprintln!("[ skip ] fixture 14 — filesystem auto-normalized the filename at write time");
        return;
    }
    assert_digest(
        &tmp,
        "afd2f606ae4f4e4d644cbb28ab2f1c5d46d6f98130304efd9941db17d6a91dcd",
    );
}

#[test]
fn fixture_15_cyrillic() {
    let tmp = TmpDir::new("m256_fix15_").unwrap();
    write_file(&tmp.path().join("привет.txt"), b"");
    assert_digest(
        &tmp,
        "c044182349eea94dff66a1ce2764e6f809cbf8893b2071d5906203b41fea21c0",
    );
}

#[test]
fn fixture_16_han() {
    let tmp = TmpDir::new("m256_fix16_").unwrap();
    write_file(&tmp.path().join("你好.txt"), b"");
    assert_digest(
        &tmp,
        "339e0893d9d4aa8df81e9e7d671983f7befa124bd86416dc69697c32d8112787",
    );
}

#[test]
fn fixture_17_arabic() {
    let tmp = TmpDir::new("m256_fix17_").unwrap();
    write_file(&tmp.path().join("مرحبا.txt"), b"");
    assert_digest(
        &tmp,
        "9ec64191ddf011278744183c8830b3b7e7c6f35fbff37c66122f0ae0e7add033",
    );
}

#[test]
fn fixture_18_emoji() {
    let tmp = TmpDir::new("m256_fix18_").unwrap();
    write_file(&tmp.path().join("🎵.txt"), b"");
    assert_digest(
        &tmp,
        "7c547ce5b89040b67d9cbf5c2ec5556090fdcfa8f3120b48a856c054769b7816",
    );
}

#[test]
fn fixture_19_multi_script() {
    let tmp = TmpDir::new("m256_fix19_").unwrap();
    for name in ["ascii.txt", "café.txt", "你好.txt", "🎵.txt"] {
        write_file(&tmp.path().join(name), b"");
    }
    assert_digest(
        &tmp,
        "b7ce4f0d4e8cde3698b11edc79c49639b3f04cf88e128b0f1c3f0951843f7966",
    );
}

#[test]
fn fixture_20_size_boundaries() {
    let tmp = TmpDir::new("m256_fix20_").unwrap();
    let sizes: &[(&str, usize)] = &[
        ("size_0000000", 0),
        ("size_0000001", 1),
        ("size_0000255", 255),
        ("size_0000256", 256),
        ("size_0065535", 65535),
        ("size_0065536", 65536),
        ("size_1000000", 1_000_000),
    ];
    for (name, size) in sizes {
        write_file(&tmp.path().join(name), &vec![0u8; *size]);
    }
    assert_digest(
        &tmp,
        "ac2ee75612a4d578fe365711b2f8aef71e40b2f8c2abf212fa26308d857160e6",
    );
}

#[test]
fn fixture_21_many_small_files() {
    let tmp = TmpDir::new("m256_fix21_").unwrap();
    for i in 0..100 {
        let name = format!("f{i:03}");
        write_file(&tmp.path().join(name), b"");
    }
    assert_digest(
        &tmp,
        "a164865515f0f66b25cc4aff36e558a602d3db6caf62d41d1e830f9283b3dc8f",
    );
}

#[test]
fn fixture_22_deeply_nested() {
    let tmp = TmpDir::new("m256_fix22_").unwrap();
    let mut nested = tmp.path().to_path_buf();
    for letter in "abcdefghij".chars() {
        nested.push(letter.to_string());
    }
    fs::create_dir_all(&nested).unwrap();
    write_file(&nested.join("file.txt"), b"");
    assert_digest(
        &tmp,
        "35997ed41f132aad8afc1e08a577090dff4aaa7bb23ffe5f874e879fbc38475f",
    );
}

#[test]
fn fixture_23_long_filename() {
    let tmp = TmpDir::new("m256_fix23_").unwrap();
    let name: String = "a".repeat(200);
    if let Err(e) = fs::write(tmp.path().join(&name), b"") {
        eprintln!("[ skip ] fixture 23 — filesystem rejected 200-byte component ({e})");
        return;
    }
    assert_digest(
        &tmp,
        "31013f1f14b4c55273b923a96047c43e157423625160c53dad1f7971de44db58",
    );
}

#[cfg(target_os = "linux")]
#[test]
fn fixture_24_surrogate_escape_filename_byte() {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;

    let tmp = TmpDir::new("m256_fix24_").unwrap();
    // Build the filename as raw bytes including 0xff (not valid UTF-8).
    let raw_name: &[u8] = b"bad\xff.txt";
    let path = tmp.path().join(OsStr::from_bytes(raw_name));
    if let Err(e) = fs::write(&path, b"") {
        eprintln!("[ skip ] fixture 24 — could not create non-UTF-8 filename ({e})");
        return;
    }
    assert_digest(
        &tmp,
        "8392ec1f2dec1510d58ade51d070394768a4fbbe917c677387901f1147dd439a",
    );
}

#[cfg(not(target_os = "linux"))]
#[test]
fn fixture_24_surrogate_escape_filename_byte() {
    eprintln!(
        "[ skip ] fixture 24 — non-UTF-8 filenames unsupported on {}",
        std::env::consts::OS
    );
}

#[test]
fn fixture_25_prefix_and_trailing_sort() {
    let tmp = TmpDir::new("m256_fix25_").unwrap();
    for name in ["foo", "foo.txt", "foobar"] {
        write_file(&tmp.path().join(name), b"");
    }
    assert_digest(
        &tmp,
        "599b5d5fd9d52740c6b40f134b260b52de60bed70ee60aa0536ee8474fc65bcc",
    );
}

#[test]
fn fixture_26_content_irrelevance() {
    let tmp = TmpDir::new("m256_fix26_").unwrap();
    write_file(&tmp.path().join("hello.txt"), b"world!");
    assert_digest(
        &tmp,
        "00c8e12fff1075e74071d424a34ec9e89e2ffc96c5c4ec6a5bf7a3b5941b3324",
    );
}
