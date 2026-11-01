import { beforeEach, describe, expect, it, vi } from 'vitest';
import { invoke, isTauri } from '@tauri-apps/api/core';
import { readNavcomSnapshot, NAVCOM_PIN } from './navcomSnapshot';
import { loadStargateSnapshot } from '../modules/navcom/snapshot';
import { createStargateGraph } from '../modules/navcom/routing';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn(), isTauri: vi.fn() }));
vi.mock('../modules/navcom/snapshot', () => ({
  loadStargateSnapshot: vi.fn(),
}));
beforeEach(() => {
  vi.mocked(isTauri).mockReturnValue(true);
  vi.mocked(loadStargateSnapshot).mockReset();
});

describe('explicit private NAVCOM load service', () => {
  it('does not access native storage in browser preview', async () => {
    vi.mocked(isTauri).mockReturnValue(false);
    expect(await readNavcomSnapshot()).toEqual({ kind: 'preview' });
    expect(invoke).not.toHaveBeenCalled();
    expect(loadStargateSnapshot).not.toHaveBeenCalled();
  });
  it('missing is normal and does not invoke validation', async () => {
    vi.mocked(invoke).mockResolvedValue({ kind: 'absent' });
    expect(await readNavcomSnapshot()).toEqual({ kind: 'missing' });
    expect(loadStargateSnapshot).not.toHaveBeenCalled();
  });
  it('returns ready only after validator succeeds with the compiled pin', async () => {
    // Transport/delegation test only. The real validator's synthetic pinned graph
    // is tested in snapshot.test.ts; never embed real data matching the app pin.
    const snapshot = {
      graph: createStargateGraph([1, 2], [[1, 2]]),
      englishName: () => undefined,
      build: 42,
    };
    vi.mocked(invoke).mockResolvedValue({
      kind: 'text',
      text: 'fictional transport text',
    });
    vi.mocked(loadStargateSnapshot).mockResolvedValue(snapshot);
    const result = await readNavcomSnapshot();
    expect(invoke).toHaveBeenCalledExactlyOnceWith('get_navcom_snapshot');
    expect(loadStargateSnapshot).toHaveBeenCalledExactlyOnceWith(
      'fictional transport text',
      NAVCOM_PIN,
      3662168,
    );
    expect(result.kind).toBe('ready');
    if (result.kind === 'ready')
      expect(result.snapshot.graph.shortestRoute(1, 2)).toEqual({
        kind: 'route',
        systems: [1, 2],
        jumps: 1,
      });
    expect(Object.isFrozen(NAVCOM_PIN)).toBe(true);
    expect(Object.isFrozen(NAVCOM_PIN.tableSha256)).toBe(true);
  });
  it('fails closed on integrity/schema validation failures and never caches', async () => {
    vi.mocked(invoke).mockResolvedValue({ kind: 'text', text: '{tampered' });
    vi.mocked(loadStargateSnapshot).mockRejectedValue(
      new Error('untrusted detail'),
    );
    expect(await readNavcomSnapshot()).toEqual({
      kind: 'error',
      code: 'invalid-snapshot',
    });
    vi.mocked(invoke).mockResolvedValue({ kind: 'absent' });
    expect(await readNavcomSnapshot()).toEqual({ kind: 'missing' });
    expect(invoke).toHaveBeenCalledTimes(2);
  });
  it.each(['unsafe-path', 'unreadable', 'oversize', 'invalid-utf8', 'changed'])(
    'preserves safe native error %s',
    async (code) => {
      vi.mocked(invoke).mockResolvedValue({ kind: 'error', code });
      expect(await readNavcomSnapshot()).toEqual({ kind: 'error', code });
    },
  );
  it.each([
    null,
    {},
    { kind: 'text', text: 12 },
    { kind: 'error', code: '/private/path' },
    { kind: 'absent', provenance: {} },
    { kind: 'text', text: 'x', pin: {} },
  ])('rejects unexpected IPC shape %j', async (value) => {
    vi.mocked(invoke).mockResolvedValue(value);
    expect(await readNavcomSnapshot()).toEqual({
      kind: 'error',
      code: 'invalid-response',
    });
  });
  it('hides raw IPC/OS details', async () => {
    vi.mocked(invoke).mockRejectedValue(
      new Error('/private/path permission denied'),
    );
    expect(await readNavcomSnapshot()).toEqual({
      kind: 'error',
      code: 'unavailable',
    });
  });
});
