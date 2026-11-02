import { beforeEach, expect, it, vi } from 'vitest';
import { invoke, isTauri } from '@tauri-apps/api/core';
import { installRouteDatabase, routeInstallStatus } from './routeInstall';
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn(), isTauri: vi.fn() }));
beforeEach(() => {
  vi.mocked(invoke).mockReset();
  vi.mocked(isTauri).mockReturnValue(true);
});
it('browser preview has no native invokes', async () => {
  vi.mocked(isTauri).mockReturnValue(false);
  await expect(installRouteDatabase()).rejects.toBe('unsupported');
  await expect(routeInstallStatus()).rejects.toBe('unsupported');
  expect(invoke).not.toHaveBeenCalled();
});
it('uses fixed argument-free commands and strict responses', async () => {
  vi.mocked(invoke).mockResolvedValue('installed');
  expect(await installRouteDatabase()).toBe('installed');
  expect(invoke).toHaveBeenLastCalledWith('install_route_database');
  vi.mocked(invoke).mockResolvedValue('verifying');
  expect(await routeInstallStatus()).toBe('verifying');
  expect(invoke).toHaveBeenLastCalledWith('get_route_install_status');
  vi.mocked(invoke).mockResolvedValue({ url: 'untrusted' });
  await expect(installRouteDatabase()).rejects.toBe('invalid-response');
  await expect(routeInstallStatus()).rejects.toBe('invalid-response');
});
it('never exposes raw native details', async () => {
  vi.mocked(invoke).mockRejectedValue(new Error('private path'));
  await expect(installRouteDatabase()).rejects.toBe('network');
  vi.mocked(invoke).mockRejectedValue('existing-data');
  await expect(installRouteDatabase()).rejects.toBe('existing-data');
});
