# M2-G-A1 — pure native topology converter

This draft adds `echelon_os_lib::sde::convert(&[u8], expected_build, trusted_sha256)` and the same production module to the native-independent test harness. It returns canonical snapshot text plus build, archive/table/snapshot hashes and topology counts, or a bounded `Error` enum. The caller must supply independently trusted metadata; computing an input hash does not authenticate CCP provenance. No production pin changes.

The module has no filesystem, extraction, network, external program, application startup, IPC or UI access. It does not install missing route data. M2-G-A2 downloading/publishing and its UI remain separate, unimplemented work. M2-F real routing/waypoint acceptance is still pending privately provisioned data and owner validation.

## Input and resource boundary

Input bytes are immutable and borrowed; the 256 MiB cap and caller hash are checked before ZIP parsing. A small directory-framing preflight bounds entry metadata before the ZIP library allocates it. Single-disk ordinary ZIP and standard ZIP64 directory records are supported. Split archives, self-extracting prefixes, nonstandard extensible ZIP64 end records and unsafe/ambiguous framing are rejected conservatively. Names must be canonical UTF-8 relative POSIX paths, with no empty/dot/parent components, NUL, backslash, colon, duplicates or directory entries. Only regular/unspecified file modes, unencrypted stored/deflated entries are accepted. No permissions or paths from the archive are applied to disk.

Budgets mirror the reviewed Python converter: 256 members; 1 GiB total declared uncompressed; 512 MiB/member; at most 1000:1 expansion; 16 MiB per map table; 64 KiB metadata/JSONL lines; 10,000 systems; 100,000 gates; 50,000 edges; 4 MiB canonical output. Total gate declarations are also bounded to 100,000, because any valid topology must match the bounded gate table. Consumed metadata/tables use bounded chunk reads and CRC verification; other entries are checked for safety/declared budgets and never decompressed. The whole archive hash covers even unused payloads, but their individual CRCs are not verified, matching Python's projection.

The 30-second budget is cooperative: checked during hashing, directory/member loops, consumed reads and topology projection, plus before returning. Library calls, one bounded JSON line and serialization are not preempted. It is not a hard wall-clock kill or process memory sandbox. Borrowed archive bytes, at most 32 MiB table bytes, capped line/depth parsing and bounded projected collections constrain allocation; caller memory/process policy is outside this API.

## Projection and canonical bytes

`_sde.jsonl` must contain the expected integer build, `_key: "sde"` and string release date. Duplicate JSON keys are rejected recursively, with serde_json's normal depth limit. Invalid UTF-8, lone Unicode surrogates, nonfinite numbers and trailing JSON are refused. This is intentionally stricter than Python's permissive handling of lone surrogates/overflowing numeric literals; none are repaired. IDs must be positive safe integers of integer JSON type. Missing `name`/`name.en` means null; a present name must be an object, and English must be null or a nonempty string of at most 256 Unicode scalar characters. Missing gate lists mean empty; present lists must be arrays of unique valid IDs.

All gate ownership declarations, references and reciprocal pairs must agree exactly. Duplicate systems/gates/connections, unknown systems, missing declarations, self connections and nonreciprocal pairs fail. Isolated systems remain present. Systems and undirected edges sort numerically. Output has fixed lexicographic keys `edges,systems,version`, compact separators, ASCII escaping (including non-BMP UTF-16 pairs and DEL), and exactly one final LF. No region filtering, invented names, live risk or SDE update logic.

`Metadata.table_sha256` is ordered as `mapSolarSystems.jsonl`, then `mapStargates.jsonl`; hashes cover original table bytes including original line endings. Snapshot format/converter version remains 1. This output is not independently signed provenance or automatically trusted by the runtime reader.

## Dependencies and synthetic evidence

Both manifests pin `zip = 9.0.0`, defaults disabled, enabling only `deflate-flate2-zlib-rs`. It declares Rust 1.88 and is compatible with repository Rust 1.90. This selects a pure Rust DEFLATE backend without archive encryption or other compression formats. Only member-reading APIs are used; extraction APIs are never called. Version 9 addresses the reviewed RustSec extraction advisories RUSTSEC-2026-0367/0368/0369/0429; using a patched crate is not a general security certification. Existing serde_json and SHA-256 dependencies are reused; existing locked versions/checksums are retained. No workflow or frontend dependencies change.

Tests generate only invented stored/deflated archives in memory. Independent fixed canonical bytes/hash cover unordered records, Unicode, null defaults and an isolated system. The development-only Python oracle uses the unchanged reviewed Python converter and deletes its fabricated temporary ZIP before exit:

```sh
python3 src-tauri/src/services/sde/reference.py
python3 -m unittest discover -s scripts/sde -p 'test_*.py'
cargo +1.90.0 test --manifest-path tools/storage-tests/Cargo.toml --locked
cargo +1.90.0 clippy --manifest-path tools/storage-tests/Cargo.toml --locked --all-targets -- -D warnings
cargo fmt --manifest-path src-tauri/Cargo.toml --all --check
```

The Python oracle and Rust tests assert the same independently specified 99-byte ASCII snapshot and SHA-256 `c7b670c1645bed915e059f887c05c2256ca98ba57f84d48e7508786d4f587d90`. Python is used only for development comparison, never by the native API. Negative fixtures exercise digest/build/CRC mismatch, duplicate names/JSON keys, unsafe names/modes/encryption/compression, declared bombs, budgets, malformed/truncated directories, ZIP64, JSON depth/types/lines and contradictory topology. Tight private test limits and fabricated size fields avoid allocating bombs. No real CCP archive, fragment, table or snapshot was downloaded or converted for A1.

Local shared-module compilation/tests and lint do not prove full Tauri or Windows compilation/runtime behavior. The full Tauri check remains unavailable here due missing Linux GLib/GObject/GIO/GDK prerequisites. No native Windows run, installer, real data retrieval, account login or waypoint request is authorised or performed by A1.
