# matrix256-rs

Rust reference implementation of [**matrix256v1**](https://github.com/shitwolfymakes/matrix256) — a SHA-256 fingerprint over the (path, size) records of a rooted filesystem tree.

## Dependencies

Two runtime dependencies. Zero dev dependencies. Otherwise pure Rust on the standard library:

- `std::fs` — directory walk, file metadata.
- `std::path` — path manipulation; `Path::components()` for canonical separator handling.
- `OsStr::to_string_lossy` — UTF-8 with U+FFFD substitution for invalid sequences (spec §2.2).

The Rust ecosystem is treated as a supply-chain risk; no third-party crates may be added without explicit justification. Both deps below are irreducible without violating either Rust standard library availability ("the stdlib doesn't ship this") or basic security hygiene ("don't roll your own crypto").

Rust 1.70 or newer (2021 edition).

### Dependency: SHA-256

[`sha2`](https://crates.io/crates/sha2) — SHA-2 family (224/256/384/512). We use only `Sha256`.

**Why we accept this dep.** matrix256v1 §2.6 specifies SHA-256 as the hash function. Rust's standard library has no cryptographic hash functions — `std::hash` is for hashtables (`SipHash`-1-3 by default, explicitly *not* stable across compiler versions, not cryptographic). The two real options were (a) take a hash crate or (b) transcribe FIPS 180-4 in-tree. Option (b) is "rolling your own crypto," which is the wrong call even for a small textbook algorithm: cross-checking against the FIPS test vectors catches transcription bugs but not constant-time issues, side-channel exposure, or future hardware-acceleration gaps.

`sha2` is the [RustCrypto](https://github.com/RustCrypto) organization's reference implementation — pure Rust, multiply audited, hardware-accelerated via `cpufeatures` (SHA extensions on modern x86/ARM), and depended on transitively by most of the Rust ecosystem.

**Drop this dep the moment Rust's standard library exposes SHA-256.** If `std::hash::sha256` (or equivalent cryptographic hashing) ever ships in std, swap the call sites in [`src/v1.rs`](src/v1.rs) (`Sha256::new` / `update` / `finalize` in `fingerprint`) and remove `sha2` from `Cargo.toml`. The matrix256v1 algorithm and digest do not change; this is a pure dep removal. (As of 2026, no proposal to add crypto to std has gained traction; the Rust team's longstanding position is that crypto belongs in the ecosystem.)

### Dependency: NFC normalization

[`unicode-normalization`](https://crates.io/crates/unicode-normalization) — Unicode Normalization Forms C/D/KC/KD. We use only `nfc()`.

**Why we accept this dep.** matrix256v1 §2.2 mandates that the relative path be normalized to Unicode Normalization Form C before the bytes are hashed. NFC is not a small or self-contained algorithm: it requires the canonical-decomposition mappings, canonical-combining-class data, and composition-exclusion list from the Unicode Character Database — several thousand lines of tables that must be regenerated whenever Unicode updates. Rust's standard library does not expose normalization, and the only realistic alternative to this dep is to hand-vendor (and continually re-vendor) those UCD tables in-tree. Without NFC, the implementation is non-conformant for any input that contains non-NFC filenames on a byte-preserving filesystem (conformance fixture #14 demonstrates this exact case).

`unicode-normalization` is the canonical Rust crate for this — maintained by the unicode-rs working group, used transitively by most of the ecosystem (idna, url, regex, …), with one tiny transitive dep (`tinyvec`).

**Drop this dep the moment Rust's standard library exposes Unicode normalization.** If `str::nfc()` (or equivalent) ships in std, swap the call site in [`src/v1.rs`](src/v1.rs) (single `.nfc()` invocation in `build_relative`) and remove `unicode-normalization` from `Cargo.toml`. The matrix256v1 algorithm and digest do not change; this is a pure dep removal.

## Library discipline

The library promise is: **a consumer's process must never break because of code in this crate.** To enforce this rather than promise it, [`src/lib.rs`](src/lib.rs) turns the relevant footguns into compile errors at the crate root. CI runs `cargo clippy --all-targets -- -D warnings` so any new violation fails the build.

| Category | Lints | What's guarded |
|---|---|---|
| Memory safety | `forbid(unsafe_code)` | No `unsafe { }` blocks, ever — `forbid` cannot be opted out via `#[allow]`. |
| Panic discipline | `clippy::unwrap_used`, `clippy::expect_used`, `clippy::unwrap_in_result`, `clippy::panic`, `clippy::unreachable`, `clippy::todo`, `clippy::unimplemented`, `clippy::dbg_macro` | Every direct or stylistic route to a runtime panic. Code is refactored so the success case is the only representable one — e.g. `data.get(..64).and_then(|s| <[u8; 64]>::try_from(s).ok())` instead of slicing-then-unwrapping. |
| Bounds checking | `clippy::indexing_slicing`, `clippy::string_slice` | `arr[i]` and `&s[i..j]` are panic sites. Force the `.get(...)` / `.get_mut(...)` Option-returning forms. |
| Conversion safety | `clippy::as_conversions` | `as` casts can silently truncate or lose precision. Force `From` / `TryFrom`. |
| Output discipline | `clippy::print_stdout`, `clippy::print_stderr` | A library has no business writing to stdout/stderr from a fingerprint call. |
| Documentation | `deny(missing_docs)` | Every public item carries a `///` doc comment. Public API stays self-describing. |

Tests in `src/.../mod tests` opt back out via an inner `#![allow(...)]` where the lint conflicts with idiomatic test code (e.g. `assert_eq!`). Integration tests under [`tests/`](tests/) are a separate crate and unaffected by these lints.

## Usage

```rust
use matrix256::v1;

fn main() -> std::io::Result<()> {
    let digest = v1::fingerprint("/media/user/DISC")?;
    println!("{digest}");
    Ok(())
}
```

The crate exposes nothing at the top level. Future algorithm versions will be added as sibling submodules (`v2`, …) so callers always address an explicit version.

## Conformance

The Tier-1 conformance test is the synthetic fixture suite at [`tests/conformance.rs`](tests/conformance.rs). Each fixture is a `#[test]` function: it constructs the fixture in a temporary directory, runs `v1::fingerprint`, and asserts the produced digest matches the canonical value from the spec repo's [`conformance_fixtures.json`](https://github.com/shitwolfymakes/matrix256/blob/main/conformance_fixtures.json) (human-readable companion: [`CONFORMANCE_FIXTURES.md`](https://github.com/shitwolfymakes/matrix256/blob/main/CONFORMANCE_FIXTURES.md)). The expected digests are inlined as constants so the test suite has no external data dependency and no JSON parser is needed.

```
cargo test                                  # all fixtures
cargo test --test conformance fixture_05    # one fixture
```

Platform-incompatible fixtures (e.g. case-sensitive sort on a case-insensitive filesystem, surrogate-escape paths off Linux) are reported as no-op passes with an `eprintln!` skip note — Cargo's test framework has no distinct "skip" status.

If the spec repo regenerates its fixtures, the inlined expected-digest constants in `tests/conformance.rs` must be re-synced from `conformance_fixtures.json`.

## See also (in the [spec repo](https://github.com/shitwolfymakes/matrix256))

- `SPEC.md` — normative algorithm
- `RATIONALE.md` — design rationale
- `IMPLEMENTERS.md` — practical guidance (encoding, mount handling, bridge discs)
- `CORPUS.md` — known-good digests across real discs
- `CONFORMANCE_FIXTURES.md` / `conformance_fixtures.json` — Tier-1 synthetic fixture suite

## License

Licensed under the Apache License, Version 2.0. See [`LICENSE`](LICENSE) for the full text. Apache 2.0 grants an explicit patent license from contributors to users and includes a patent retaliation clause that terminates those grants for any party that initiates patent litigation alleging the work infringes their patents.
