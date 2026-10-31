import { beforeEach, describe, expect, it, vi } from 'vitest';
import { invoke, isTauri } from '@tauri-apps/api/core';
import { readAppMetadata } from './appMetadata';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn(), isTauri: vi.fn() }));
const metadata = {
  name: 'ECHELON OS',
  version: '0.1.0',
  platform: 'windows',
  milestone: 'M1-A',
};

beforeEach(() => {
  vi.mocked(isTauri).mockReturnValue(true);
});

describe('desktop metadata boundary', () => {
  it('uses the read-only command and validates the wire response', async () => {
    vi.mocked(invoke).mockResolvedValue(metadata);
    await expect(readAppMetadata()).resolves.toEqual({
      kind: 'desktop',
      metadata,
    });
    expect(invoke).toHaveBeenCalledExactlyOnceWith('get_app_metadata');
  });

  it('does not fake a desktop connection in a browser', async () => {
    vi.mocked(isTauri).mockReturnValue(false);
    await expect(readAppMetadata()).resolves.toEqual({ kind: 'preview' });
    expect(invoke).not.toHaveBeenCalled();
  });

  it.each([
    null,
    {},
    { ...metadata, version: '' },
    { ...metadata, platform: 12 },
    { ...metadata, milestone: 'M1-B' },
    { ...metadata, name: 'Other app' },
  ])('rejects malformed responses: %j', async (value) => {
    vi.mocked(invoke).mockResolvedValue(value);
    await expect(readAppMetadata()).rejects.toThrow(
      'invalid metadata response',
    );
  });

  it('propagates transport failures for the shell to display', async () => {
    vi.mocked(invoke).mockRejectedValue(new Error('IPC denied'));
    await expect(readAppMetadata()).rejects.toThrow('IPC denied');
  });
});
