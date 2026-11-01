import { createStargateGraph, ROUTING_LIMITS } from './routing';

/** Supply this pin from a separately reviewed local provenance record, never from
 * the untrusted snapshot itself. Hashes prove integrity against that pin, not CCP
 * authenticity or freshness. The caller selects the approved concrete build. */
export interface SnapshotPin {
  readonly version: 1;
  readonly converterVersion: '1';
  readonly build: number;
  readonly sourceUrl: string;
  readonly archiveSha256: string;
  readonly snapshotSha256: string;
  readonly tableSha256: Readonly<{
    'mapSolarSystems.jsonl': string;
    'mapStargates.jsonl': string;
  }>;
  readonly systems: number;
  readonly gates: number;
  readonly edges: number;
  readonly isolatedSystems: number;
}

function check(condition: unknown): asserts condition {
  if (!condition) throw new Error('Invalid or unapproved NAVCOM snapshot');
}
function integer(n: unknown, min: number, max: number): n is number {
  return Number.isSafeInteger(n) && Number(n) >= min && Number(n) <= max;
}
function record(n: unknown): n is Record<string, unknown> {
  return !!n && typeof n === 'object' && !Array.isArray(n);
}
function hash(n: unknown): n is string {
  return typeof n === 'string' && /^[a-f0-9]{64}$/.test(n);
}

/** Explicit local text input only; no filesystem, network, UI or startup hooks. */
export async function loadStargateSnapshot(
  text: unknown,
  pin: SnapshotPin,
  approvedBuild: number,
) {
  check(record(pin));
  check(
    pin.version === 1 &&
      pin.converterVersion === '1' &&
      integer(approvedBuild, 1, Number.MAX_SAFE_INTEGER) &&
      pin.build === approvedBuild &&
      pin.sourceUrl ===
        `https://developers.eveonline.com/static-data/tranquility/eve-online-static-data-${approvedBuild}-jsonl.zip` &&
      hash(pin.archiveSha256) &&
      hash(pin.snapshotSha256) &&
      record(pin.tableSha256) &&
      hash(pin.tableSha256['mapSolarSystems.jsonl']) &&
      hash(pin.tableSha256['mapStargates.jsonl']) &&
      integer(pin.systems, 1, ROUTING_LIMITS.systems) &&
      integer(pin.edges, 0, ROUTING_LIMITS.edges) &&
      pin.gates === pin.edges * 2 &&
      integer(pin.isolatedSystems, 0, pin.systems),
  );
  check(typeof text === 'string' && text.length <= 4 * 1024 * 1024);
  const bytes = new TextEncoder().encode(text);
  check(bytes.length <= 4 * 1024 * 1024);
  const actual = Array.from(
    new Uint8Array(await crypto.subtle.digest('SHA-256', bytes)),
    (b) => b.toString(16).padStart(2, '0'),
  ).join('');
  check(actual === pin.snapshotSha256);
  const data: unknown = JSON.parse(text);
  check(record(data) && data.version === 1);
  check(Object.keys(data).sort().join(',') === 'edges,systems,version');
  check(Array.isArray(data.systems) && data.systems.length === pin.systems);
  check(Array.isArray(data.edges) && data.edges.length === pin.edges);
  const ids: number[] = [];
  const names = new Map<number, string>();
  for (const row of data.systems) {
    check(Array.isArray(row) && row.length === 2);
    const [id, name] = row as unknown[];
    check(integer(id, 1, Number.MAX_SAFE_INTEGER));
    check(ids.length === 0 || id > ids[ids.length - 1]!);
    check(
      name === null ||
        (typeof name === 'string' && name.length > 0 && name.length <= 256),
    );
    ids.push(id);
    if (typeof name === 'string') names.set(id, name);
  }
  let previous: number[] | undefined;
  const connected = new Set<number>();
  for (const edge of data.edges) {
    check(Array.isArray(edge) && edge.length === 2);
    const [a, b] = edge as unknown[];
    check(integer(a, 1, Number.MAX_SAFE_INTEGER));
    check(integer(b, 1, Number.MAX_SAFE_INTEGER) && a < b);
    check(
      !previous || a > previous[0]! || (a === previous[0] && b > previous[1]!),
    );
    previous = [a, b];
    connected.add(a);
    connected.add(b);
  }
  const graph = createStargateGraph(ids, data.edges);
  check(ids.length - connected.size === pin.isolatedSystems);
  return Object.freeze({
    graph,
    // Do not expose a mutable Map, or the caller's mutable provenance object.
    englishName: (id: number): string | undefined => names.get(id),
    build: approvedBuild,
  });
}
