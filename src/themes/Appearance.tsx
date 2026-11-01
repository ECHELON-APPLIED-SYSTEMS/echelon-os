import { useId } from 'react';
import { resolveTheme, themes, type ThemeId } from './themes';
import type { AppearancePhase } from './useAppearance';

export function Appearance({
  theme,
  onChange,
  phase,
  onRetry,
}: Readonly<{
  theme: ThemeId;
  onChange: (theme: ThemeId) => void;
  phase: AppearancePhase;
  onRetry: () => void;
}>) {
  const id = useId();
  const hint = `${id}-hint`;
  return (
    <div className="appearance">
      <label htmlFor={id}>Appearance</label>
      <select
        id={id}
        value={theme}
        disabled={
          phase === 'loading' || phase === 'load-error' || phase === 'saving'
        }
        aria-describedby={hint}
        onChange={(event) => onChange(resolveTheme(event.currentTarget.value))}
      >
        {themes.map((option) => (
          <option key={option.id} value={option.id}>
            {option.name}
          </option>
        ))}
      </select>
      <small id={hint} aria-live="polite" aria-atomic="true">
        {phase === 'loading' && 'Loading saved appearance…'}
        {phase === 'preview' && 'Browser-only preview · changes are not saved'}
        {phase === 'ready' && 'Saved with this app folder'}
        {phase === 'saving' && 'Saving appearance…'}
        {phase === 'load-error' &&
          'Saved appearance could not be read. Selection is unavailable.'}
        {phase === 'save-error' &&
          'This appearance is not saved. It applies to this session only.'}
      </small>
      {(phase === 'load-error' || phase === 'save-error') && (
        <button className="secondary" onClick={onRetry}>
          {phase === 'load-error'
            ? 'Retry appearance load'
            : 'Save appearance again'}
        </button>
      )}
    </div>
  );
}
