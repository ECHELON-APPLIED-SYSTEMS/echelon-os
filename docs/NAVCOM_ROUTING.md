# NAVCOM offline routing core (M2-A)

`src/modules/navcom/routing.ts` is an independent, dependency-free domain core.
It has no network, filesystem, persistence, React or native IPC access. It is not
connected to the desktop shell: NAVCOM remains **planned**, with route planning
unavailable. No actual New Eden topology is bundled; all artificial graphs live
only in unit tests.

## Boundary and outcomes

`createStargateGraph(systemIds, edges)` accepts unknown plain data: an array of
unique positive safe-integer system IDs and an array of two-ID undirected stargate
pairs. It copies validated inputs into private adjacency lists. Empty graphs and
isolated systems are valid. Missing endpoints, self-edges, duplicate systems,
malformed IDs/pairs and duplicate edges (including reversed pairs) throw
`RoutingInputError` with a stable `code`; corrupted topology is never repaired.
Structural validation cannot establish whether a connection is real or current.

The returned graph exposes only `shortestRoute(origin, destination, avoidIds?)`.
Avoid IDs are a mathematical set represented by an optional bounded array of
distinct known IDs (default empty); duplicates are rejected. Unknown or invalid
origin/destination/avoid IDs throw, including on same-system requests. All input
validation precedes outcome selection. An avoided endpoint returns
`{ kind: 'no-route', reason: 'avoided-endpoint' }`, even if both endpoints match.
Otherwise identical endpoints return `same-system`, a one-ID path and zero jumps.
A reachable destination returns `route`, ordered IDs including both endpoints,
and `jumps = systems.length - 1`; unreachable destinations return `no-route` with
reason `disconnected`. Result objects and path arrays are frozen. Repeated calls
have independent traversal state; caller arrays are never changed or retained.

Breadth-first search visits ascending numeric neighbors, selecting the
lexicographically smallest ID path among equal minimum-hop routes, independent
of input order or edge orientation. Cycles cannot cause repeated visits. The
implementation uses a queue index and iterative path reconstruction, not
recursion. Hard limits are 10,000 systems, 50,000 edges and 10,000 avoid entries;
length limits are checked before element traversal. These are resource budgets,
not promises about future universe sizes. With V systems and E edges, validation
uses O(V + E) space and sorting up to O(E log V) time; a query uses O(V + E) time
and O(V) space. Inputs must be inert plain data, not hostile executable JavaScript
getters/proxies/custom iterators; no data parser can sandbox caller code.

## Limits and future data boundary

Minimum hops is **not** a safest route, security rating, threat estimate or promise
of matching in-game routing. There are no wormholes, Ansiblex links, risk scores,
character tracking, waypoints, authentication or live services.

A future separately approved static-data adapter must verify source provenance,
version/completeness, system identity and reciprocal stargate relationships before
passing plain ID/pair arrays into this boundary. No provider framework or importer
is implemented. ESI/service-provided routes would be a separate adapter with its
own validation and freshness semantics, not implicit edges in this graph. See
[EVE route calculation background](https://developers.eveonline.com/docs/guides/route-calculation/);
it does not validate our synthetic test routes or supply production topology.

Tests cover small synthetic graphs, deterministic ties, exclusions, invalid data,
immutability and a maximum-system-count chain. No native build or live-route
validation is needed or claimed for this standalone core.
