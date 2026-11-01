import { useState } from 'react';
import { readNavcomSnapshot } from '../../services/navcomSnapshot';
import type { NavcomSnapshotResult } from '../../services/navcomSnapshot';
import type { RouteResult } from './routing';

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
}: {
  label: string;
  snapshot: Loaded;
  onSelect: (id: number | null) => void;
}) {
  const [query, setQuery] = useState('');
  const [selected, setSelected] = useState('');
  const matches = snapshot.searchSystems(query);
  return (
    <fieldset className="navcom-endpoint">
      <legend>{label}</legend>
      <label>
        Search {label.toLowerCase()} by name or ID
        <input
          type="search"
          value={query}
          maxLength={64}
          autoComplete="off"
          onChange={(e) => {
            setQuery(e.target.value);
            setSelected('');
            onSelect(null);
          }}
        />
      </label>
      <label>
        {label} system
        <select
          value={selected}
          onChange={(e) => {
            const match = matches.find(
              (system) => String(system.id) === e.target.value,
            );
            setSelected(match ? String(match.id) : '');
            onSelect(match?.id ?? null);
          }}
        >
          <option value="">Choose a system</option>
          {matches.map((system) => (
            <option key={system.id} value={system.id}>
              {system.name ?? 'Unnamed system'} — {system.id}
            </option>
          ))}
        </select>
      </label>
      <small>
        {!query.trim()
          ? 'Enter a name or numeric system ID.'
          : matches.length === 0
            ? 'No matching systems in this snapshot.'
            : 'Up to 20 matches. Refine your search, then select a system.'}
      </small>
    </fieldset>
  );
}

function Planner({ snapshot }: { snapshot: Loaded }) {
  const [origin, setOrigin] = useState<number | null>(null);
  const [destination, setDestination] = useState<number | null>(null);
  const [route, setRoute] = useState<RouteResult | null>(null);
  const [error, setError] = useState(false);
  const [visible, setVisible] = useState(100);
  function change(set: (id: number | null) => void, id: number | null) {
    set(id);
    setRoute(null);
    setError(false);
    setVisible(100);
  }
  function calculate() {
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
      <div className="navcom-endpoints">
        <Endpoint
          label="Origin"
          snapshot={snapshot}
          onSelect={(id) => change(setOrigin, id)}
        />
        <Endpoint
          label="Destination"
          snapshot={snapshot}
          onSelect={(id) => change(setDestination, id)}
        />
      </div>
      <button
        className="secondary"
        disabled={origin === null || destination === null}
        onClick={calculate}
      >
        Calculate minimum-hop route
      </button>
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
      <h2 id="navcom-title">Stargate route planner</h2>
      <p>
        Minimum hops through static stargates. This is not a safest-route
        calculation. Wormholes, Ansiblex, current gate changes and live risk are
        not included.
      </p>
      <button
        className="secondary"
        disabled={phase.kind === 'loading'}
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
        <p role="status">
          Route data is not provisioned. Use the documented private snapshot
          procedure for this application. No file was created.
        </p>
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
          <Planner snapshot={phase.snapshot} />
        </>
      )}
      <p className="scope-note">
        No live ESI connection or character location is read. Routes are not
        sent to EVE. Compare them in game with matching shortest-route and
        avoidance settings.
      </p>
    </section>
  );
}
