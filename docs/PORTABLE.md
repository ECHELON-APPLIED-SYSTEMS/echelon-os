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

The on-demand standard Windows job preserves full frontend/Rust checks and stages only the EXE plus this readme in `ECHELON-OS`. It rejects unexpected payloads or payloads over 50 MiB, compresses the folder and uploads only the explicitly named ZIP with one-day retention. No app-data, cache, secrets or test files are packaged. This packaging path has not been executed for M1-B until a separate Windows run is approved. Automatic PR/main checks remain frontend-only; dispatch/label triggers are unchanged. There is no updater, autostart, registry setup, elevation request, telemetry, new runtime background service or release publication.

See docs/SMOKE_TEST.md in the source repository for the acceptance procedure. M1-A's earlier native launch evidence does not validate M1-B storage or this ZIP.
