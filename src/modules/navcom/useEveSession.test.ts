import { act, renderHook } from '@testing-library/react';
import { expect, it, vi } from 'vitest';
import { invoke, isTauri } from '@tauri-apps/api/core';
import { disconnected } from '../../services/eveSession';
import { useEveSession } from './useEveSession';
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn(), isTauri: vi.fn() }));
it('marks cached location stale and clears expired identity using local timers only', async () => {
  vi.useFakeTimers();
  try {
    vi.setSystemTime(new Date('2026-11-01T00:00:00Z'));
    const now = Date.now() / 1000;
    vi.mocked(isTauri).mockReturnValue(true);
    vi.mocked(invoke).mockResolvedValue({
      ...disconnected,
      phase: 'connected',
      character: { id: 42, name: 'Fiction Pilot' },
      expires_at: now + 2,
      location: {
        system_id: 7,
        fetched_at: now,
        server_as_of: now - 2,
        fresh_until: now + 1,
        cached: true,
        stale: false,
      },
    });
    const { result, unmount } = renderHook(() => useEveSession());
    await act(async () => {});
    expect(result.current.view.phase).toBe('connected');
    await act(async () => vi.advanceTimersByTimeAsync(1000));
    expect(result.current.view.location?.stale).toBe(true);
    await act(async () => vi.advanceTimersByTimeAsync(1000));
    expect(result.current.view.phase).toBe('expired');
    expect(result.current.view.character).toBeNull();
    expect(result.current.view.location).toBeNull();
    expect(invoke).toHaveBeenCalledExactlyOnceWith('eve_status', undefined);
    unmount();
  } finally {
    vi.useRealTimers();
  }
});
