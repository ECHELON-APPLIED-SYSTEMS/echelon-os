import { beforeEach, expect, it, vi } from 'vitest';
import { invoke, isTauri } from '@tauri-apps/api/core';
import { cancelRoute, prepareRoute, sendRoute } from './eveRoute';
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn(), isTauri: vi.fn() }));
beforeEach(() => {
  vi.mocked(isTauri).mockReturnValue(true);
  vi.mocked(invoke).mockReset();
});
it('uses narrowly typed preparation and id-only send/cancel without credential or flag inputs', async () => {
  const c = {
    id: 'a'.repeat(43),
    character: { id: 42, name: 'Fiction Pilot' },
    systems: [1, 2, 3],
    expires_at: 2_000_000_000,
  };
  vi.mocked(invoke).mockResolvedValue(c);
  expect(await prepareRoute([1, 2, 3])).toEqual(c);
  expect(invoke).toHaveBeenLastCalledWith('eve_prepare_route', {
    systems: [1, 2, 3],
  });
  vi.mocked(invoke).mockResolvedValue({
    delivery: 'all-accepted',
    total: 2,
    accepted_count: 2,
    last_accepted: 3,
    failed_system: null,
    next_unsent: null,
    reason: null,
  });
  await sendRoute(c.id);
  expect(invoke).toHaveBeenLastCalledWith('eve_send_route', {
    confirmationId: c.id,
  });
  await cancelRoute(c.id);
  expect(invoke).toHaveBeenLastCalledWith('eve_cancel_route', {
    confirmationId: c.id,
  });
});
it('rejects caps and altered native sequences before any send', async () => {
  await expect(
    prepareRoute(Array.from({ length: 12 }, (_, i) => i + 1)),
  ).rejects.toBe('invalid-response');
  expect(invoke).not.toHaveBeenCalled();
  vi.mocked(invoke).mockResolvedValue({
    id: 'a'.repeat(43),
    character: { id: 42, name: 'Fiction Pilot' },
    systems: [1, 3],
    expires_at: 2_000_000_000,
  });
  await expect(prepareRoute([1, 2, 3])).rejects.toBe('invalid-response');
});
it('rejects fabricated completion and sanitizes failures', async () => {
  vi.mocked(invoke).mockResolvedValue({
    delivery: 'all-accepted',
    total: 2,
    accepted_count: 1,
    last_accepted: 2,
    failed_system: null,
    next_unsent: null,
    reason: null,
  });
  await expect(sendRoute('a'.repeat(43))).rejects.toBe('invalid-response');
  vi.mocked(invoke).mockRejectedValue(new Error('private detail'));
  await expect(sendRoute('a'.repeat(43))).rejects.toBe('unavailable');
  vi.mocked(isTauri).mockReturnValue(false);
  await expect(prepareRoute([1, 2])).rejects.toBe('unsupported');
});
