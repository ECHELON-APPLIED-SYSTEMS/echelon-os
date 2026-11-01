# M2-F — session-only EVE connection

This draft adds native-owned EVE SSO and explicit location/destination controls to NAVCOM. Synthetic tests cover the shared Rust service and frontend. **Windows browser/callback, Tauri command dispatch/ACL, native TLS and real character/waypoint behavior have not been validated.** The next native validation needs separate Product Owner approval after independent source and lightweight CI review. This document is not approval to sign in, send real requests, build Windows, merge or release.

## Registered public client

Owner-reported CCP registration, authorised in [M2-F-R1](https://github.com/ECHELON-APPLIED-SYSTEMS/echelon-os/issues/1#issuecomment-6564765916):

- Application: ECHELON OS
- Public client ID: `49be7fb9afec4015a78f6034d126db8e` (public metadata, not a secret)
- Exact registered redirect: `http://localhost:43173/oauth/callback`
- Scopes: `esi-location.read_location.v1 esi-ui.write_waypoint.v1`, and no others

The native listener binds **IPv4 `127.0.0.1:43173` only**, but authorize and token requests use the exact **localhost** redirect. Host must be `localhost:43173`, path must be `/oauth/callback`. No substitute redirect or port fallback. IPv6-first localhost resolution/browser fallback and Windows listener exclusivity still need practical validation. An occupied port fails before discovery/browser launch. Sign-in has a 120-second callback deadline, bounded reads (8 KiB, two-second total per connection), at most 16 rejected connections and a single valid callback. Cancel/disconnect invalidates the attempt. Wrong state, host, port, path, duplicate parameters, denial/code combinations, request bodies and malformed requests fail closed; a valid denial also consumes the callback.

Connect is explicit and opens only the native-generated CCP HTTPS authorization URL in the external OS browser using `ShellExecuteW`; no shell, arbitrary URL IPC, embedded browser or opener permission. PKCE uses 32 random bytes, S256 and a distinct random state. Windows system browser, DNS, firewall, endpoint protection and concurrent instance behavior remain practical validation concerns.

## Trust and credential boundary

Only these native, compile-time endpoints are used:

- `https://login.eveonline.com/.well-known/oauth-authorization-server`
- `https://login.eveonline.com/v2/oauth/authorize` (OS browser only)
- `https://login.eveonline.com/v2/oauth/token`
- `https://login.eveonline.com/oauth/jwks`
- `https://esi.evetech.net/characters/{verified_character_id}/location`
- `https://esi.evetech.net/ui/autopilot/waypoint`

Discovery must match pinned issuer/endpoints and support S256. JWT verification accepts RS256 only, one matching trusted RSA signature key, a valid signature, approved EVE issuer spellings, both public client ID and `EVE Online` audiences, expiry, optional not-before/authorized party, a canonical positive `CHARACTER:EVE:<id>` subject, name and exactly the two approved scopes. Missing/rotated unsupported keys or algorithms fail closed. No token-supplied URL is fetched. A token lifetime over 24 hours is refused; actual session expiry is the earlier of verified JWT expiry and token response lifetime. There is no refresh-token flow: any returned refresh token is discarded, and the owner reconnects after expiry.

Access token, verifier and callback code remain native memory only. There is no token/credential IPC, application logging, credential file, settings change, WebView storage, cookie jar or HTTP disk cache. Session-owned secret strings and response/callback buffers use zeroization; parser, HTTP/TLS/OS/browser internal memory is outside this erasure guarantee. Disconnect drops native credentials, identity, location and pending confirmation; a local expiry worker also clears them and exits on logout. In-flight request copies can remain until their bounded request completes (up to 20 seconds). Disconnect cannot undo a request already sent or revoke CCP account consent. Browser/account consent storage is controlled by CCP and the owner's browser, not this application.

Native requests use HTTPS, normal inherited proxy/native certificate trust, no redirects, no automatic retries, 10-second connection/20-second total limits and a 128 KiB response cap. Only bounded cache/rate headers are retained. No application CSP, portable settings, generic network/shell/filesystem grant or credential persistence permission is added. Typed IPC commands are generated into the application ACL and allowed only for the local `main` window: status, connect, disconnect, refresh location, prepare waypoint, send confirmed waypoint. Status/expiry timers do not initiate HTTP. One native operation is in flight at a time; disconnect invalidates late responses even while sign-in is pending.

## Public schema preflight and cache semantics

Official [SSO documentation](https://developers.eveonline.com/docs/services/sso/), [API Explorer](https://developers.eveonline.com/api-explorer), public discovery/JWKS and ESI OpenAPI were reviewed through the permitted environment proxy. Public discovery advertised S256 and the exact endpoints above; JWKS supplied an RS256 signature key. No login, authorization code, private token, character request or waypoint request was used.

The public ESI schema's supported compatibility date was **`2026-08-18`** (returned in its version/header and compatibility parameter enum). Runtime ESI requests pin `X-Compatibility-Date: 2026-08-18`; this is reviewed metadata, not a guessed current date. API evolution needs a future source review. The schema specifies:

| Operation                                 | Permission / meaning                                                                              |
| ----------------------------------------- | ------------------------------------------------------------------------------------------------- |
| GET `/characters/{character_id}/location` | location scope; `solar_system_id`, optional station/structure IDs; cache hint 5 seconds           |
| POST `/ui/autopilot/waypoint`             | waypoint scope; query `destination_id`, `clear_other_waypoints`, `add_to_beginning`; 204 accepted |

Location is requested only by **Refresh current location**. Cache-Control/Expires, Age/Date, ETag/304 and Last-Modified are honoured conservatively; a missing freshness policy means stale immediately. Response fetch time, server observation time (when available), cache reuse, freshness end and staleness are shown separately. An ESI response is never described as instantaneous. Explicit refresh can reuse an unexpired native cache; it does not force CCP's cache to refresh. Retry-After/error-budget exhaustion blocks requests until its bounded retry time; no automatic retries or polling. Authentication/permission failures clear the session; malformed/unavailable responses show safe errors and no invented location.

**Use Current System as origin** is explicit, requires a fresh-enough observation and membership in the loaded validated graph, and preserves the manually selected destination. An absent/unknown system is refused. The static graph never updates from ESI. Manual offline autocomplete and minimum-hop routing remain independent of sign-in.

## Confirmed waypoint writes

Choose a destination from the validated local graph, then **Set Destination…** or **Add Waypoint…**. A native-issued confirmation displays the connected character's exact name/ID, destination name/ID and queue behavior. A separate **Confirm EVE request** is required within 30 seconds. Cancel, selection change, logout or expiry removes the displayed confirmation. Native confirmation is bound to the verified session/character, destination and action, single-use and consumed before HTTP; replay/double submission is refused. Renderer-supplied character IDs, URLs and write flags are not accepted.

The native boundary re-reads the existing safe fixed snapshot path and checks its SHA-256 against the same approved M2-B build-3662168 pin before preparation **and** before sending. The destination must be a system in those pinned bytes; arbitrary stations/structures and modified/missing data are refused. This preserves directory identity/junction/reparse checks and does not download or distribute the dataset. Frontend graph validation remains unchanged; native ID authorization relies on the exact separately reviewed byte pin.

| Action          | `clear_other_waypoints`            | `add_to_beginning` |
| --------------- | ---------------------------------- | ------------------ |
| Set Destination | `true` — clears existing queue     | `false`            |
| Add Waypoint    | `false` — preserves queue, appends | `false`            |

Only the selected destination is sent, once. No intermediate route, movement or automatic flight. 204 displays **request accepted by ESI**, not proof that the EVE client applied it. Timeout/transport failure, oversized/unusable response or server failure after sending has an unknown outcome: check EVE before preparing another request. An already sent request may complete after disconnect. There is no automatic retry.

## Dependencies and verification

New direct dependencies are pinned in both Cargo manifests/lockfiles: `reqwest 0.12.28` (blocking native HTTP; rustls/native CA roots, no default cookie/compression features), `jsonwebtoken 9.3.1` (ring-backed RSA verification; PEM defaults disabled), `getrandom 0.3.4`, `sha2 0.10.9`, `base64 0.22.1` (PKCE), `zeroize 1.8.2` (owned secrets) and `httpdate 1.0.3` (cache/rate dates). Existing native locked versions are retained; newly required transitive dependencies use registry checksums. No frontend dependency or broad Tauri plugin was added. Rust 1.90 compiles the actual service in the native-independent harness; this is not a dependency vulnerability-audit certification or a Windows build.

The committed DER/JWKS pair is an explicitly **synthetic test-only RSA key**, not a CCP signing key, OAuth client secret or account credential. Only tests include it; generated mock JWTs stay in test memory. Mock transports and a real local loopback callback exercise security/failure boundaries without CCP credentials or datasets. Native Tauri ACL tests use the real generated application capability but require native prerequisites; they have not run in this Linux environment. Normal Ubuntu CI runs frontend tests plus the same actual Rust modules in the storage harness, with packaging regressions; it produces no Windows application.

After separate approvals, owner-controlled native acceptance can verify one real sign-in with the two scopes, character identity, explicit cached location/graph-validated origin, independently preserved offline routing, cancel/logout/expiry, and one carefully reviewed request per waypoint action in EVE. Compare queue clearing/appending and client application directly. Do not expose real credentials/character details in GitHub, CI or handovers. These are pending validation steps, not claims of completed acceptance.
