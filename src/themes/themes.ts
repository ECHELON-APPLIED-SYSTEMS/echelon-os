export const themes = [
  { id: 'echelon', name: 'ECHELON Graphite' },
  { id: 'amarr', name: 'Amarr Empire' },
  { id: 'caldari', name: 'Caldari State' },
  { id: 'gallente', name: 'Gallente Federation' },
  { id: 'minmatar', name: 'Minmatar Republic' },
] as const;

export type ThemeId = (typeof themes)[number]['id'];
export const defaultTheme: ThemeId = 'echelon';

export function resolveTheme(value: unknown): ThemeId {
  return themes.find((theme) => theme.id === value)?.id ?? defaultTheme;
}
