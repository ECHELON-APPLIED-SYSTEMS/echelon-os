import { useEffect, useRef, useState } from 'react';
import { NAVCOM_PIN } from '../../services/navcomSnapshot';
import {
  canInstallRoutes,
  installError,
  installMessages,
  installRouteDatabase,
  routeInstallStatus,
} from '../../services/routeInstall';
import type { InstallPhase } from '../../services/routeInstall';
const progress: Record<InstallPhase, string> = {
  idle: 'Starting the explicit route-data request…',
  downloading: 'Downloading the pinned official CCP archive…',
  verifying: 'Verifying hashes and converting static route data…',
  installing: 'Publishing the validated private snapshot…',
  ready: 'Re-reading and validating installed route data…',
  error: 'The native operation is finishing; waiting for its result…',
};
export function RouteDataManager({
  onInstalled,
}: {
  onInstalled: () => Promise<void>;
}) {
  const [busy, setBusy] = useState(false);
  const [phase, setPhase] = useState<InstallPhase>('idle');
  const [message, setMessage] = useState('');
  const flight = useRef(false);
  const mounted = useRef(true);
  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
    };
  }, []);
  useEffect(() => {
    if (!busy) return;
    let active = true;
    let pending = false;
    const timer = setInterval(() => {
      if (pending) return;
      pending = true;
      void routeInstallStatus()
        .then((p) => {
          if (active) setPhase(p);
        })
        .catch(() => {
          clearInterval(timer);
        })
        .finally(() => {
          pending = false;
        });
    }, 400);
    return () => {
      active = false;
      clearInterval(timer);
    };
  }, [busy]);
  async function download() {
    if (flight.current) return;
    flight.current = true;
    setBusy(true);
    setPhase('idle');
    setMessage('');
    try {
      await installRouteDatabase();
      if (mounted.current) {
        setPhase('ready');
        await onInstalled();
      }
    } catch (e) {
      if (mounted.current) setMessage(installMessages[installError(e)]);
    } finally {
      flight.current = false;
      if (mounted.current) setBusy(false);
    }
  }
  return (
    <section className="navcom-session" aria-label="NAVCOM Route Database">
      <h3>NAVCOM Route Database</h3>
      <p>
        Not installed. Download approximately 99 MB from official CCP static
        data, approved build {NAVCOM_PIN.build}. Only the validated converted
        snapshot is stored privately beside this application.
      </p>
      <p>
        Use a writable local extracted app folder outside source control and
        cloud-synced folders. This pinned build is static; updates are not
        automatic.
      </p>
      <p>
        <a
          href="https://developers.eveonline.com/docs/services/static-data/"
          target="_blank"
          rel="noreferrer"
        >
          Official pinned source
        </a>
        {' · '}
        <a
          href="https://developers.eveonline.com/license-agreement"
          target="_blank"
          rel="noreferrer"
        >
          CCP Developer License Agreement
        </a>
      </p>
      <p>
        CCP’s terms apply to this third-party, non-commercial player-support
        tool. © 2014 CCP hf. All rights reserved. EVE, EVE Online and CCP are
        CCP trademarks. No CCP affiliation or endorsement.
      </p>
      <button
        className="secondary"
        disabled={busy || !canInstallRoutes()}
        onClick={() => void download()}
      >
        Download Route Data
      </button>
      {busy && (
        <p role="status">
          {progress[phase]} The download request has a 120-second timeout;
          verification and installation take additional time. It is not retried
          automatically.
        </p>
      )}
      {message && <p role="alert">{message}</p>}
    </section>
  );
}
