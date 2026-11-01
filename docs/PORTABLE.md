# ECHELON OS — portable Windows foundation (M1-B)

## Extract and run

Extract the complete `ECHELON-OS` folder from `echelon-os-portable-windows-x64.zip` to a writable local folder. Launch `echelon-os.exe` as your regular user. No installer or elevation is needed. Do not run directly inside the ZIP. This unsigned build may show Windows publisher/SmartScreen warnings; do not disable security controls.

Windows 11 x64 and Microsoft Edge **WebView2 Evergreen Runtime** are required. This ZIP does not bundle a Fixed Version runtime and is not dependency-free. If WebView2 is absent or cannot start, the application reports a startup recovery error; install/repair Microsoft's official Evergreen Runtime separately and retry. Runtime installation is not automated by this app.

All currently supported desktop builds, including development, use portable storage. NSIS generation/support has been retired for M1-B; historical installer results remain historical evidence only. Do not install this executable in a protected system directory.

## Data and recovery

Before a WebView opens, Rust resolves the real executable directory and validates a dedicated `app-data` child. The process working directory is never used as a storage anchor. Settings, application configuration/data, cache/log locations and the default Windows WebView2 profile use this root via Tauri's supported app directory override. Window `dataDirectory` stays unset. Windows and the installed WebView2 runtime can still create OS-managed data outside this folder; this is not a claim of zero system writes.

First run creates `app-data/settings.json`, `app-data/caches` and `app-data/logs`; WebView2 adds its own files on native startup. Initial settings are non-sensitive and minimal:

```json
{
  "schema_version": 1,
  "preferences": {}
}
```

There are no editable preferences, five-theme implementation, SQLite, credentials or settings-write IPC yet. The empty typed preferences object is an explicit future extension seam. Existing valid settings are loaded without rewriting. Unsupported versions, extra fields, malformed/oversized files and linked/junction data paths fail closed; there is no automatic migration or reset. The startup scan rejects special files, links/reparse points and unusually deep/large data trees (over 64 levels or 100,000 entries). Another process changing filesystem paths concurrently is outside this local same-user trust boundary; close other app copies before moving or repairing data.

Creation writes a uniquely created temporary file, flushes it and atomically publishes it only if `settings.json` is absent. Windows uses same-directory no-replace `MoveFileExW`; the Unix development/test implementation uses create-if-absent hard-link publication. A crash can leave a `.settings-*.tmp` file, which is preserved and never treated as settings. No existing settings are overwritten. Power-loss durability ultimately depends on the filesystem/hardware; keep backups. Recovery does not automatically delete stale files. There is no update/write API in this milestone; future preference changes need separate validated atomic replacement/version rules.

If the app cannot write, it shows a recovery message before opening a WebView and uses no fallback in AppData, home, temp or the working directory. Close the app, preserve a backup, and move the complete folder to a writable location. For corrupt/newer-version settings, preserve the original and restore a known-good backup or use the matching app version. Do not delete data merely to suppress an error. Messages omit absolute paths and raw OS details. A successful real write/flush/delete probe is required even when settings already exist.

To move the app, close every copy and move the **whole folder including app-data**. Then relaunch the executable. No absolute data paths are saved in settings. Do not move live WebView profile files or share one folder between concurrent instances. The OS Evergreen WebView2 prerequisite must also be present on the destination machine. Cross-drive, removable-media and read-only behavior still require native smoke testing; automated same-filesystem moves alone do not prove these.

## Development and build

Use Node 24, pinned Rust 1.90.0, MSVC/Windows SDK and Evergreen WebView2 as described in README.md:

```powershell
npm ci
npm run tauri -- dev
# Build only when separately authorised:
npm run tauri -- build --no-bundle -- --locked
```

The shared Tauri config intentionally applies `appDirectoriesOverride: "./app-data"` to **every supported distribution**, since M1-B no longer supports an installed flavour. Startup resolves and validates the absolute executable-adjacent path before configuring Tauri. Development data stays beside the built executable under the ignored target tree; browser preview does not invoke native storage. Tests use isolated temporary fixtures and a mocked runtime, not global user AppData. No arbitrary file access or new IPC permissions are granted.

The on-demand standard Windows job calls `scripts/package-portable.ps1`, preserves full frontend/Rust checks and stages only the EXE plus this readme in a newly created private scratch `ECHELON-OS` folder. It never reuses or recursively cleans arbitrary pre-existing staging data. It enumerates every immediate entry with hidden entries included, rejects all directories, reparse points/links, hidden/system files, non-allowlisted names and payloads over 50 MiB. It validates the produced archive against the exact two-file entry allowlist (an empty parent entry is harmless), rejects duplicates/traversal/links and enforces the uncompressed-size limit again before no-replace publication. Only the explicitly named ZIP is uploaded with one-day retention. No app-data, cache, secrets or test files are packaged. This packaging path has not been executed for M1-B until a separate Windows run is approved. Automatic PR/main checks remain frontend-only; dispatch/label triggers are unchanged. There is no updater, autostart, registry setup, elevation request, telemetry, new runtime background service or release publication.

See docs/SMOKE_TEST.md in the source repository for the acceptance procedure. M1-A's earlier native launch evidence does not validate M1-B storage or this ZIP.

## Reproducible source-only validation

From a clean checkout with Rust 1.90.0 and PowerShell 7 installed:

```sh
cargo +1.90.0 test --manifest-path tools/storage-tests/Cargo.toml --locked
cargo +1.90.0 clippy --manifest-path tools/storage-tests/Cargo.toml --locked --all-targets -- -D warnings
pwsh -NoLogo -NoProfile -File scripts/test-package-portable.ps1
```

The committed harness imports the production storage module and exact locked serde versions. All nine original Linux storage tests plus three regression/scan tests run without Tauri, GTK or WebKit. The bounded five-minute standard Ubuntu storage CI job runs these tests and synthetic packaging checks on normal automatic PR/main events. It does not execute an EXE, compile Tauri/Windows, produce an application ZIP or upload artifacts. PowerShell tests use fake file bytes and disposable temporary archives only; no Pester/new project dependency is required. Full Windows conditions are unchanged and remain on hold.

### Startup scan assessment — retained, performance risk deferred

`validate_tree` still inspects the entire data tree on each launch to reject nested links/reparse points before WebView receives it. It is O(total entries), with unchanged hard limits of 64 levels and 100,000 entries. A local Linux fixture measured approximately **1.61 ms for 1,000** and **13.24 ms for 10,000** recently created regular cache files, plus the normal root metadata/probe/settings overhead. These are warm synthetic local filesystem measurements, not cold-cache, Windows, removable-drive or real WebView profile benchmarks, and no timing threshold is asserted in CI. Reproduce current timings with:

```sh
cargo +1.90.0 test --manifest-path tools/storage-tests/Cargo.toml --locked large_regular_cache -- --nocapture
```

A 10,000-file legitimate cache succeeds; deep nested link, depth and entry-budget regressions remain fail-closed. Skipping WebView cache trees or caching scan results would allow links added since the last scan and weaken the current protection. No safe narrow platform-neutral optimisation has been established, so traversal behavior is intentionally preserved. Large/cold Windows profiles may still launch slowly or hit the limits; this is a **deferred performance/availability risk**, not a solved or Windows-validated property. A future handle-based Windows design or realistic profiling needs separate scoped review when it blocks a concrete decision; this remediation requests no native build.
