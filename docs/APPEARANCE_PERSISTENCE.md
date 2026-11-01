# M1-D — portable Appearance persistence

Desktop Appearance is owned by Rust and stored only in `app-data/settings.json` beside the executable. Browser preview remains temporary, with no native invoke, local/session storage, cookies, IndexedDB or alternate directory. All five IDs are allowlisted: `echelon`, `amarr`, `caldari`, `gallente`, `minmatar`. The shared neutral surfaces and accent palettes are unchanged.

## Schema and compatibility

New files use schema version 2. Missing `preferences.theme` means ECHELON default; saving a choice writes its ID:

```json
{
  "schema_version": 2,
  "preferences": { "theme": "amarr" }
}
```

Strict v1 files with exactly empty preferences are read without changing their bytes. They restore ECHELON and upgrade to v2 only on the first explicit, successful theme save. A failed save leaves the v1 bytes intact. There are no unknown fields to discard during this migration: extra v1 fields are rejected. Unsupported versions, malformed/oversized JSON, extra fields, explicit null or invalid/case-mismatched theme IDs fail closed without deletion/reset. A v2 document with no theme also defaults safely to ECHELON. Old v1-only binaries refuse v2; keep backups and use a compatible version rather than deleting settings to silence errors. No general migration/reset or settings editor is provided.

## Write protocol and trust boundary

Startup preserves the dedicated executable-relative root, reparse/link checks, bounded tree scan and real write/flush/delete probe before opening a WebView. The Rust store keeps an OS exclusive lock on the ordinary `.settings.lock` file for its lifetime, using Rust 1.90's file-lock API. A second cooperating instance or an unsupported lock operation fails closed; no unprotected fallback. The file stays after exit, but the OS releases its lock on handle/process exit, so a crash does not require deleting a stale lock file. Do not remove/replace the lock file or edit/move data while an instance is running. Older programs that do not honor this protocol are not made safe to run concurrently.

A mutex serializes reads/saves inside the process. Before each save the store revalidates root and settings paths/content and refuses readonly settings, missing/replaced files or external byte changes. Save stages a uniquely `create_new` file in the same root, writes complete strict JSON, flushes it, closes it and replaces the destination through a single same-directory operation. Windows uses `MoveFileExW(REPLACE_EXISTING | WRITE_THROUGH)` without COPY_ALLOWED; Unix uses rename. There is no delete-old-first, cross-volume copy or fallback directory. A failed commit leaves in-memory selection unchanged and preserves the original document. Initial creation remains no-replace. Owned temporary files are removed on ordinary failure; interrupted staging remains preserved and is never read as settings. New Unix temporary files are private (0600). Windows replacement files inherit the app-data directory ACL, rather than promising preservation of custom per-file DACLs; OS access checks and readonly attributes are not overridden. This file contains only a non-sensitive allowlisted preference, never credentials.

The same-user process/filesystem is trusted while running; checks and cooperative locks are not a sandbox against an uncooperative process replacing paths between checks. Real Windows ACL, junction/reparse, file-lock and rename behavior of this feature has not been tested in a native build. No guarantee against hardware/power-loss/filesystem failure is made: flushed-file atomic replacement is not a substitute for backups, especially on removable/network filesystems. Windows WRITE_THROUGH does not provide a universal hardware durability promise.

## IPC and UI

Only two custom commands are added: `get_theme()` returns the allowlisted ID, and `set_theme({theme})` saves that enum. They take no paths, arbitrary JSON or settings objects. The existing local-main-window capability grants only the two corresponding permissions in addition to metadata read; remote origins/other windows remain denied and CSP is unchanged. Errors sent over IPC are fixed recovery messages without absolute paths/raw OS details. Typed real-handler authorization tests are included but need Tauri native prerequisites to run.

The selector waits for desktop preference load before accepting edits. A missing/corrupt response or load failure disables selection and offers an explicit retry, without overwriting unknown settings. A choice changes visible accents immediately, shows Saving and disables further selection until the result arrives. Only acknowledged success shows Saved. On failure the visible choice remains session-only with an explicit not-saved message and retry; no automatic retry loop or false saved claim. This prevents rapid overlapping frontend writes; Rust also serializes concurrent callers. Unmount prevents stale UI updates but does not retract a native save already requested. Wait for Saving to finish before closing; interruption can leave the previous or committed value, never a deliberately partial JSON file.

## Automated evidence and remaining limits

Run from the repository root:

```sh
cargo +1.90.0 test --manifest-path tools/storage-tests/Cargo.toml --locked
cargo +1.90.0 clippy --manifest-path tools/storage-tests/Cargo.toml --locked --all-targets -- -D warnings
npm run lint
npm run format:check
npm run typecheck
npm test
npm run build
```

The committed native-independent harness imports the production storage module, using existing locked serde dependencies. Tests cover v1 byte-preserving load/lazy upgrade and failed upgrade, default/absent choice, every ID save/reload, invalid/null/corrupt/future settings, readonly refusal, redirected root/settings rejection, failed replacement preserving disk bytes/cache, external edits, lock exclusion/release, concurrent writers and move/reload. These run on Linux, not a proof of Windows behavior. Frontend tests cover restoration, saving acknowledgment, overlapping-change suppression, honest failure/retry, invalid native response refusal, preview nonpersistence and existing theme/accessibility/contrast/navigation contracts. Full Tauri dispatch/ACL tests and Windows-specific branches remain unrun for M1-D until separately authorized prerequisites/testing are available.

The merged M1-B/M1-C Windows validation and owner smoke are historical baseline evidence, not M1-D persistence acceptance. No discretionary full Windows build, label/dispatch, repeated manual user smoke, merge or release is requested here. The Owner explicitly waived repeating a manual smoke solely for this feature; any newly demonstrated native blocker should be reported for a separate decision. Advanced Windows/OS/profile/DPI/assistive-tech and power-loss guarantees remain unverified.
