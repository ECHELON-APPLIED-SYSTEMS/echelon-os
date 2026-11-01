# Native-independent storage tests

From the repository root, with Rust 1.90.0 installed:

```sh
cargo +1.90.0 test --manifest-path tools/storage-tests/Cargo.toml --locked
cargo +1.90.0 clippy --manifest-path tools/storage-tests/Cargo.toml --locked --all-targets -- -D warnings
```

This crate imports the actual production `portable.rs` module by relative path, not a source copy. It also imports the actual NAVCOM reader and M2-F native EVE service. Exact dependency versions and the independent lockfile align with the native manifest; there is no Tauri, GTK or WebKit dependency. M2-F adds the reviewed cryptography/HTTP dependencies documented in docs/EVE_SESSION.md. Dependency versions must remain aligned with src-tauri/Cargo.lock. All nine original storage tests execute on Linux, plus any source-only regressions added there. Windows-specific junction/FFI branches remain Windows-only and are not validated by this target. Write denial uses an explicit internal seam, not a claim of real Windows ACL testing.

The bounded Ubuntu storage CI job runs this command on the same automatic events as frontend checks. It does not compile Tauri or produce an app/artifact. See docs/PORTABLE.md for startup scan assessment and pending native validation.

M1-D also exercises strict v1/v2 compatibility, allowlisted save/reload, failed replacement preservation, external-edit detection, OS-lock exclusion and concurrent native callers against the actual production module. These Linux tests do not prove the Windows replace/locking branches or full Tauri IPC ACL; the latter has a real-handler test gated by native prerequisites.

M2-F tests use synthetic RSA/JWKS, mocked CCP transports and local loopback listeners. They cover callback/PKCE/JWT boundaries, cancellation, cache/rate limits, session clearing, pinned destination authorization and one-shot waypoint flags. They never contact CCP or use real credentials/datasets. Full Tauri ACL and Windows-specific browser/TLS/listener behavior need separately approved native validation.
