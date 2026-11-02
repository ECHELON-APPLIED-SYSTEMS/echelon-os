import { useEffect, useId, useRef, useState } from 'react';
import { readNavcomSnapshot } from '../../services/navcomSnapshot';
import type { NavcomSnapshotResult } from '../../services/navcomSnapshot';
import type { RouteResult } from './routing';
import { EveConnection, WaypointActions } from './EveSession';
import type { Session } from './EveSession';
import { useEveSession } from './useEveSession';
import { RouteDataManager } from './RouteDataManager';
import { RouteTransfer } from './RouteTransfer';

type Loaded = Extract<NavcomSnapshotResult, { kind: 'ready' }>['snapshot'];
type Phase = NavcomSnapshotResult | { kind: 'idle' | 'loading' };
const errorMessages = {
  'unsafe-path':
    'Route data was refused because its location is unsafe. Preserve the portable folder and check for links or junctions.',
  unreadable:
    'Route data could not be read. Check file access, other processes and whether the local filesystem supports directory identity checks.',
  oversize:
    'The route data file exceeds the supported size. Restore the reviewed snapshot; it has not been changed.',
  'invalid-utf8':
    'Route data has invalid text encoding. Restore the reviewed snapshot; it has not been changed.',
  changed:
    'The portable folder or route data changed during reading. Close other copies and retry with the complete folder intact.',
  'invalid-snapshot':
    'Route data failed integrity, build or topology validation. Use the snapshot matching this application; it has not been reset.',
  'invalid-response':
    'The desktop returned an unsupported route data response. Use a matching application version.',
  unavailable:
    'The desktop route data service is unavailable. Retry from the desktop application.',
};

function Endpoint({
  label,
  snapshot,
  onSelect,
  preset,
  disabled = false,
}: {
  label: string;
  snapshot: Loaded;
  preset?: number | undefined;
  disabled?: boolean;
  onSelect: (id: number | null) => void;
}) {
  const [query, setQuery] = useState(
    preset
      ? `${snapshot.englishName(preset) ?? 'Unnamed system'} — ${preset}`
      : '',
  );
  const [selected, setSelected] = useState<
    ReturnType<Loaded['searchSystems']>[number] | null
  >(preset ? { id: preset, name: snapshot.englishName(preset) } : null);
  const [open, setOpen] = useState(false);
  const [active, setActive] = useState(-1);
  const input = useRef<HTMLInputElement>(null);
  const list = useRef<HTMLUListElement>(null);
  const id = useId();
  const listId = `${id}-matches`;
  const hintId = `${id}-hint`;
  const matches = snapshot.searchSystems(
    selected ? String(selected.id) : query,
  );
  const expanded = open && query.trim().length > 0;
  const activeOption = expanded ? matches[active] : undefined;
  const optionId = (systemId: number) => `${id}-system-${systemId}`;
  useEffect(() => {
    if (expanded && active >= 0)
      list.current?.children
        .item(active)
        ?.scrollIntoView?.({ block: 'nearest' });
  }, [active, expanded]);
  function choose(system: (typeof matches)[number]) {
    input.current?.focus();
    setQuery(system.name ? `${system.name} — ${system.id}` : String(system.id));
    setSelected(system);
    setOpen(false);
    setActive(-1);
    onSelect(system.id);
  }
  return (
    <div className="navcom-endpoint">
      <label htmlFor={id}>{label} system</label>
      <div className="navcom-combobox">
        <input
          disabled={disabled}
          ref={input}
          id={id}
          role="combobox"
          type="text"
          value={query}
          maxLength={64}
          autoComplete="off"
          spellCheck={false}
          aria-autocomplete="list"
          aria-expanded={expanded}
          aria-controls={expanded ? listId : undefined}
          aria-activedescendant={
            activeOption ? optionId(activeOption.id) : undefined
          }
          aria-describedby={hintId}
          onFocus={() => {
            setOpen(true);
            setActive(-1);
          }}
          onBlur={() => {
            setOpen(false);
            setActive(-1);
          }}
          onChange={(e) => {
            setQuery(e.target.value);
            setSelected(null);
            setActive(-1);
            setOpen(true);
            onSelect(null);
          }}
          onKeyDown={(e) => {
            if (e.nativeEvent.isComposing) return;
            if (e.key === 'Escape') {
              setOpen(false);
              setActive(-1);
              return;
            }
            if (
              (e.key === 'ArrowDown' || e.key === 'ArrowUp') &&
              query.trim()
            ) {
              e.preventDefault();
              setOpen(true);
              setActive(
                matches.length === 0
                  ? -1
                  : e.key === 'ArrowDown'
                    ? (expanded ? active + 1 : 0) % matches.length
                    : (expanded && active >= 0
                        ? active - 1 + matches.length
                        : matches.length - 1) % matches.length,
              );
            } else if (e.key === 'Enter' && activeOption) {
              e.preventDefault();
              choose(activeOption);
            }
          }}
        />
        {expanded && (
          <div className="navcom-suggestions">
            <ul
              ref={list}
              id={listId}
              role="listbox"
              aria-label={`${label} matches`}
            >
              {matches.map((system, index) => (
                <li
                  id={optionId(system.id)}
                  key={system.id}
                  role="option"
                  aria-selected={selected?.id === system.id}
                  data-active={active === index ? 'true' : undefined}
                  onPointerDown={(e) => e.preventDefault()}
                  onMouseDown={(e) => e.preventDefault()}
                  onMouseMove={() => setActive(index)}
                  onClick={() => choose(system)}
                >
                  {system.name ?? 'Unnamed system'} <span>— {system.id}</span>
                </li>
              ))}
            </ul>
            {matches.length === 0 && (
              <p role="status">No matching systems in this snapshot.</p>
            )}
          </div>
        )}
      </div>
      <small id={hintId}>
        {selected
          ? `Selected system ID ${selected.id}.`
          : 'Type a name or ID, then choose a match. Up to 20 results.'}
      </small>
    </div>
  );
}

function Planner({
  snapshot,
  session,
}: {
  snapshot: Loaded;
  session: Session;
}) {
  const [preset, setPreset] = useState<{ id: number; revision: number } | null>(
    null,
  );
  const [origin, setOrigin] = useState<number | null>(null);
  const [destination, setDestination] = useState<number | null>(null);
  const [route, setRoute] = useState<RouteResult | null>(null);
  const [error, setError] = useState(false);
  const [visible, setVisible] = useState(100);
  const [routeRevision, setRouteRevision] = useState(0);
  function change(set: (id: number | null) => void, id: number | null) {
    set(id);
    setRoute(null);
    setError(false);
    setVisible(100);
  }
  function calculate() {
    setRouteRevision((v) => v + 1);
    if (origin === null || destination === null) return;
    try {
      setRoute(snapshot.graph.shortestRoute(origin, destination));
      setError(false);
    } catch {
      setRoute(null);
      setError(true);
    }
    setVisible(100);
  }
  return (
    <>
      {session.view.location && (
        <>
          <p>
            Observed location:{' '}
            {snapshot.englishName(session.view.location.system_id) ??
              'System not named in snapshot'}{' '}
            · System ID {session.view.location.system_id}.
          </p>
          <button
            className="secondary"
            disabled={
              session.busy ||
              !snapshot.hasSystem(session.view.location.system_id) ||
              session.view.location.stale
            }
            onClick={() => {
              const id = session.view.location!.system_id;
              if (!snapshot.hasSystem(id)) return;
              setPreset((p) => ({ id, revision: (p?.revision ?? 0) + 1 }));
              change(setOrigin, id);
            }}
          >
            Use Current System as origin
          </button>
          {!snapshot.hasSystem(session.view.location.system_id) && (
            <p>
              Observed system is absent from this static snapshot; it cannot be
              selected.
            </p>
          )}
        </>
      )}
      <div className="navcom-endpoints">
        <Endpoint
          key={preset?.revision ?? 0}
          preset={preset?.id}
          disabled={session.busy}
          label="Origin"
          snapshot={snapshot}
          onSelect={(id) => change(setOrigin, id)}
        />
        <Endpoint
          disabled={session.busy}
          label="Destination"
          snapshot={snapshot}
          onSelect={(id) => change(setDestination, id)}
        />
      </div>
      <button
        className="secondary"
        disabled={session.busy || origin === null || destination === null}
        onClick={calculate}
      >
        Calculate minimum-hop route
      </button>
      <WaypointActions
        key={`${destination}-${session.view.character?.id}-${session.view.phase}`}
        session={session}
        destination={destination}
        name={
          destination !== null ? snapshot.englishName(destination) : undefined
        }
      />
      {error && (
        <p role="alert">
          The selected route could not be calculated. Select both systems again.
        </p>
      )}
      {route && (
        <section aria-labelledby="route-heading" className="navcom-route">
          <h3 id="route-heading">Static route result</h3>
          {route.kind === 'no-route' ? (
            <p role="status">
              No static stargate route connects these systems in this snapshot.
            </p>
          ) : (
            <>
              <p role="status">
                {route.kind === 'same-system'
                  ? 'Already in the destination system. '
                  : ''}
                {route.jumps} jumps · {route.systems.length} systems
              </p>
              {route.kind === 'route' && (
                <RouteTransfer
                  key={routeRevision}
                  systems={route.systems}
                  session={session}
                  name={snapshot.englishName}
                />
              )}
              <ol className="navcom-steps">
                {route.systems.slice(0, visible).map((id) => (
                  <li key={id}>
                    <strong>
                      {snapshot.englishName(id) ?? 'Unnamed system'}
                    </strong>{' '}
                    <span>System ID {id}</span>
                  </li>
                ))}
              </ol>
              {visible < route.systems.length && (
                <button
                  className="secondary"
                  onClick={() => setVisible((v) => v + 100)}
                >
                  Show next route steps
                </button>
              )}
            </>
          )}
        </section>
      )}
    </>
  );
}

export function Navcom() {
  const session = useEveSession();
  const [phase, setPhase] = useState<Phase>({ kind: 'idle' });
  const [validatedAt, setValidatedAt] = useState('');
  async function load() {
    setPhase({ kind: 'loading' });
    try {
      const result = await readNavcomSnapshot();
      setPhase(result);
      if (result.kind === 'ready')
        setValidatedAt(new Date().toLocaleTimeString());
    } catch {
      setPhase({ kind: 'error', code: 'unavailable' });
    }
  }
  return (
    <section className="navcom-panel" aria-labelledby="navcom-title">
      <EveConnection session={session} />
      <h2 id="navcom-title">Stargate route planner</h2>
      <p>
        Minimum hops through static stargates. This is not a safest-route
        calculation. Wormholes, Ansiblex, current gate changes and live risk are
        not included.
      </p>
      <button
        className="secondary"
        disabled={phase.kind === 'loading' || session.busy}
        onClick={() => void load()}
      >
        {phase.kind === 'loading'
          ? 'Loading route data…'
          : phase.kind === 'ready'
            ? 'Reload route data'
            : phase.kind === 'idle'
              ? 'Load route data'
              : 'Retry loading'}
      </button>
      {phase.kind === 'idle' && (
        <p role="status">
          Load your privately provisioned, reviewed route data to begin. No data
          is downloaded.
        </p>
      )}
      {phase.kind === 'loading' && (
        <p role="status">Reading and validating route data…</p>
      )}
      {phase.kind === 'missing' && (
        <>
          <p role="status">
            Route data is not provisioned. No file was created.
          </p>
          <RouteDataManager onInstalled={load} />
        </>
      )}
      {phase.kind === 'preview' && (
        <p role="status">
          Browser preview cannot access private desktop route data. Open the
          desktop application to calculate genuine routes.
        </p>
      )}
      {phase.kind === 'error' && (
        <p role="alert">{errorMessages[phase.code]}</p>
      )}
      {phase.kind === 'ready' && (
        <>
          <p className="navcom-provenance" role="status">
            Static CCP SDE build {phase.snapshot.build} ·{' '}
            {phase.snapshot.systemCount} systems · Validated locally at{' '}
            {validatedAt}. This snapshot does not update live.
          </p>
          <Planner snapshot={phase.snapshot} session={session} />
        </>
      )}
      <p className="scope-note">
        Static routes and ESI observations are separate. EVE requests require a
        connected character and explicit confirmation. Compare routes in game
        with matching shortest-route and avoidance settings.
      </p>
    </section>
  );
}
