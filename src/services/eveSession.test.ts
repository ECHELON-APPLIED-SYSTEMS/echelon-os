import { beforeEach, describe, expect, it, vi } from 'vitest';
import { invoke, isTauri } from '@tauri-apps/api/core';
import { disconnected, eveSession, parseView } from './eveSession';
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn(), isTauri: vi.fn() }));
const view = {
  ...disconnected,
  phase: 'connected',
  character: { id: 42, name: 'Fiction Pilot' },
  expires_at: 2_000_000_000,
};
beforeEach(() => {
  vi.mocked(isTauri).mockReturnValue(true);
  vi.mocked(invoke).mockReset();
});
describe('typed credential-free EVE IPC', () => {
  it('does not invoke anything in browser preview', async () => {
    vi.mocked(isTauri).mockReturnValue(false);
    expect(await eveSession.status()).toEqual(disconnected);
    await expect(eveSession.connect()).rejects.toBe('unsupported');
    expect(invoke).not.toHaveBeenCalled();
  });
  it('uses only specific commands with no renderer character, credentials, URL or flags', async () => {
    vi.mocked(invoke).mockResolvedValue(view);
    await eveSession.connect();
    await eveSession.refresh();
    await eveSession.disconnect();
    expect(vi.mocked(invoke).mock.calls.map((c) => c[0])).toEqual([
      'eve_connect',
      'eve_refresh_location',
      'eve_disconnect',
    ]);
    const confirmation = {
      id: 'a'.repeat(43),
      character: view.character,
      destination: 7,
      action: 'set-destination',
      expires_at: 2_000_000_000,
    };
    vi.mocked(invoke).mockResolvedValue(confirmation);
    expect(await eveSession.prepare(7, 'set-destination')).toEqual(
      confirmation,
    );
    expect(invoke).toHaveBeenLastCalledWith('eve_prepare_waypoint', {
      destination: 7,
      action: 'set-destination',
    });
    vi.mocked(invoke).mockResolvedValue(view);
    await eveSession.send(confirmation.id);
    expect(invoke).toHaveBeenLastCalledWith('eve_send_waypoint', {
      confirmationId: confirmation.id,
    });
  });
  it.each([
    null,
    {},
    { ...view, access_token: 'synthetic' },
    { ...view, character: { id: 0, name: 'x' } },
    { ...view, phase: 'disconnected' },
    { ...view, expires_at: null },
    { ...view, error: 'raw private error' },
    { ...view, location: { system_id: 7 } },
    { ...view, retry_at: Infinity },
  ])('rejects malformed or credential-bearing status %j', (v) => {
    expect(() => parseView(v)).toThrow();
  });
  it('sanitizes raw errors and rejects invalid confirmation shapes/arguments', async () => {
    vi.mocked(invoke).mockRejectedValue(new Error('private transport detail'));
    await expect(eveSession.refresh()).rejects.toBe('unavailable');
    vi.mocked(invoke).mockRejectedValue('rate-limited');
    await expect(eveSession.refresh()).rejects.toBe('rate-limited');
    vi.mocked(invoke).mockResolvedValue({ id: 'untrusted' });
    await expect(eveSession.prepare(7, 'add-waypoint')).rejects.toBe(
      'invalid-response',
    );
    vi.mocked(invoke).mockClear();
    await expect(eveSession.prepare(-1, 'add-waypoint')).rejects.toBe(
      'invalid-response',
    );
    await expect(eveSession.send('bad')).rejects.toBe('invalid-response');
    expect(invoke).not.toHaveBeenCalled();
  });
});
