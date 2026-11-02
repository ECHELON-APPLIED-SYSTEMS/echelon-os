import { useEffect, useRef, useState } from 'react';
import { errorCode, eveMessages, eveSession } from '../../services/eveSession';
import type { Action, Confirmation } from '../../services/eveSession';

import type { useEveSession } from './useEveSession';
export type Session = ReturnType<typeof useEveSession>;
const at = (seconds: number) => new Date(seconds * 1000).toLocaleString();
export function EveConnection({ session }: { session: Session }) {
  const { view, busy, message, perform, disconnect } = session;
  const error = message || (view.error ? eveMessages[view.error] : '');
  return (
    <section className="navcom-session" aria-label="EVE character session">
      <h3>EVE character session</h3>
      <p>
        Optional, session-only connection. Sign-in opens your system browser.
        Offline routing works independently.
      </p>
      {!eveSession.desktop() && <p>Browser preview cannot sign in to EVE.</p>}
      <p role="status">
        {view.phase === 'connected' && view.character
          ? `Connected: ${view.character.name} · Character ID ${view.character.id}`
          : view.phase === 'connecting'
            ? 'Waiting for EVE sign-in in your browser…'
            : view.phase === 'expired'
              ? 'Session expired.'
              : 'No connected character.'}
      </p>
      {view.phase !== 'connected' && view.phase !== 'connecting' && (
        <button
          className="secondary"
          disabled={busy || !eveSession.desktop()}
          onClick={() => void perform(eveSession.connect, true)}
        >
          Connect EVE character
        </button>
      )}
      {(view.phase === 'connected' || view.phase === 'connecting') && (
        <button className="secondary" onClick={() => void disconnect()}>
          {view.phase === 'connecting'
            ? 'Cancel sign-in'
            : 'Disconnect EVE character'}
        </button>
      )}
      {view.phase === 'connected' && (
        <>
          <button
            className="secondary"
            disabled={busy || view.retry_at !== null}
            onClick={() => void perform(eveSession.refresh)}
          >
            Refresh current location
          </button>
          <p>
            Session expires at {at(view.expires_at!)}. Sign in again after
            expiry; credentials are not saved. Disconnect clears the local
            session; an already sent request may still complete.
          </p>
          {view.location ? (
            <p role="status">
              ESI system ID {view.location.system_id} · response fetched{' '}
              {at(view.location.fetched_at)}
              {view.location.server_as_of !== null
                ? ` · server data as of ${at(view.location.server_as_of)}`
                : ' · server observation time unavailable'}
              {view.location.cached
                ? ' · reused cached observation'
                : ' · ESI may serve cached data'}
              {view.location.stale
                ? ' · stale; refresh before relying on location.'
                : ` · cache freshness ends ${at(view.location.fresh_until)}; not instantaneous.`}
            </p>
          ) : (
            <p>
              No location requested. Refresh explicitly to read the connected
              character’s system.
            </p>
          )}
        </>
      )}
      {busy && view.phase === 'connected' && (
        <p role="status">EVE operation in progress…</p>
      )}
      {view.retry_at !== null && (
        <p role="status">
          ESI retry permitted after {at(view.retry_at)}. No automatic retries.
        </p>
      )}
      {error && <p role="alert">{error}</p>}
    </section>
  );
}

export function WaypointActions({
  session,
  destination,
  name,
}: {
  session: Session;
  destination: number | null;
  name: string | undefined;
}) {
  const [confirmation, setConfirmation] = useState<Confirmation | null>(null);
  const [message, setMessage] = useState('');
  const [busy, setBusy] = useState(false);
  const flight = useRef(false);
  const mounted = useRef(true);
  const confirmButton = useRef<HTMLButtonElement>(null);
  const initiator = useRef<HTMLButtonElement | null>(null);
  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
    };
  }, []);
  useEffect(() => {
    if (!confirmation) return;
    confirmButton.current?.focus();
    const timer = setTimeout(
      () => {
        initiator.current?.focus();
        setConfirmation(null);
        setMessage('Confirmation expired. Review a new request.');
      },
      Math.max(0, confirmation.expires_at * 1000 - Date.now()),
    );
    return () => clearTimeout(timer);
  }, [confirmation]);
  const available =
    session.view.phase === 'connected' &&
    destination !== null &&
    !session.busy &&
    !busy;
  async function prepare(action: Action) {
    if (!available || flight.current || destination === null) return;
    flight.current = true;
    setBusy(true);
    setMessage('');
    try {
      const result = await eveSession.prepare(destination, action);
      if (
        result.character.id !== session.view.character?.id ||
        result.destination !== destination ||
        result.action !== action
      )
        throw 'invalid-confirmation';
      if (mounted.current) setConfirmation(result);
    } catch (e) {
      if (mounted.current) setMessage(eveMessages[errorCode(e)]);
    } finally {
      flight.current = false;
      if (mounted.current) setBusy(false);
    }
  }
  async function confirm() {
    if (!available || flight.current || !confirmation) return;
    if (
      confirmation.destination !== destination ||
      confirmation.character.id !== session.view.character?.id
    ) {
      setConfirmation(null);
      setMessage(eveMessages['invalid-confirmation']);
      return;
    }
    flight.current = true;
    setBusy(true);
    setConfirmation(null);
    setMessage('');
    await session.perform(async () => {
      try {
        const result = await eveSession.send(confirmation.id);
        if (mounted.current)
          setMessage(
            'Request accepted by ESI. Check EVE; application by the client is not confirmed.',
          );
        return result;
      } catch (e) {
        if (mounted.current) setMessage(eveMessages[errorCode(e)]);
        throw e;
      }
    });
    flight.current = false;
    if (mounted.current) setBusy(false);
  }
  return (
    <section className="navcom-waypoints" aria-label="EVE destination requests">
      <h3>Send selected destination to EVE</h3>
      <p>
        Only the selected system is sent. No intermediate route, ship movement
        or autopilot activation.
      </p>
      <button
        className="secondary"
        disabled={!available}
        onClick={(e) => {
          initiator.current = e.currentTarget;
          void prepare('set-destination');
        }}
      >
        Set Destination…
      </button>
      <button
        className="secondary"
        disabled={!available}
        onClick={(e) => {
          initiator.current = e.currentTarget;
          void prepare('add-waypoint');
        }}
      >
        Add Waypoint…
      </button>
      {confirmation && (
        <div
          role="alertdialog"
          aria-label="Confirm EVE waypoint request"
          aria-describedby="eve-confirm-details"
        >
          <p id="eve-confirm-details">
            {confirmation.action === 'set-destination'
              ? 'Set destination'
              : 'Add waypoint'}{' '}
            for {confirmation.character.name} (Character ID{' '}
            {confirmation.character.id}): {name ?? 'Unnamed system'} (System ID{' '}
            {confirmation.destination}).
            {confirmation.action === 'set-destination'
              ? ' This clears all existing waypoints.'
              : ' This preserves existing waypoints and appends to the queue.'}{' '}
            This request does not activate autopilot. Confirm within 30 seconds.
          </p>
          <button
            ref={confirmButton}
            className="secondary"
            disabled={!available}
            onClick={() => {
              initiator.current?.focus();
              void confirm();
            }}
          >
            Confirm EVE request
          </button>
          <button
            className="secondary"
            disabled={busy}
            onClick={() => {
              initiator.current?.focus();
              setConfirmation(null);
            }}
          >
            Cancel request
          </button>
        </div>
      )}
      {busy && <p role="status">Processing EVE request…</p>}
      {message && <p role="status">{message}</p>}
    </section>
  );
}
