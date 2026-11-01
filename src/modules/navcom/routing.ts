/** Offline, equal-cost stargates only. No topology or external services included. */
export const ROUTING_LIMITS = Object.freeze({ systems: 10_000, edges: 50_000 });

declare const solarSystemId: unique symbol;
export type SolarSystemId = number & { readonly [solarSystemId]: true };
export type RouteResult =
  | {
      readonly kind: 'route';
      readonly systems: readonly SolarSystemId[];
      readonly jumps: number;
    }
  | {
      readonly kind: 'same-system';
      readonly systems: readonly SolarSystemId[];
      readonly jumps: 0;
    }
  | {
      readonly kind: 'no-route';
      readonly reason: 'disconnected' | 'avoided-endpoint';
    };

export type RoutingInputErrorCode =
  | 'invalid-topology'
  | 'limit-exceeded'
  | 'invalid-id'
  | 'duplicate-system'
  | 'invalid-edge'
  | 'unknown-system'
  | 'duplicate-edge'
  | 'invalid-avoid';

export class RoutingInputError extends Error {
  constructor(readonly code: RoutingInputErrorCode) {
    super(`Invalid routing input: ${code}`);
    this.name = 'RoutingInputError';
  }
}

export interface StargateGraph {
  /** Avoid IDs form a set: supply a bounded array of distinct known system IDs. */
  shortestRoute(
    origin: unknown,
    destination: unknown,
    avoidSystemIds?: unknown,
  ): RouteResult;
}

function id(value: unknown): SolarSystemId {
  if (typeof value !== 'number' || !Number.isSafeInteger(value) || value <= 0) {
    throw new RoutingInputError('invalid-id');
  }
  return value as SolarSystemId;
}

/**
 * Validate plain data and copy into private adjacency lists. Each edge is one
 * undirected [systemA, systemB] pair; repeated/reversed edges are errors.
 * This validates structure, not the provenance or in-game accuracy of the data.
 */
export function createStargateGraph(
  systemIds: unknown,
  edges: unknown,
): StargateGraph {
  if (!Array.isArray(systemIds) || !Array.isArray(edges)) {
    throw new RoutingInputError('invalid-topology');
  }
  if (
    systemIds.length > ROUTING_LIMITS.systems ||
    edges.length > ROUTING_LIMITS.edges
  ) {
    throw new RoutingInputError('limit-exceeded');
  }
  const adjacency = new Map<SolarSystemId, SolarSystemId[]>();
  for (const value of systemIds) {
    const system = id(value);
    if (adjacency.has(system)) throw new RoutingInputError('duplicate-system');
    adjacency.set(system, []);
  }
  const seenEdges = new Set<string>();
  for (const edge of edges) {
    if (!Array.isArray(edge) || edge.length !== 2) {
      throw new RoutingInputError('invalid-edge');
    }
    const a = id(edge[0]);
    const b = id(edge[1]);
    if (a === b) throw new RoutingInputError('invalid-edge');
    const from = adjacency.get(a);
    const to = adjacency.get(b);
    if (!from || !to) throw new RoutingInputError('unknown-system');
    const key = a < b ? `${a}:${b}` : `${b}:${a}`;
    if (seenEdges.has(key)) throw new RoutingInputError('duplicate-edge');
    seenEdges.add(key);
    from.push(b);
    to.push(a);
  }
  for (const neighbors of adjacency.values()) neighbors.sort((a, b) => a - b);

  function knownId(value: unknown): SolarSystemId {
    const system = id(value);
    if (!adjacency.has(system)) throw new RoutingInputError('unknown-system');
    return system;
  }

  return Object.freeze({
    shortestRoute(
      origin: unknown,
      destination: unknown,
      avoidSystemIds: unknown = [],
    ): RouteResult {
      const start = knownId(origin);
      const end = knownId(destination);
      if (!Array.isArray(avoidSystemIds))
        throw new RoutingInputError('invalid-avoid');
      if (avoidSystemIds.length > ROUTING_LIMITS.systems) {
        throw new RoutingInputError('limit-exceeded');
      }
      const avoided = new Set<SolarSystemId>();
      for (const value of avoidSystemIds) {
        const system = knownId(value);
        if (avoided.has(system)) throw new RoutingInputError('invalid-avoid');
        avoided.add(system);
      }
      if (avoided.has(start) || avoided.has(end)) {
        return Object.freeze({ kind: 'no-route', reason: 'avoided-endpoint' });
      }
      if (start === end) {
        return Object.freeze({
          kind: 'same-system',
          systems: Object.freeze([start]),
          jumps: 0,
        });
      }
      const parents = new Map<SolarSystemId, SolarSystemId | null>([
        [start, null],
      ]);
      const queue = [start];
      for (let head = 0; head < queue.length; head++) {
        const current = queue[head]!;
        for (const neighbor of adjacency.get(current)!) {
          if (avoided.has(neighbor) || parents.has(neighbor)) continue;
          parents.set(neighbor, current);
          if (neighbor === end) {
            const path: SolarSystemId[] = [];
            let cursor: SolarSystemId | null = end;
            while (cursor !== null) {
              path.push(cursor);
              cursor = parents.get(cursor)!;
            }
            path.reverse();
            return Object.freeze({
              kind: 'route',
              systems: Object.freeze(path),
              jumps: path.length - 1,
            });
          }
          queue.push(neighbor);
        }
      }
      return Object.freeze({ kind: 'no-route', reason: 'disconnected' });
    },
  });
}
