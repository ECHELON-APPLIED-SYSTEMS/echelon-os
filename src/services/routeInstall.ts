import { invoke, isTauri } from '@tauri-apps/api/core';
export const installErrors = [
  'busy',
  'location',
  'unsafe-path',
  'existing-data',
  'network',
  'rate-limited',
  'response',
  'too-large',
  'truncated',
  'integrity',
  'conversion',
  'storage',
  'publish',
  'unsupported',
  'invalid-response',
] as const;
export type InstallError = (typeof installErrors)[number];
export const installPhases = [
  'idle',
  'downloading',
  'verifying',
  'installing',
  'ready',
  'error',
] as const;
export type InstallPhase = (typeof installPhases)[number];
export const installMessages: Record<InstallError, string> = {
  busy: 'A route-data installation is already running. Wait for it to finish before retrying.',
  location:
    'Download requires a release Windows app in a writable local extracted folder outside source control, build targets and cloud-synced storage.',
  'unsafe-path':
    'The portable folder changed or contains an unsafe link or junction. Preserve the folder and inspect it before retrying.',
  'existing-data':
    'Existing route data could not be validated. Preserve it and follow the recovery guide; it was not replaced.',
  network:
    'The official download could not complete. Check connectivity and retry explicitly.',
  'rate-limited':
    'CCP has limited requests. Wait before trying another download; no automatic retry will occur.',
  response:
    'The official source returned an unsupported response. No alternate source was used.',
  'too-large': 'The download exceeds the 256 MiB limit or available memory.',
  truncated:
    'The download ended early or returned an unexpected length. Retry when connectivity is stable.',
  integrity:
    'Data did not match the reviewed hashes or counts. Nothing unverified was installed.',
  conversion:
    'The archive could not be converted within the supported format and resource limits.',
  storage:
    'The portable folder could not complete a safe write. Preserve the folder and check access before retrying.',
  publish:
    'Publication was refused. Another snapshot may have appeared; use Retry loading before attempting another download.',
  unsupported: 'Download is available in the Windows desktop application.',
  'invalid-response':
    'The desktop returned an unsupported installer response. Use a matching application version.',
};
export function installError(value: unknown): InstallError {
  return installErrors.includes(value as InstallError)
    ? (value as InstallError)
    : 'network';
}
export function canInstallRoutes(): boolean {
  return isTauri();
}
export async function installRouteDatabase(): Promise<
  'installed' | 'already-installed'
> {
  if (!isTauri()) throw 'unsupported';
  let response: unknown;
  try {
    response = await invoke('install_route_database');
  } catch (e) {
    throw installError(e);
  }
  if (response !== 'installed' && response !== 'already-installed')
    throw 'invalid-response';
  return response;
}
/** Local progress IPC only, used only during an owner-initiated operation. */
export async function routeInstallStatus(): Promise<InstallPhase> {
  if (!isTauri()) throw 'unsupported';
  const value: unknown = await invoke('get_route_install_status');
  if (!installPhases.includes(value as InstallPhase)) throw 'invalid-response';
  return value as InstallPhase;
}
