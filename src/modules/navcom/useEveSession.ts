import { useCallback, useEffect, useRef, useState } from 'react';
import {
  disconnected,
  errorCode,
  eveMessages,
  eveSession,
} from '../../services/eveSession';
import type { View } from '../../services/eveSession';

export function useEveSession() {
  const [view, setView] = useState<View>(disconnected);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState('');
  const flight = useRef(false);
  const generation = useRef(0);
  const mounted = useRef(true);
  useEffect(() => {
    mounted.current = true;
    const epoch = ++generation.current;
    void eveSession
      .status()
      .then((v) => {
        if (mounted.current && epoch === generation.current) setView(v);
      })
      .catch((e) => {
        if (mounted.current && epoch === generation.current)
          setMessage(eveMessages[errorCode(e)]);
      });
    return () => {
      mounted.current = false;
    };
  }, []);
  // Local-only expiry/cache timers. No automatic HTTP, login or polling.
  useEffect(() => {
    const expiry = view.expires_at;
    if (!expiry) return;
    const timer = setTimeout(
      () => {
        setView({ ...disconnected, phase: 'expired', error: 'expired' });
        setMessage(eveMessages.expired);
      },
      Math.max(0, expiry * 1000 - Date.now()),
    );
    return () => clearTimeout(timer);
  }, [view.expires_at]);
  useEffect(() => {
    if (!view.location || view.location.stale) return;
    const timer = setTimeout(
      () =>
        setView((v) =>
          v.location
            ? {
                ...v,
                location: { ...v.location, stale: true },
              }
            : v,
        ),
      Math.max(0, view.location.fresh_until * 1000 - Date.now()),
    );
    return () => clearTimeout(timer);
  }, [view.location]);
  useEffect(() => {
    if (view.retry_at === null) return;
    const timer = setTimeout(
      () => setView((v) => ({ ...v, retry_at: null })),
      Math.max(0, view.retry_at * 1000 - Date.now()),
    );
    return () => clearTimeout(timer);
  }, [view.retry_at]);
  const perform = useCallback(
    async (operation: () => Promise<View>, connecting = false) => {
      if (flight.current) return;
      flight.current = true;
      setBusy(true);
      setMessage('');
      const epoch = ++generation.current;
      if (connecting) setView({ ...disconnected, phase: 'connecting' });
      try {
        const result = await operation();
        if (mounted.current && epoch === generation.current) setView(result);
      } catch (e) {
        // Safe enum only; never display raw IPC/HTTP errors.
        const code = errorCode(e);
        try {
          const current = await eveSession.status();
          if (mounted.current && epoch === generation.current) setView(current);
        } catch {
          /* Preserve last known view with explicit error. */
        }
        if (mounted.current && epoch === generation.current)
          setMessage(eveMessages[code]);
      } finally {
        flight.current = false;
        if (mounted.current) setBusy(false);
      }
    },
    [],
  );
  async function disconnect() {
    const epoch = ++generation.current; // Late connect/HTTP responses cannot resurrect the UI.
    setMessage('');
    try {
      const result = await eveSession.disconnect();
      if (mounted.current && epoch === generation.current) setView(result);
    } catch (e) {
      if (mounted.current && epoch === generation.current)
        setMessage(eveMessages[errorCode(e)]);
    }
  }
  return { view, busy, message, perform, disconnect };
}
