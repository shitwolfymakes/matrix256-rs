//! matrix256 — reproducible fingerprints for optical discs and filesystem trees.
//!
//! The active algorithm version lives in [`v1`]: a SHA-256 over a canonical
//! serialization of the (path, size) records of every regular file under the
//! walk root. See `SPEC.md` in the spec repo for the normative specification:
//! <https://github.com/shitwolfymakes/matrix256/blob/main/SPEC.md>.
//!
//! Calling code addresses the algorithm explicitly:
//!
//! ```no_run
//! use matrix256::v1;
//! let digest = v1::fingerprint("/media/user/DISC")?;
//! # Ok::<(), std::io::Error>(())
//! ```
//!
//! The crate exposes nothing at the top level so future versions can be added
//! as sibling submodules (`v2`, …) without a "current" default that would
//! silently change behavior.

// Library-discipline lints. The promise to consumers: a process using this
// crate must never break because of code we shipped. The lints below
// enforce that promise by making the relevant footguns into compile errors
// rather than review comments.
//
//   - `forbid(unsafe_code)`         No `unsafe { }` blocks, ever. Cannot
//                                   be opted out via `#[allow]`.
//   - `deny(missing_docs)`          Every public item carries a `///`
//                                   doc comment. Public API stays
//                                   self-describing.
//   - Panic-discipline (clippy):    `unwrap_used`, `expect_used`,
//                                   `unwrap_in_result`, `panic`,
//                                   `unreachable`, `todo`, `unimplemented`,
//                                   `dbg_macro` — every direct or stylistic
//                                   route to a runtime panic is denied.
//   - Bounds-checking (clippy):     `indexing_slicing`, `string_slice` —
//                                   `arr[i]` and `&s[i..j]` are panic
//                                   sites; force the `.get(...)` /
//                                   `.get_mut(...)` Option-returning forms.
//   - Conversion safety (clippy):   `as_conversions` — `as` casts can
//                                   silently truncate or lose precision;
//                                   force `From` / `TryFrom`.
//   - Output discipline (clippy):   `print_stdout`, `print_stderr` — lib
//                                   code has no business writing to
//                                   stdout/stderr from a fingerprint call.
//
// Tests in `src/.../mod tests` opt back out via an inner `#![allow(...)]`
// where the lint conflicts with idiomatic test code. Integration tests
// under `tests/` are a separate crate and not affected by this block.
#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::unwrap_in_result,
    clippy::panic,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented,
    clippy::dbg_macro,
    clippy::indexing_slicing,
    clippy::string_slice,
    clippy::as_conversions,
    clippy::print_stdout,
    clippy::print_stderr
)]

pub mod v1;
