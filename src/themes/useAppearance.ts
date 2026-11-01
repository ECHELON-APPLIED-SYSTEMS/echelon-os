import { useEffect, useRef, useState } from 'react';
import { defaultTheme, type ThemeId } from './themes';
import { loadTheme, saveTheme } from './themeStorage';

export type AppearancePhase =
  'loading' | 'preview' | 'ready' | 'saving' | 'load-error' | 'save-error';

/** One selection source; no browser persistence, background polling or implicit retry. */
export function useAppearance() {
  const [state, setState] = useState<{
    theme: ThemeId;
    phase: AppearancePhase;
  }>({
    theme: defaultTheme,
    phase: 'loading',
  });
  const [attempt, setAttempt] = useState(0);
  const mounted = useRef(false);
  const saving = useRef(false);

  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
    };
  }, []);

  useEffect(() => {
    let active = true;
    loadTheme().then(
      (result) => {
        if (active)
          setState(
            result.kind === 'preview'
              ? { theme: defaultTheme, phase: 'preview' }
              : { theme: result.theme, phase: 'ready' },
          );
      },
      () => {
        if (active) setState({ theme: defaultTheme, phase: 'load-error' });
      },
    );
    return () => {
      active = false;
    };
  }, [attempt]);

  function select(theme: ThemeId) {
    if (
      saving.current ||
      state.phase === 'loading' ||
      state.phase === 'load-error'
    )
      return;
    if (state.phase === 'preview') {
      setState({ theme, phase: 'preview' });
      return;
    }
    saving.current = true;
    setState({ theme, phase: 'saving' });
    saveTheme(theme)
      .then(
        (result) => {
          if (mounted.current)
            setState({
              theme,
              phase: result === 'saved' ? 'ready' : 'preview',
            });
        },
        () => {
          if (mounted.current) setState({ theme, phase: 'save-error' });
        },
      )
      .finally(() => {
        saving.current = false;
      });
  }

  function retry() {
    if (state.phase === 'load-error') {
      setState({ theme: defaultTheme, phase: 'loading' });
      setAttempt((value) => value + 1);
    } else if (state.phase === 'save-error') select(state.theme);
  }
  return { ...state, select, retry };
}
