import { invoke, isTauri } from '@tauri-apps/api/core';

export type AppMetadata = Readonly<{
  name: 'ECHELON OS';
  version: string;
  platform: string;
  milestone: 'M1-B';
}>;

export type MetadataResult =
  | Readonly<{ kind: 'desktop'; metadata: AppMetadata }>
  | Readonly<{ kind: 'preview' }>;

function parseMetadata(value: unknown): AppMetadata {
  if (
    typeof value !== 'object' ||
    value === null ||
    !('name' in value) ||
    value.name !== 'ECHELON OS' ||
    !('version' in value) ||
    typeof value.version !== 'string' ||
    !value.version.trim() ||
    !('platform' in value) ||
    typeof value.platform !== 'string' ||
    !value.platform.trim() ||
    !('milestone' in value) ||
    value.milestone !== 'M1-B'
  ) {
    throw new Error('The desktop returned an invalid metadata response.');
  }
  return {
    name: value.name,
    version: value.version,
    platform: value.platform,
    milestone: value.milestone,
  };
}

export async function readAppMetadata(): Promise<MetadataResult> {
  if (!isTauri()) return { kind: 'preview' };
  const response: unknown = await invoke('get_app_metadata');
  return { kind: 'desktop', metadata: parseMetadata(response) };
}
