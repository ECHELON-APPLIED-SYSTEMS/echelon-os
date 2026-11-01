# Local SDE topology preparation

M2-B adds an explicit offline developer tool and a framework-independent adapter.
NAVCOM remains Planned. No data, importer, network request or snapshot load runs
at application startup, npm installation, frontend build or CI. CI tests use only
fictional synthetic ZIP/JSONL records. No production snapshot is bundled.

## Data boundary and rights

Never commit or upload original archives, extracted tables, generated snapshots,
real route lists or system-name tables to GitHub, PR attachments, logs, CI caches
or artifacts. Use a private directory **outside every public source checkout**,
with mode 0700 and output files 0600. Do not rely on `.gitignore` alone. The CLI
rejects input/output paths inside this checkout; the developer must also avoid
other repositories, shared directories and synced/uploaded locations. Clean up
private temporary data once validation is complete; do not stage with `git add .`.

The Owner approved this technical architecture, not a legal opinion or public
data release. Read the current [CCP Developer License Agreement](https://developers.eveonline.com/license-agreement)
before accessing data. Access/download carries agreement obligations; the Owner
must ensure the responsible developer has authority and meets its requirements.
Section 1.13 and Section 4 constrain the Purpose to non-commercial EVE support
(subject to the agreement's specific exceptions). Section 2.1 grants conditional
use/distribution **within an Application**, not a blanket source-data license.
Section 7.1 requires retaining proprietary notices, substantially:

> © 2014 CCP hf. All rights reserved. "EVE", "EVE Online", "CCP", and all related
> logos and images are trademarks or registered trademarks of CCP hf.

Do not imply CCP affiliation (Section 2.7), combine marks without required consent,
or ignore removal/termination obligations (Sections 2.2 and 6). Any later controlled
application-bundle process requires separate Owner release approval, verification
of then-current terms/notices, non-commercial compliance and CCP clarification of
material uncertainties. No such bundling or publication is implemented here.

## Explicit local workflow

Requires Python 3.9+ standard library; Node 24 for the adapter/tests. No new package
or workflow dependency is installed. The importer itself has **no download mode**.

1. Read [official SDE documentation](https://developers.eveonline.com/docs/services/static-data/).
   Resolve `https://developers.eveonline.com/static-data/tranquility/latest.jsonl`
   once, then pin the concrete `sde.buildNumber`. Obtain the corresponding
   `https://developers.eveonline.com/static-data/tranquility/eve-online-static-data-<build>-jsonl.zip`
   through normal proxy/TLS verification in a separately authorised developer step.
   Bound download time and size; never use mirrors or runtime/CI downloads.
2. Record the archive SHA256 from that trusted retrieval separately. A SHA256
   supplied by an untrusted party is not proof of CCP origin. There was no
   independently signed upstream checksum in this workflow; HTTPS source plus
   recorded bytes/hash and in-archive build agreement establish the observed provenance.
3. Choose a **new** output directory under a private parent outside source control:

   ```sh
   umask 077
   mkdir -p /tmp/echelon-private-sde
   chmod 700 /tmp/echelon-private-sde
   python3 scripts/sde/convert.py --archive /tmp/echelon-private-sde/source.zip \
     --build <concrete-build> --sha256 <recorded-archive-sha256> \
     --output /tmp/echelon-private-sde/new-output
   ```

   Existing output directories are always refused, including symlinks. There is
   no overwrite flag: select a new directory and compare outputs before retiring
   old data. Failure never replaces an existing snapshot. Partial new output on
   filesystem failure must be discarded, not trusted.

4. The tool writes canonical ASCII JSON `snapshot.json` and separate
   `provenance.json`. It prints **metadata only**. Independently approve/store the
   provenance pin, then explicitly pass snapshot text, the trusted `SnapshotPin`
   and the approved concrete build to `loadStargateSnapshot` in
   `src/modules/navcom/snapshot.ts`. This async API requires Web Crypto SHA256.
   It performs no file I/O and exposes a private graph plus immutable name lookup.
   Do not trust provenance supplied alongside arbitrary input without review.
   A pin does not self-authenticate: replacing both snapshot and trusted pin cannot
   be detected cryptographically here. Freshness means matching the explicitly
   approved build, not automatically checking latest or a guessed expiry date.
5. Refresh only by repeating explicit resolution/download/review/conversion to a
   new output directory, approving new provenance, and testing it. There is no
   silent fallback to fixtures, stale data or live ESI.

## Verified schema, budgets and topology

Inspected official build **3662168**, release metadata **2026-10-30T11:21:02Z**.
ZIP root contains `_sde.jsonl`, `mapSolarSystems.jsonl`, `mapStargates.jsonl`.
The converter verifies `_sde` build number. It reads only build metadata and the
two map tables, never extracts ZIP paths. Systems use `_key`, optional localized
`name.en`, and optional `stargateIDs` (absent means none). Gates use `_key`,
`solarSystemID`, `destination.stargateID` and `destination.solarSystemID`.
Other table fields are intentionally outside this topology projection.

All IDs must be unique positive safe integers; systems must declare every owned
gate exactly once. Each gate must point to a known gate/system that points back
to exactly the original gate/system. Self-links, duplicate/reversed system
connections, missing references and contradictions fail; nothing is repaired.
All systems, including isolated/out-of-scope regions without static gates, are
retained. No regional filtering or invented English names. Missing English names
are null. Rows/edges sort numerically, names are lookup-only, and output bytes are
deterministic. The existing graph limits remain **10,000 systems / 50,000 edges**;
exceeding either is an error, never truncation.

Resource limits: archive 256 MiB compressed, 256 members, 1 GiB aggregate declared
uncompressed, 512 MiB/member, 1000:1 maximum expansion, 16 MiB/map table,
64 KiB/JSONL line, 10,000 system rows / 100,000 gate rows, 30-second conversion
budget and 4 MiB snapshot. ZIP-slip, symlinks/special files, directories, duplicate
members, encryption, unsupported compression and malformed JSON are rejected.
Only stored/deflated ZIPs are accepted. ZIP CRC is checked on consumed tables;
unrelated members are inspected for safety/budgets but never decompressed or used.
The already-hashed archive bytes are used for ZIP reading to avoid input replacement.
Python/ZIP parser exceptions fail the command closed. Inputs remain untrusted;
run the bounded tool in a constrained developer process, not a privileged service.

Snapshot loading verifies schema/version, converter version, build, official URL,
hash formats, pinned SHA256, canonical ID/edge ordering, counts, isolated-system
count and the existing graph's full topology validation. It copies data into
private adjacency/name maps. Minimum hops and avoid semantics are unchanged.
Static gates exclude wormholes, Ansiblex and dynamic edges. Shortest hops are
**not** a safety rating or a real-time/in-game route guarantee.

## Local verification evidence (metadata only)

The official ZIP was 99,304,848 bytes, with 102 members and 580,772,221 declared
uncompressed bytes. Conversion and a byte-identical second conversion passed;
the generated local snapshot also passed the TypeScript adapter.

| Input/output           | SHA256                                                             |
| ---------------------- | ------------------------------------------------------------------ |
| Archive                | `2114f82f41c390f3df43d6f94277ac6d658c03f13c7e255dcbe3d86e5a523751` |
| mapSolarSystems.jsonl  | `d2ea980c977a4818938bf3d9238608660e6685149d28139aa7749841e80ed7c7` |
| mapStargates.jsonl     | `2e1185669643d7a4454cd0a88be394dd67b9d8a3d95d028638350a8d78d16d5b` |
| Snapshot (converter 1) | `61c6e401dd97e52d4c236ed5dd16920a38dfa9c76fc637a17dc8a2e90ab9fc3b` |

Verified counts: **8,490 systems, 13,978 reciprocal gates, 6,989 undirected
connections, 3,222 isolated systems**. No real IDs, names, edges or routes are
included in this document. No official exceptions or topology contradictions
were needed. These digests record observations, not legal approval or upstream
signatures. No CCP data was committed, uploaded or bundled.

Run `python3 -m unittest discover -s scripts/sde -p 'test_*.py'` for converter
regressions. `npm test` also invokes these synthetic tests via the snapshot suite,
so normal lightweight PR CI exercises them without downloading data. Synthetic
adapter tests cover route/avoid/disconnected/same-system behavior, malformed or
stale pins, tampering and bounded topology. Native Windows tests are unrelated
and were not requested for this change.
