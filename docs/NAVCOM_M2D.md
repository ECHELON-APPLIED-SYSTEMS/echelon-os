# NAVCOM M2-D: genuine static routing interface

NAVCOM now opens a usable static stargate route planner. Availability means the
interface is installed; it does **not** mean a dataset or live EVE connection is
present. The existing validated private snapshot is its sole topology source.
No production fixtures, CCP dataset, runtime downloader, ESI request, SSO or
credential storage is added. The native reader, compiled pin, portable filesystem
protections, IPC grants and CSP are unchanged from accepted M2-C PR #8.

## What works

Open NAVCOM and explicitly choose **Load route data**. Nothing reads the snapshot
on app startup or simply opening the module. The existing native read boundary
and SHA256/build/schema/topology validator must succeed before selection appears.
Browser preview, missing data, loading, unsafe/unreadable/unsupported data and
validation failures have distinct honest states and explicit retry.

Search origin and destination by English name or genuine numeric system ID, then
select a result. Search is bounded to 20 matches and 64 characters; duplicate
names include IDs, and systems without English labels remain selectable by ID.
The adapter exposes frozen search results rather than a mutable data map.
Calculate a minimum-hop route with the existing deterministic M2-A engine. Results
show names, genuine IDs, ordered steps and jumps, including same-system and
no-static-route outcomes. Endpoint edits invalidate the old result. Reloading
clears the prior graph/route before validation, including if the new read fails.
Long routes display 100 steps at a time. All selections are memory-only.

The UI labels the concrete SDE build and local validation time. Local validation
time is **not** CCP data publication time or a live refresh. Current compiled pin
is build **3662168** (official release metadata **2026-10-30T11:21:02Z**); its
reviewed digests/counts are recorded in [SDE_IMPORT.md](SDE_IMPORT.md). Genuine
static systems/gates can differ from current game state. Shortest hops do not
imply safety; dynamic wormholes/Ansiblex, current events and live risk are absent.
Routes are not transmitted to EVE and no game action occurs.

## Owner validation inside EVE Online

Practical acceptance is player-led using genuine systems, not fictional gameplay
simulations. Synthetic tests cover regressions and security independently.

1. Use a desktop build containing this PR. The older M2-C executable does **not**
   contain this interface. No Windows build was triggered for M2-D; a suitable
   desktop build must be separately arranged/approved for practical testing.
2. Privately generate the reviewed snapshot using the existing converter and
   verify its metadata against the app's pin. Follow
   [NAVCOM_PROVISIONING.md](NAVCOM_PROVISIONING.md) and [SDE_IMPORT.md](SDE_IMPORT.md).
   Place only the reviewed snapshot at
   `<standalone-executable-directory>/app-data/navcom/snapshot.json` in an ordinary
   extracted app folder **outside every public source checkout**. Never place
   actual data in `src-tauri/target` in a checkout, Git, CI caches/artifacts, PR
   attachments or public logs. No dataset distribution is authorised by this PR.
3. In EVE, note your actual current system. Enter/select that name or ID manually
   in NAVCOM; select a real destination. Compare ordered steps and jump count to
   EVE with **shortest-route** preferences and matching avoidance settings. This
   increment has no avoid controls: remove avoids for a comparable baseline. A
   different shortest tie is possible. Do not expect safest-route preference,
   wormholes, Ansiblex or changes newer than the pinned snapshot to match.
4. Check a same-system destination, a destination change, and a real disconnected
   pair if relevant to your play. Confirm no stale route remains after endpoint
   edits or failed reloads. Change Appearance while the route is open; theme and
   current route should coexist. Leaving NAVCOM resets its memory-only selection.
5. For a reported mismatch, record privately the build, game route preferences,
   avoids, endpoint IDs and observed outcome. Do not paste copied route tables or
   datasets into public GitHub. Investigate an actual discrepancy rather than
   requiring repeated generic manual smoke runs.

Missing/invalid data is not repaired, cleared, fetched or silently substituted.
The owner retains all merge, executable release and CCP data-distribution rights
and approval decisions, including then-current CCP license/notice compliance.

## Read-only ESI assessment

The first increment needs genuine topology and names already supplied by SDE;
adding a second network source is unnecessary for minimum-hop calculation. ESI
is assessed here, not represented as implemented or connected. Current official
[ESI overview](https://developers.eveonline.com/docs/services/esi/overview/) separates
public endpoints from authenticated routes and specifies compatibility dates.
Before implementing a future adapter, verify exact current schemas/scopes in the
[API Explorer](https://developers.eveonline.com/api-explorer).

| Information                  | Appropriate ESI surface                                      | Boundary / next increment                                                                                                                                             |
| ---------------------------- | ------------------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Public system details        | `GET /universe/systems/{system_id}/`                         | Suitable next read-only detail view. IDs/names already come from SDE; don't replace trusted topology with an unvalidated response.                                    |
| Batch public names           | `POST /universe/names/`                                      | Read-only lookup despite POST. Not needed for this UI because approved English names already exist locally.                                                           |
| Current character location   | `GET /characters/{character_id}/location/`                   | Requires owner-approved EVE SSO and `esi-location.read_location.v1`; not obtainable as public character information. Deferred; no token or credential handling added. |
| Future activity/risk context | `GET /universe/system_jumps/`, `GET /universe/system_kills/` | Cached aggregates, not instantaneous safety or a complete threat picture. No risk score or safest-route claim in this increment.                                      |

A future public adapter should use explicit user requests, fixed endpoint/host
allowlists, bounded time/response size, strict schemas and known system IDs. Use
a deliberately reviewed `X-Compatibility-Date`, identify the application, honor
ETag/Expires/cache/error-budget/rate-limit headers and show fetched/as-of/expiry
information, including stale/error states. Do not turn cached observations into
instantaneous claims or silently mix a newer response into pinned static edges.

Current runtime CSP and native permissions do not grant arbitrary external
networking. A future public ESI implementation needs a separately reviewed narrow
transport/CSP decision. SSO, scopes, token lifecycle and any credential persistence
require separate approval. This PR does not make those changes.

## Evidence and remaining limits

Focused automated tests exercise the actual snapshot validator, engine, immutable
bounded search, route rendering, honest preview/missing/error states, invalidation,
and theme preservation with synthetic data. They are regression coverage, not a
fictional owner acceptance prerequisite.

For this increment, the official pinned ZIP was privately fetched and verified
against the existing archive hash, then converted outside Git. The resulting
snapshot hash matched the compiled pin. Chromium loaded the actual UI using this
genuine dataset through a **simulated IPC bridge**, exercising real SHA256/schema
validation, name/ID selection, ordered two-hop route, theme retention and narrow
viewport layout. No system names/IDs/edges/route lists were printed or committed.
The ordinary browser preview separately refused native data as designed.

This demonstrates genuine-data frontend functionality, not a real Windows
WebView/native IPC end-to-end run or in-game player acceptance. Native M2-C was
previously accepted on Windows; this PR changes no Rust/IPC/security code. No
additional Windows build, release, data packaging or owner manual smoke run was
triggered. Temporary official data and the browser harness are deleted after
validation; they are not shipped as a demo or runtime fallback.
