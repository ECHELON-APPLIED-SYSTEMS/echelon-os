import { useId } from 'react';
import { resolveTheme, themes, type ThemeId } from './themes';

export function Appearance({
  theme,
  onChange,
}: Readonly<{ theme: ThemeId; onChange: (theme: ThemeId) => void }>) {
  const id = useId();
  const hint = `${id}-hint`;
  return (
    <div className="appearance">
      <label htmlFor={id}>Appearance</label>
      <select
        id={id}
        value={theme}
        aria-describedby={hint}
        onChange={(event) => onChange(resolveTheme(event.currentTarget.value))}
      >
        {themes.map((option) => (
          <option key={option.id} value={option.id}>
            {option.name}
          </option>
        ))}
      </select>
      <small id={hint}>This session only · resets on restart</small>
    </div>
  );
}
