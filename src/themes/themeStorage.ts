import { invoke, isTauri } from '@tauri-apps/api/core';
import { themes, type ThemeId } from './themes';

export type ThemeLoad =
  Readonly<{ kind: 'preview' }> | Readonly<{ kind: 'desktop'; theme: ThemeId }>;

export async function loadTheme(): Promise<ThemeLoad> {
  if (!isTauri()) return { kind: 'preview' };
  const value: unknown = await invoke('get_theme');
  const theme = themes.find((entry) => entry.id === value);
  if (!theme) throw new Error('Invalid saved appearance response.');
  return { kind: 'desktop', theme: theme.id };
}

export async function saveTheme(theme: ThemeId): Promise<'saved' | 'preview'> {
  if (!isTauri()) return 'preview';
  await invoke('set_theme', { theme });
  return 'saved';
}
