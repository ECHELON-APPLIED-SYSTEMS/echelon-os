import { createHash, webcrypto } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { describe, expect, it, vi } from 'vitest';
import { loadStargateSnapshot } from './snapshot';
import type { SnapshotPin } from './snapshot';

vi.stubGlobal('crypto', webcrypto);
const data = {
  version: 1,
  systems: [
    [1, 'Fiction Alpha'],
    [2, null],
    [3, null],
  ],
  edges: [[1, 2]],
};
function fixture(value: unknown = data) {
  const text = JSON.stringify(value);
  const pin: SnapshotPin = {
    version: 1,
    converterVersion: '1',
    build: 42,
    sourceUrl:
      'https://developers.eveonline.com/static-data/tranquility/eve-online-static-data-42-jsonl.zip',
    archiveSha256: 'a'.repeat(64),
    tableSha256: {
      'mapSolarSystems.jsonl': 'b'.repeat(64),
      'mapStargates.jsonl': 'c'.repeat(64),
    },
    snapshotSha256: createHash('sha256').update(text).digest('hex'),
    systems: 3,
    gates: 2,
    edges: 1,
    isolatedSystems: 1,
  };
  return { text, pin };
}

describe('explicit offline snapshot adapter', () => {
  it('preserves route, avoid, same-system and disconnected outcomes', async () => {
    const { text, pin } = fixture();
    const result = await loadStargateSnapshot(text, pin, 42);
    expect(result.graph.shortestRoute(1, 2)).toEqual({
      kind: 'route',
      systems: [1, 2],
      jumps: 1,
    });
    expect(result.graph.shortestRoute(1, 2, [2])).toEqual({
      kind: 'no-route',
      reason: 'avoided-endpoint',
    });
    expect(result.graph.shortestRoute(1, 1).kind).toBe('same-system');
    expect(result.graph.shortestRoute(1, 3).kind).toBe('no-route');
    expect(result.englishName(1)).toBe('Fiction Alpha');
    expect(result.englishName(2)).toBeUndefined();
  });
  it('searches validated names and IDs with immutable bounded results', async () => {
    const systems = Array.from({ length: 25 }, (_, i) => [
      i + 1,
      i === 24 ? null : 'Fiction Duplicate',
    ]);
    const { text, pin } = fixture({ version: 1, systems, edges: [] });
    const result = await loadStargateSnapshot(
      text,
      { ...pin, systems: 25, gates: 0, edges: 0, isolatedSystems: 25 },
      42,
    );
    expect(result.systemCount).toBe(25);
    const matches = result.searchSystems('duplicate');
    expect(matches).toHaveLength(20);
    expect(Object.isFrozen(matches)).toBe(true);
    expect(Object.isFrozen(matches[0])).toBe(true);
    expect(result.searchSystems('25')).toEqual([{ id: 25, name: undefined }]);
    expect(result.searchSystems('')).toEqual([]);
    expect(result.searchSystems('x'.repeat(65))).toEqual([]);
    expect(result.searchSystems(' Fiction DuPlicate ')[0]?.id).toBe(1);
  });
  it('rejects missing, oversized, tampered and stale data', async () => {
    const { text, pin } = fixture();
    for (const input of [undefined, '', text + ' ', 'x'.repeat(4194305)]) {
      await expect(loadStargateSnapshot(input, pin, 42)).rejects.toThrow();
    }
    await expect(loadStargateSnapshot(text, pin, 43)).rejects.toThrow();
  });
  it('rejects malformed provenance and counts', async () => {
    const { text, pin } = fixture();
    for (const change of [
      { version: 2 },
      { converterVersion: '2' },
      { sourceUrl: 'https://example.com' },
      { archiveSha256: 'bad' },
      { tableSha256: {} },
      { systems: 10001 },
      { edges: 50001 },
      { gates: 3 },
      { isolatedSystems: 0 },
    ]) {
      await expect(
        loadStargateSnapshot(text, { ...pin, ...change } as SnapshotPin, 42),
      ).rejects.toThrow();
    }
  });
  it('rejects invalid topology even with a matching digest', async () => {
    for (const value of [
      { ...data, version: 2 },
      { ...data, extra: true },
      {
        ...data,
        systems: [
          [1, null],
          [1, null],
          [3, null],
        ],
      },
      {
        ...data,
        systems: [
          [2, null],
          [1, null],
          [3, null],
        ],
      },
      { ...data, edges: [[1, 99]] },
      { ...data, edges: [[2, 1]] },
      { ...data, edges: [[1, 1]] },
      {
        ...data,
        systems: [
          [1, 123],
          [2, null],
          [3, null],
        ],
      },
    ]) {
      const { text, pin } = fixture(value);
      await expect(loadStargateSnapshot(text, pin, 42)).rejects.toThrow();
    }
  });
  it('runs offline converter safety regressions using fictional ZIP/JSONL', () => {
    const output = execFileSync(
      'python3',
      ['-m', 'unittest', 'discover', '-s', 'scripts/sde', '-p', 'test_*.py'],
      { encoding: 'utf8', timeout: 30000 },
    );
    expect(output).toBe('');
  });
});
