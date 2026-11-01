# Private portable NAVCOM snapshot boundary (M2-C)

M2-C provides the accepted read-only plumbing. [M2-D](NAVCOM_M2D.md) now
invokes it from an explicit NAVCOM load action; no automatic startup load is added.
Absence of a snapshot is normal and does not trigger file creation, download,
conversion, a fallback directory, or app startup failure. No production game data
is included in source, fixtures, CI, artifacts, or application packaging.

## Trust and explicit call boundary

The only native command is argument-free `get_navcom_snapshot`. The generated
application ACL grants it solely to the existing local `main` window. Other
windows and remote origins remain denied; no generic filesystem, network or shell
permission is added and CSP is unchanged. The command uses the current validated
`PortableStore::root()` and exactly `navcom/snapshot.json` beneath it. Caller paths,
CWD, home, AppData and temporary folders are never consulted as data locations.

The Rust service checks the bounded ancestor chain, app-data, navcom and final
file for links/reparse points and nonregular types. It checks the 4-MiB byte limit
before open, on the opened handle, and while reading at most limit + 1 bytes.
Invalid UTF-8, unreadable, oversized, unsafe or observed changed paths produce
sanitized typed errors without OS paths. Windows uses OPEN_REPARSE_POINT,
BACKUP_SEMANTICS for directories, retained directory handles denying rename/delete,
and a snapshot handle denying write/delete for the bounded read. Linux uses
O_NOFOLLOW/O_NONBLOCK, metadata device/inode comparisons and post-read directory
checks. There is no per-query recursive tree scan and no write/create operation.

Same-user filesystem trust matches existing portable storage. This is not a
sandbox against an adversarial same-user process; it rejects known links and
observed replacements, not a formal proof against every concurrent mutation.
There is no network-path fallback. Existing PortableStore startup validation and
its handling of unsafe entries anywhere in app-data are unchanged; that pre-existing
whole-tree safety check can still reject an unsafe provisioned link at startup.
This service does not parse JSON natively: valid UTF-8 is only transport text,
not a trusted or ready graph.

`readNavcomSnapshot()` in `src/services/navcomSnapshot.ts` is **explicitly called
only**, with no cache or polling. Browser preview returns `preview` without invoking
native storage. Native absence returns `missing`; other failures return safe error
codes. Text is passed through the existing M2-B SHA256/schema/topology validator
before `ready` can expose a graph. Invalid/tampered/unsupported data fails closed.

The compiled, deeply frozen metadata-only `NAVCOM_PIN` contains the reviewed M2-B
build **3662168**, hashes and counts from `docs/SDE_IMPORT.md`. Neither snapshot
content nor a neighboring mutable `provenance.json` supplies trust. Pin refreshes
require a separately approved and reviewed source change. The pin proves integrity
against reviewed bytes; it is not a CCP signature, automatic latest-build check,
license assurance, or permission to distribute data. The adapter preserves M2-A
shortest-hop/avoid semantics; routes are not safety or real-time claims.

## Strictly manual developer provisioning (not performed by this change)

1. Read [SDE_IMPORT.md](SDE_IMPORT.md) and the current
   [CCP Developer License Agreement](https://developers.eveonline.com/license-agreement).
   The Owner must ensure authority, conditional non-commercial application use,
   required proprietary notices, non-affiliation and all applicable obligations.
   No game data publication/bundling/release is authorised by this milestone.
2. Only when separately authorised, obtain the pinned official archive and run
   the M2-B converter in a private directory **outside all source checkouts**.
   Review archive/build/table/snapshot digests and counts against the compiled pin.
   A newly generated different build is unsupported until a separately approved
   pin source change. Never copy a co-located provenance file as a trust override.
3. Use a complete standalone extracted app folder **outside every public Git
   checkout**, after the relevant executable build has been separately authorised
   and validated. Do not provision real data into `src-tauri/target`, a development
   checkout, CI cache/artifact, PR attachment or release package. Close the app.
   Preserve a backup of the entire portable folder and existing data first.
4. In that standalone folder's existing executable-adjacent `app-data`, manually
   create an ordinary `navcom` directory and copy **only** the reviewed
   `snapshot.json` to:

   ```text
   <standalone-executable-directory>/app-data/navcom/snapshot.json
   ```

   Use regular local folders/files, not symlinks, junctions, reparse points or
   network locations. Do not overwrite existing data blindly. No provisioner,
   installer, updater, filesystem picker or automatic copy is implemented here.

5. Keep the complete portable folder together when moving/relaunching it,
   including app-data. No absolute path is saved in settings. A future explicitly
   approved caller can request the load; M2-D provides the explicit Load route data action.

`missing` means the fixed child directory/file is absent, not that bad data was
silently discarded. `unsafe-path`/`changed` require closing the app and preserving
and inspecting the complete folder for redirection/replacement; `unreadable`
requires checking ordinary access and other processes. `oversize`/`invalid-utf8`
or `invalid-snapshot` require verifying the private original and pinned conversion.
Do not reset, clear, silently regenerate, fetch latest, or accept a new user pin.
The service never overwrites or repairs a failed file. Existing Appearance settings
and schema are unchanged.

## Validation boundaries

The native-independent harness compiles the actual production reader and portable
module. Synthetic tests cover absent directory/file without creation, exact 4-MiB
and oversized input, growth before open, invalid UTF-8, malformed UTF-8 text passed
only as untrusted transport, nonregular file, relocation, preserved settings/data,
Unix denied access, links, FIFO and swapped file/parent paths. Windows-only junction
test and actual generated Tauri mock ACL tests are included but require native
prerequisites. The ACL test covers local main missing/text responses and denied
remote/secondary windows.

TS service tests cover preview/missing/error states, strict wire responses,
explicit no-argument invoke, use of the compiled pin, validation failure and no
cache. The ready-transport test mocks the validator with a fictional graph;
existing `snapshot.test.ts` separately executes real SHA256/graph validation with
its synthetic test pin. No synthetic fixture masquerades as the production pin.

This Linux environment lacks GTK 3/WebKitGTK 4.1 native dependencies; therefore
Tauri mock dispatch and Windows reparse/handle behavior remain unexecuted here.
Linux module tests and frontend tests do not establish native Windows runtime
acceptance. ChatGPT/Owner must decide whether a narrowly scoped native Windows
validation run is required before accepting first-use IPC/Windows path behavior.
No Windows build, dispatch or label is preauthorised or triggered by this work.
