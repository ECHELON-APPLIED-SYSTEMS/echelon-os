import { useEffect, useState } from 'react';
import { readAppMetadata, type MetadataResult } from '../services/appMetadata';

type RuntimeState = MetadataResult | { kind: 'loading' } | { kind: 'error' };

export function RuntimeStatus() {
  const [state, setState] = useState<RuntimeState>({ kind: 'loading' });
  const [attempt, setAttempt] = useState(0);

  useEffect(() => {
    let active = true;
    readAppMetadata().then(
      (result) => {
        if (active) setState(result);
      },
      () => {
        if (active) setState({ kind: 'error' });
      },
    );
    return () => {
      active = false;
    };
  }, [attempt]);

  return (
    <section className="runtime" aria-labelledby="runtime-title">
      <div className="section-heading">
        <h2 id="runtime-title">Desktop status</h2>
        <span className="eyebrow">Local runtime</span>
      </div>
      <div aria-live="polite" aria-atomic="true">
        {state.kind === 'loading' && (
          <p role="status">Checking desktop connection…</p>
        )}
        {state.kind === 'preview' && (
          <p>
            Browser preview · Desktop connection unavailable. Open the desktop
            app to verify its runtime.
          </p>
        )}
        {state.kind === 'desktop' && (
          <>
            <p className="runtime-connected">Desktop connected</p>
            <dl className="metadata">
              <div>
                <dt>Application</dt>
                <dd>{state.metadata.name}</dd>
              </div>
              <div>
                <dt>Version</dt>
                <dd>{state.metadata.version}</dd>
              </div>
              <div>
                <dt>Platform</dt>
                <dd>{state.metadata.platform}</dd>
              </div>
              <div>
                <dt>Build stage</dt>
                <dd>{state.metadata.milestone}</dd>
              </div>
            </dl>
          </>
        )}
        {state.kind === 'error' && (
          <div>
            <p role="alert">
              Desktop status could not be read. Try again or restart the
              application.
            </p>
            <button
              className="secondary"
              onClick={() => {
                setState({ kind: 'loading' });
                setAttempt(attempt + 1);
              }}
            >
              Retry connection
            </button>
          </div>
        )}
      </div>
    </section>
  );
}
