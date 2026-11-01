import { invoke, isTauri } from '@tauri-apps/api/core';
import { loadStargateSnapshot } from '../modules/navcom/snapshot';
import type { SnapshotPin } from '../modules/navcom/snapshot';

/** Reviewed metadata only, from M2-B build 3662168. Not a CCP signature or
 * permission to distribute data. Updates require a reviewed source change. */
export const NAVCOM_PIN: SnapshotPin = Object.freeze({
  version: 1,
  converterVersion: '1',
  build: 3662168,
  sourceUrl:
    'https://developers.eveonline.com/static-data/tranquility/eve-online-static-data-3662168-jsonl.zip',
  archiveSha256:
    '2114f82f41c390f3df43d6f94277ac6d658c03f13c7e255dcbe3d86e5a523751',
  snapshotSha256:
    '61c6e401dd97e52d4c236ed5dd16920a38dfa9c76fc637a17dc8a2e90ab9fc3b',
  tableSha256: Object.freeze({
    'mapSolarSystems.jsonl':
      'd2ea980c977a4818938bf3d9238608660e6685149d28139aa7749841e80ed7c7',
    'mapStargates.jsonl':
      '2e1185669643d7a4454cd0a88be394dd67b9d8a3d95d028638350a8d78d16d5b',
  }),
  systems: 8490,
  gates: 13978,
  edges: 6989,
  isolatedSystems: 3222,
});

type Loaded = Awaited<ReturnType<typeof loadStargateSnapshot>>;
export type NavcomSnapshotResult =
  | Readonly<{ kind: 'preview' | 'missing' }>
  | Readonly<{ kind: 'ready'; snapshot: Loaded }>
  | Readonly<{
      kind: 'error';
      code:
        | 'unsafe-path'
        | 'unreadable'
        | 'oversize'
        | 'invalid-utf8'
        | 'changed'
        | 'invalid-snapshot'
        | 'invalid-response'
        | 'unavailable';
    }>;

/** Called explicitly only; no cache, polling, startup/UI import or caller pin. */
export async function readNavcomSnapshot(): Promise<NavcomSnapshotResult> {
  if (!isTauri()) return { kind: 'preview' };
  let response: unknown;
  try {
    response = await invoke('get_navcom_snapshot');
  } catch {
    return { kind: 'error', code: 'unavailable' };
  }
  if (!response || typeof response !== 'object' || !('kind' in response))
    return { kind: 'error', code: 'invalid-response' };
  if (response.kind === 'absent' && Object.keys(response).length === 1)
    return { kind: 'missing' };
  if (
    response.kind === 'error' &&
    'code' in response &&
    Object.keys(response).length === 2
  ) {
    const code = response.code;
    if (
      code === 'unsafe-path' ||
      code === 'unreadable' ||
      code === 'oversize' ||
      code === 'invalid-utf8' ||
      code === 'changed'
    )
      return { kind: 'error', code };
  }
  if (
    response.kind === 'text' &&
    'text' in response &&
    typeof response.text === 'string' &&
    Object.keys(response).length === 2
  ) {
    try {
      const snapshot = await loadStargateSnapshot(
        response.text,
        NAVCOM_PIN,
        NAVCOM_PIN.build,
      );
      return { kind: 'ready', snapshot };
    } catch {
      return { kind: 'error', code: 'invalid-snapshot' };
    }
  }
  return { kind: 'error', code: 'invalid-response' };
}
