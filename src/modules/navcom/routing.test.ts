import { describe, expect, it } from 'vitest';
import {
  createStargateGraph,
  ROUTING_LIMITS,
  RoutingInputError,
} from './routing';

function rejects(action: () => unknown, code: string) {
  expect(action).toThrow(RoutingInputError);
  expect(action).toThrow(`Invalid routing input: ${code}`);
}

// Synthetic identifiers and connections only; these are not New Eden data.
describe('offline stargate routing', () => {
  const systems = [1, 2, 3, 4, 5, 6];
  const edges = [
    [1, 3],
    [3, 4],
    [1, 2],
    [2, 4],
    [4, 5],
    [2, 3],
  ];
  const graph = createStargateGraph(systems, edges);

  it('routes direct edges in either direction', () => {
    expect(graph.shortestRoute(1, 2)).toEqual({
      kind: 'route',
      systems: [1, 2],
      jumps: 1,
    });
    expect(graph.shortestRoute(2, 1)).toEqual({
      kind: 'route',
      systems: [2, 1],
      jumps: 1,
    });
  });
  it('finds minimum hops through cycles with numeric lexicographic ties', () => {
    expect(graph.shortestRoute(1, 5)).toEqual({
      kind: 'route',
      systems: [1, 2, 4, 5],
      jumps: 3,
    });
    const reordered = createStargateGraph(
      [...systems].reverse(),
      edges.map(([a, b]) => [b, a]).reverse(),
    );
    expect(reordered.shortestRoute(1, 5)).toEqual(graph.shortestRoute(1, 5));
    expect(
      createStargateGraph(
        [1, 10, 2, 20],
        [
          [1, 10],
          [10, 20],
          [1, 2],
          [2, 20],
        ],
      ).shortestRoute(1, 20),
    ).toEqual({ kind: 'route', systems: [1, 2, 20], jumps: 2 });
  });
  it('distinguishes same-system and disconnected results', () => {
    expect(graph.shortestRoute(6, 6)).toEqual({
      kind: 'same-system',
      systems: [6],
      jumps: 0,
    });
    expect(graph.shortestRoute(1, 6)).toEqual({
      kind: 'no-route',
      reason: 'disconnected',
    });
  });
  it('uses an avoided alternative or reports no route', () => {
    expect(graph.shortestRoute(1, 5, [2])).toEqual({
      kind: 'route',
      systems: [1, 3, 4, 5],
      jumps: 3,
    });
    expect(graph.shortestRoute(1, 5, [2, 3])).toEqual({
      kind: 'no-route',
      reason: 'disconnected',
    });
  });
  it.each([
    [1, 5, [1]],
    [1, 5, [5]],
    [1, 1, [1]],
  ])('blocks avoided endpoints (%j, %j)', (a, b, avoid) => {
    expect(graph.shortestRoute(a, b, avoid)).toEqual({
      kind: 'no-route',
      reason: 'avoided-endpoint',
    });
  });
  it.each([
    0,
    -1,
    1.1,
    NaN,
    Infinity,
    Number.MAX_SAFE_INTEGER + 1,
    '1',
    null,
    undefined,
  ])('rejects invalid IDs %j', (bad) => {
    rejects(() => createStargateGraph([bad], []), 'invalid-id');
    rejects(() => graph.shortestRoute(bad, 1), 'invalid-id');
    rejects(() => graph.shortestRoute(1, bad), 'invalid-id');
    rejects(() => graph.shortestRoute(1, 2, [bad]), 'invalid-id');
  });
  it('rejects unknown endpoints and avoid entries even for same-system calls', () => {
    rejects(() => graph.shortestRoute(7, 1), 'unknown-system');
    rejects(() => graph.shortestRoute(1, 7), 'unknown-system');
    rejects(() => graph.shortestRoute(1, 1, [7]), 'unknown-system');
    rejects(() => createStargateGraph([1], [[1, 2]]), 'unknown-system');
  });
  it('rejects malformed topology, duplicate nodes, sparse data and malformed edges', () => {
    rejects(() => createStargateGraph(null, []), 'invalid-topology');
    rejects(() => createStargateGraph([], {}), 'invalid-topology');
    rejects(() => createStargateGraph([1, 1], []), 'duplicate-system');
    rejects(() => createStargateGraph(new Array(2), []), 'invalid-id');
    for (const edge of [null, {}, [1], [1, 2, 3], [1, 1]]) {
      rejects(() => createStargateGraph([1, 2, 3], [edge]), 'invalid-edge');
    }
    rejects(() => createStargateGraph([1, 2], [[1, '2']]), 'invalid-id');
  });
  it.each([
    [
      [1, 2],
      [1, 2],
    ],
    [
      [1, 2],
      [2, 1],
    ],
  ])('rejects duplicate undirected edges %j', (...duplicates) => {
    rejects(() => createStargateGraph([1, 2], duplicates), 'duplicate-edge');
  });
  it('rejects malformed or duplicate avoid sets', () => {
    rejects(() => graph.shortestRoute(1, 2, new Set([3])), 'invalid-avoid');
    rejects(() => graph.shortestRoute(1, 2, [3, 3]), 'invalid-avoid');
  });
  it('does not mutate inputs or share mutable graph/result state across calls', () => {
    const nodes = Object.freeze([1, 2, 3]);
    const connections = [
      [1, 2],
      [2, 3],
    ];
    const snapshot = structuredClone(connections);
    const copy = createStargateGraph(nodes, connections);
    const avoided = Object.freeze([2]);
    expect(copy.shortestRoute(1, 3, avoided)).toEqual({
      kind: 'no-route',
      reason: 'disconnected',
    });
    expect(connections).toEqual(snapshot);
    connections[0]![0] = 3;
    connections.length = 0;
    const expected = { kind: 'route', systems: [1, 2, 3], jumps: 2 };
    const result = copy.shortestRoute(1, 3);
    expect(result).toEqual(expected);
    if (result.kind === 'route')
      expect(Object.isFrozen(result.systems)).toBe(true);
    expect(Object.isFrozen(result)).toBe(true);
    expect(copy.shortestRoute(1, 3)).toEqual(expected);
    expect(avoided).toEqual([2]);
  });
  it('accepts empty and maximum-sized graphs and terminates iterative traversal', () => {
    rejects(
      () => createStargateGraph([], []).shortestRoute(1, 1),
      'unknown-system',
    );
    const nodes = Array.from(
      { length: ROUTING_LIMITS.systems },
      (_, index) => index + 1,
    );
    const chain = nodes.slice(1).map((node) => [node - 1, node]);
    const large = createStargateGraph(nodes, chain);
    const result = large.shortestRoute(1, nodes.length);
    expect(result.kind).toBe('route');
    if (result.kind === 'route') {
      expect(result.jumps).toBe(nodes.length - 1);
      expect(result.systems).toEqual(nodes);
    }
    rejects(
      () => createStargateGraph([...nodes, nodes.length + 1], []),
      'limit-exceeded',
    );
    rejects(
      () => createStargateGraph([], new Array(ROUTING_LIMITS.edges + 1)),
      'limit-exceeded',
    );
    rejects(
      () => large.shortestRoute(1, 1, new Array(ROUTING_LIMITS.systems + 1)),
      'limit-exceeded',
    );
  });
});
