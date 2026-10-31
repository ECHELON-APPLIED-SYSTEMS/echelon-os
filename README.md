# ECHELON OS

A modular Windows desktop operations platform for EVE Online.

Developed by ECHELON APPLIED SYSTEMS.

Development is in progress. NAVCOM is planned and not yet functional.

## M1-A private review stage

The feature branch stages the desktop foundation: a graphite shell, static typed module registry and read-only desktop metadata connection. NAVCOM remains planned and nonfunctional; this build does not connect to EVE or provide navigation.

The repository remains private and Product Owner reports GitHub Actions disabled. The included workflow describes future checks only; do not enable or run CI without separate approval. Fresh target CI and Windows native runtime/installer smoke validation remain outstanding. This migration is an unmerged draft for independent review, not acceptance or release.

## Windows prerequisites

- Windows 11 x64 with Microsoft Edge WebView2 Evergreen Runtime (normally included with Windows 11).
- Node.js **24 LTS** and its bundled npm; Git.
- Rust via [rustup](https://rustup.rs/), using the MSVC host. The repository pins **Rust 1.90.0**, rustfmt and Clippy in `rust-toolchain.toml`.
- Visual Studio 2022 Build Tools: **Desktop development with C++**, MSVC v143 x64/x86 tools and a Windows 10/11 SDK.
- Internet access for initial npm/Cargo downloads and Tauri's NSIS tooling. No application credentials or signing certificate are required.

Use a regular PowerShell terminal after installing prerequisites. Application development and the per-user installer do not require an elevated terminal.

## Clean checkout and development

```powershell
git clone https://github.com/ECHELON-APPLIED-SYSTEMS/echelon-os.git
cd echelon-os
# During review, check out the M1-A PR branch before installing.
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
npm run tauri -- build --bundles nsis --no-sign -- --locked
```

`npm run format` applies frontend/document formatting; Cargo formats Rust separately. `npm run test:watch` is available during development. Frontend tests exercise registry validation, availability views, metadata response validation and visible pending/error/retry states. Rust tests check the service, serialized command contract and IPC dispatch against the real generated ACL using Tauri's mock runtime: local main-window access is allowed, while another window, a remote origin and an unknown command are denied. These tests do not substitute for a real Windows WebView smoke test.

The executable is `src-tauri/target/release/echelon-os.exe`; the per-user NSIS installer is under `src-tauri/target/release/bundle/nsis/`. These builds are unsigned; Windows may show the expected untrusted publisher/SmartScreen prompt. Signing, releases and publishing are outside M1-A. The installer can download Microsoft's WebView2 bootstrapper if WebView2 is missing; application startup itself makes no service calls.

When separately authorised and enabled, `.github/workflows/windows.yml` is configured to run every listed check on `windows-latest` for pull requests and pushes to `main`, including draft PRs. It builds the application and NSIS installer and uploads both as a seven-day artifact named `echelon-os-windows-unsigned-<commit SHA>`. Missing artifacts fail the upload step. The workflow has read-only repository permissions, no signing/publishing step and no secret requirements. See the actual PR Actions run for results; a configured workflow is not a successful build.

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
- Only `get_app_metadata` is exposed, with no caller arguments. It returns compile-time name/version, platform and milestone. No process execution, file access, credentials or network services are exposed.
- `build.rs` declares the command to Tauri's application ACL generator. The `main-metadata` capability grants only `allow-get-app-metadata` to the local `main` window. There are no core default permissions, remote capability origins, or shell/filesystem/HTTP plugins.
- Production CSP permits bundled scripts/styles and Tauri's local IPC transport (`ipc:` / `http://ipc.localhost`). Objects, frames and forms are blocked. Development additionally permits Vite's loopback HTTP/WebSocket, inline styles for CSS hot reload, and the specific SHA-256 hash of the pinned React refresh bootstrap. It does not allow arbitrary inline scripts. Vite dev/preview serve the same CSP headers, so browser checks exercise these policies; recheck the bootstrap hash when upgrading the React Vite plugin. There is no production `unsafe-inline` or `unsafe-eval`. Tauri may add its own asset hashes/nonces when embedding resources.
- The shell reads metadata once on mount or on explicit retry, without polling, background services, telemetry, external fonts, animations or GPU effects. React StrictMode can repeat effect setup in development.
- Graphite colours, muted teal focus, keyboard navigation, skip link, live status and visible error/empty states are included. Remote content is not loaded. An unavailable module opens an information view, not a simulated operational tool.

## M1-A limits

No detached/multiple-window management, workspace restoration, monitor recovery, SQLite, persistent settings, EVE SSO/ESI, route calculations, intelligence feeds, AI, overlays or game interaction is implemented. The shell's current selection is in memory only. Diagnostic/packaging hardening and recovery signoff remain later work. Windows manual runtime validation must be recorded separately using the smoke plan; do not infer it from frontend tests, Rust unit tests or an installer build.

Coordination and scope approval remain in [Issue #1](https://github.com/ECHELON-APPLIED-SYSTEMS/echelon-os/issues/1). Follow `AGENTS.md` and `docs/HANDOVER.md`; Product Owner approval is required for merging and release.
