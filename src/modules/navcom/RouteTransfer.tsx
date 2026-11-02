import { useEffect, useRef, useState } from 'react';
import {
  cancelRoute,
  MAX_ROUTE_POSTS,
  prepareRoute,
  sendRoute,
} from '../../services/eveRoute';
import type { RouteConfirmation, RouteReport } from '../../services/eveRoute';
import { errorCode, eveMessages, eveSession } from '../../services/eveSession';
import type { Session } from './EveSession';
export function RouteTransfer({
  systems,
  session,
  name,
}: {
  systems: readonly number[];
  session: Session;
  name: (id: number) => string | undefined;
}) {
  const [confirmation, setConfirmation] = useState<RouteConfirmation | null>(
    null,
  );
  const [report, setReport] = useState<RouteReport | null>(null);
  const [message, setMessage] = useState('');
  const [busy, setBusy] = useState(false);
  const mounted = useRef(true);
  const flight = useRef(false);
  const pending = useRef<string | null>(null);
  const confirmButton = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
      if (pending.current) void cancelRoute(pending.current).catch(() => {});
    };
  }, []);
  useEffect(() => {
    if (!confirmation) return;
    confirmButton.current?.focus();
    const timer = setTimeout(
      () => {
        void cancelRoute(confirmation.id).catch(() => {});
        pending.current = null;
        setConfirmation(null);
        setMessage(
          'Confirmation or location freshness expired. Refresh current location and recalculate.',
        );
      },
      Math.max(0, confirmation.expires_at * 1000 - Date.now()),
    );
    return () => clearTimeout(timer);
  }, [confirmation]);
  const location = session.view.location;
  const unavailable =
    systems.length < 2
      ? 'Calculate a route with at least one jump.'
      : systems.length - 1 > MAX_ROUTE_POSTS
        ? 'Full transfer is limited to 10 requests. Use Set Destination for this longer route.'
        : session.view.phase !== 'connected'
          ? 'Connect a character first.'
          : !location || location.stale
            ? 'Refresh current location explicitly, use current system and recalculate.'
            : location.system_id !== systems[0]
              ? 'The route origin differs from the connected character’s observed location. Refresh, use current system and recalculate.'
              : '';
  const available = !unavailable && !busy && !session.busy;
  async function prepare() {
    if (!available || flight.current) return;
    flight.current = true;
    setBusy(true);
    setMessage('');
    setReport(null);
    try {
      const c = await prepareRoute(systems);
      if (c.character.id !== session.view.character?.id)
        throw 'invalid-confirmation';
      if (mounted.current) {
        pending.current = c.id;
        setConfirmation(c);
      } else {
        void cancelRoute(c.id).catch(() => {});
      }
    } catch (e) {
      if (mounted.current) setMessage(eveMessages[errorCode(e)]);
    } finally {
      flight.current = false;
      if (mounted.current) setBusy(false);
    }
  }
  function cancel() {
    const id = pending.current;
    pending.current = null;
    setConfirmation(null);
    if (id) void cancelRoute(id).catch(() => {});
  }
  async function confirm() {
    if (!available || flight.current || !confirmation) return;
    const c = confirmation;
    if (c.character.id !== session.view.character?.id) {
      cancel();
      setMessage(eveMessages['invalid-confirmation']);
      return;
    }
    flight.current = true;
    setBusy(true);
    setConfirmation(null);
    setMessage('');
    await session.perform(async () => {
      try {
        const result = await sendRoute(c.id);
        if (mounted.current) setReport(result);
        return await eveSession.status();
      } catch (e) {
        if (mounted.current) setMessage(eveMessages[errorCode(e)]);
        throw e;
      } finally {
        pending.current = null;
      }
    });
    flight.current = false;
    if (mounted.current) setBusy(false);
  }
  return (
    <section
      className="navcom-waypoints"
      aria-label="Transfer planned waypoints"
    >
      <h3>Transfer planned waypoints</h3>
      <p>
        Up to 10 systems after origin, in calculated order. EVE may choose
        different paths between waypoints. This does not activate autopilot or
        control your ship.
      </p>
      <button
        className="secondary"
        disabled={!available}
        onClick={() => void prepare()}
      >
        Set Route in EVE…
      </button>
      {unavailable && <p>{unavailable}</p>}
      {confirmation && (
        <div
          role="alertdialog"
          aria-label="Confirm EVE route transfer"
          aria-describedby="route-transfer-detail"
        >
          <p id="route-transfer-detail">
            Character {confirmation.character.name} (ID{' '}
            {confirmation.character.id}). Origin{' '}
            {name(systems[0]!) ?? 'Unnamed system'} ({systems[0]}) → target{' '}
            {name(systems[systems.length - 1]!) ?? 'Unnamed system'} (
            {systems[systems.length - 1]}). {systems.length - 1} jumps ·{' '}
            {systems.length - 1} ESI requests.
          </p>
          <p>
            The first accepted request clears existing EVE waypoints. Partial
            delivery cannot be rolled back. Leaving this view stops later sends
            where possible; an in-flight request may complete. Inspect EVE’s
            queue after any result. Confirmation expires with native location
            freshness, at most 30 seconds.
          </p>
          <ol>
            {confirmation.systems.slice(1).map((id) => (
              <li key={id}>
                {name(id) ?? 'Unnamed system'} · System ID {id}
              </li>
            ))}
          </ol>
          <button
            ref={confirmButton}
            className="secondary"
            disabled={!available}
            onClick={() => void confirm()}
          >
            Confirm route transfer
          </button>
          <button className="secondary" onClick={cancel}>
            Cancel route transfer
          </button>
        </div>
      )}
      {busy && (
        <p role="status">
          Transferring planned waypoints serially. Disconnect stops later
          requests; an in-flight write may complete.
        </p>
      )}
      {message && <p role="alert">{message}</p>}
      {report && (
        <p role="status">
          {report.accepted_count} of {report.total} requests accepted by ESI.{' '}
          {report.delivery === 'all-accepted'
            ? 'All planned requests accepted; client application is not verified.'
            : report.delivery === 'unknown'
              ? 'Partial/unknown outcome. No automatic resumption or retry.'
              : 'Transfer stopped. No automatic resumption or rollback.'}{' '}
          Last API-accepted system: {report.last_accepted ?? 'none'}. Attempted
          failed/uncertain system: {report.failed_system ?? 'none'}. Next
          never-sent system: {report.next_unsent ?? 'none'}.{' '}
          {report.reason ? eveMessages[report.reason] : ''} Inspect EVE’s
          waypoint queue before any new request.
        </p>
      )}
    </section>
  );
}
