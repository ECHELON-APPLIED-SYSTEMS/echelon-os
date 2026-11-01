# ECHELON OS

A modular Windows desktop operations platform for EVE Online.

Developed by ECHELON APPLIED SYSTEMS.

Development is in progress. NAVCOM now provides static minimum-hop routing after explicit loading of privately provisioned, integrity-checked official SDE data. No dataset is bundled and no live ESI connection is implemented. See [NAVCOM usage, ESI assessment and player validation](docs/NAVCOM_M2D.md).

## M1-D portable Appearance review

M1-B portable startup storage and M1-C's five neutral Graphite/accent themes are merged. This draft adds Rust-owned Appearance persistence in executable-adjacent `app-data/settings.json`, with strict v2 schema, byte-preserving v1 compatibility and atomic saves. Only theme ID is writable; workspaces still reset and NAVCOM static routing is documented separately in M2-D. See [Appearance persistence and validation limits](docs/APPEARANCE_PERSISTENCE.md).

Desktop **Appearance** restores the saved choice and shows Loading/Saving/Saved or a clear not-saved error with explicit retry. Browser preview remains temporary and never invokes native storage. Existing v1 empty preferences are upgraded only on a successful explicit save; corrupt/future/unknown data is never reset. No theme editor, arbitrary filesystem IPC, EVE services, credentials or unrelated features are introduced.

The repository retains automatic lightweight frontend and native-independent storage/packaging checks; full Windows validation remains explicitly requested only. Prior merged-baseline Windows CI and owner basic smoke do not validate M1-D. No native run, repeated manual smoke, release or merge is authorised by this implementation. Native metadata remains at the accepted M1-B baseline; this persistence PR does not imply a new native signoff.

## Windows prerequisites

- Windows 11 x64 with Microsoft Edge WebView2 Evergreen Runtime (normally included with Windows 11).
- Node.js **24 LTS** and its bundled npm; Git.
- Rust via [rustup](https://rustup.rs/), using the MSVC host. The repository pins **Rust 1.90.0**, rustfmt and Clippy in `rust-toolchain.toml`.
- Visual Studio 2022 Build Tools: **Desktop development with C++**, MSVC v143 x64/x86 tools and a Windows 10/11 SDK.
- Internet access for initial npm/Cargo downloads and Tauri's build dependencies. No application credentials or signing certificate are required.

Use a regular PowerShell terminal after installing prerequisites. Portable application development and launch do not require an elevated terminal; NSIS installation is retired.

## Clean checkout and development

```powershell
git clone https://github.com/ECHELON-APPLIED-SYSTEMS/echelon-os.git
cd echelon-os
# During review, check out the M1-B PR branch before installing.
npm ci
rustup show active-toolchain
npm run tauri -- dev
```

`tauri dev` starts Vite on `127.0.0.1:1420` and opens the native application. The overview should report **Desktop connected**, with application version, platform and build stage read from Rust. A port conflict stops startup rather than selecting an unexpected development origin.

For frontend-only work, `npm run dev` opens a browser preview at that address. It explicitly reports **Browser preview · Desktop connection unavailable**; preview is not evidence that native IPC works. `npm run preview` serves the production frontend after a build.

## Verification and unsigned Windows build

```powershell
npm ci
npm run lint
npm run format:check
npm run typecheck
npm test
npm run build
cargo fmt --manifest-path src-tauri/Cargo.toml --all --check
cargo clippy --manifest-path src-tauri/Cargo.toml --locked --all-targets -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml --locked --all-targets
npm run tauri -- build --no-bundle -- --locked
```

`npm run format` applies frontend/document formatting; Cargo formats Rust separately. `npm run test:watch` is available during development. Frontend tests exercise registry validation, availability views, metadata response validation and visible pending/error/retry states. Rust tests check the service, serialized command contract and IPC dispatch against the real generated ACL using Tauri's mock runtime: local main-window access is allowed, while another window, a remote origin and an unknown command are denied. These tests do not substitute for a real Windows WebView smoke test.

The executable is `src-tauri/target/release/echelon-os.exe`. All supported desktop builds now use executable-adjacent `app-data`, including development under the target tree; NSIS is no longer generated/supported. No installed/nonportable flavour remains, so the relative Tauri override is intentionally shared. Browser preview creates no native files. Startup fails recoverably before WebView creation if storage is unsafe, unwritable, corrupt or incompatible; it never silently resets settings or chooses another location.

These builds are unsigned; Windows may show expected publisher/SmartScreen warnings. No installer, registry setup or elevation is required. OS-provided Microsoft Edge WebView2 Evergreen is required and is not bundled or installed by this app. Signing, release publication and dependency-free runtime bundling are outside M1-B.

### Native-independent regression commands

The committed harness imports the actual Rust storage module without compiling Tauri/GTK/WebKit. With Rust 1.90.0 and PowerShell 7 installed:

```sh
cargo +1.90.0 test --manifest-path tools/storage-tests/Cargo.toml --locked
pwsh -NoLogo -NoProfile -File scripts/test-package-portable.ps1
```

All nine original Linux storage tests and additional cache/bounds/link regressions run in this target. Packaging checks use synthetic files/archives only, not a Windows application build. See [portable validation and scan-risk assessment](docs/PORTABLE.md). A bounded standard Ubuntu CI job executes these checks on the same automatic events as frontend checks. Full native validation remains on hold.

### Continuous and on-demand CI

`.github/workflows/windows.yml` runs lightweight frontend checks on standard `ubuntu-latest` for PR opened/synchronize/reopened events and pushes to `main`, including draft PRs: `npm ci`, lint, formatting, strict type checking, unit/component tests and production frontend build. These runs do not build installers or upload artifacts.

Full validation uses standard `windows-latest` only on an explicit `workflow_dispatch` request or a PR `labeled` event whose newly applied label is exactly `run-windows-build`. It preserves every frontend/Rust check and builds the unsigned EXE and portable ZIP. A label remaining on a PR does not trigger Windows builds on subsequent commits; a maintainer must deliberately apply it again for another requested build. Do not create/apply this label or request a full build without the relevant approval.

Before merge, a maintainer can explicitly request the full build by applying `run-windows-build` to the PR. Manual **Run workflow** / `workflow_dispatch` becomes available only after this workflow exists on the default branch; then select the intended ref explicitly. Neither a normal PR update nor a `main` push requests a Windows build. No full build is requested by this CI split.

Full builds use the tested packaging script with fresh owned staging and exact payload/archive allowlists to stage only the unsigned EXE and portable README in an extract-and-run `ECHELON-OS` folder, enforce a 50 MiB payload guard and upload `echelon-os-portable-windows-x64.zip` as a **one-day** artifact named `echelon-os-portable-windows-unsigned-<github.sha>`. No app-data is populated or packaged in CI. For PR events, the SHA is the generated test merge ref; verify its parents. Permissions remain read-only, with no publishing step or secrets. Standard public hosted-runner time is free, while artifact storage has a finite shared allowance; respect approved spending safeguards.

This packaging path is pending separate Windows build approval; no full run is triggered by M1-B. Historical M1-A NSIS evidence remains historical, and new NSIS acceptance is not required. See `docs/SMOKE_TEST.md` for pending portable runtime, data isolation, move/relaunch and recovery checks.

Linux frontend checks work with Node 24. Native Linux Tauri compilation additionally needs system development packages including GLib, GTK 3 and WebKitGTK 4.1. This project targets Windows; missing Linux packages must be reported as a validation limitation, not a passing Rust check.

## Structure and decisions

| Path                          | Responsibility                                                               |
| ----------------------------- | ---------------------------------------------------------------------------- |
| `src/shell/`                  | Application layout, accessible navigation, module hosting and runtime states |
| `src/core/modules.ts`         | Framework-independent generic manifest and immutable registry                |
| `src/modules/`                | Trusted module declarations; NAVCOM has no executable entry                  |
| `src/services/appMetadata.ts` | Typed IPC boundary and runtime validation of unknown responses               |
| `src-tauri/src/commands.rs`   | Single exposed read-only command                                             |
| `src-tauri/src/services/`     | Build metadata, independent of window/UI logic                               |
| `src-tauri/capabilities/`     | Explicit local main-window permission                                        |
| `docs/SMOKE_TEST.md`          | Reproducible Windows runtime and installer checks                            |
| `docs/HANDOVER.md`            | Append-only agent coordination protocol                                      |

Modules are statically imported by the trusted host. Each manifest has a stable ID, display metadata and availability; available modules require an entry and unavailable modules require an explanation. The generic registry has no React dependency and rejects invalid/duplicate entries. Only the shell adapter binds an entry to a React component. There is no discovery, external plugin loader or speculative marketplace.

Runtime dependencies are React/React DOM **19.3.1** and Tauri JS API **2.12.4**. Build tooling uses Vite **7.3.7**, TypeScript **5.9.3** in strict mode, Vitest **4.1.12**, and Tauri CLI **2.12.4**. The Rust lockfile selects Tauri **2.12.5** and tauri-build **2.7.4**. The JS API, CLI and Rust crates have independent upstream patch versions. A dev-tool override pins `minimatch` to patched version **11.0.3** for GHSA-g3hv-75wg-m249; lint and the full frontend suite verify compatibility with its ESLint/TypeScript ESLint consumers. Remove the override when those upstream ranges include the fix. Exact frontend dependency versions and both lockfiles are committed; use `npm ci` and Cargo `--locked`. Review deliberate dependency upgrades as code changes.

## Security and resource boundaries

- Windows application and test executables embed a Common Controls v6 manifest with `asInvoker` privileges. The manifest is linked to all targets because Tauri's default binary-only resource embedding leaves library unit-test executables without the v6 activation context. `embed-resource` was already a transitive build dependency; no runtime dependency was added.
- The read-only `get_app_metadata` command remains argument-free; `get_theme` and typed `set_theme` expose only the allowlisted Appearance ID. It returns compile-time name/version, platform and milestone. No process execution, arbitrary file access, credentials or network services are exposed to IPC. Rust performs narrowly scoped startup storage before opening a WebView; no general settings/filesystem write command exists.
- `build.rs` declares the command to Tauri's application ACL generator. The `main-metadata` capability grants only `allow-get-app-metadata`, `allow-get-theme` and `allow-set-theme` to the local `main` window. There are no core default permissions, remote capability origins, or shell/filesystem/HTTP plugins.
- Production CSP permits bundled scripts/styles and Tauri's local IPC transport (`ipc:` / `http://ipc.localhost`). Objects, frames and forms are blocked. Development additionally permits Vite's loopback HTTP/WebSocket, inline styles for CSS hot reload, and the specific SHA-256 hash of the pinned React refresh bootstrap. It does not allow arbitrary inline scripts. Vite dev/preview serve the same CSP headers, so browser checks exercise these policies; recheck the bootstrap hash when upgrading the React Vite plugin. There is no production `unsafe-inline` or `unsafe-eval`. Tauri may add its own asset hashes/nonces when embedding resources.
- The shell reads metadata once on mount or on explicit retry, without polling, background services, telemetry, external fonts, animations or GPU effects. React StrictMode can repeat effect setup in development.
- Graphite colours, muted teal focus, keyboard navigation, skip link, live status and visible error/empty states are included. Remote content is not loaded. An unavailable module opens an information view, not a simulated operational tool.

## Current limits

No detached/multiple-window management, workspace restoration, monitor recovery, SQLite, general settings editing, EVE SSO/ESI, live route intelligence, intelligence feeds, AI, overlays or game interaction is implemented. The shell's current selection is in memory only. Appearance alone uses strict version-2 preferences; old version-1 empty preferences are preserved until the first explicit successful save. Other preferences need separately approved validation/migration design. Diagnostic/packaging hardening and recovery signoff remain later work. Windows manual runtime validation must be recorded separately using the smoke plan; do not infer it from frontend tests, Rust unit tests or an installer build.

Coordination and scope approval remain in [Issue #1](https://github.com/ECHELON-APPLIED-SYSTEMS/echelon-os/issues/1). Follow `AGENTS.md` and `docs/HANDOVER.md`; Product Owner approval is required for merging and release.
