import { useLayoutEffect } from 'react';
import type { ThemeId } from './themes';

/** One document per app window. State is owned by App; this effect never persists it. */
export function useDocumentTheme(theme: ThemeId) {
  useLayoutEffect(() => {
    const root = document.documentElement;
    const previous = root.getAttribute('data-theme');
    root.setAttribute('data-theme', theme);
    return () => {
      if (root.getAttribute('data-theme') !== theme) return;
      if (previous === null) root.removeAttribute('data-theme');
      else root.setAttribute('data-theme', previous);
    };
  }, [theme]);
}
