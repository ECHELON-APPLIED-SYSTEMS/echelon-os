import { useState } from 'react';
import { Appearance } from '../themes/Appearance';
import { useDocumentTheme } from '../themes/useDocumentTheme';
import { defaultTheme, type ThemeId } from '../themes/themes';
import { moduleRegistry } from '../modules/registry';
import { ModuleView } from './ModuleView';
import { RuntimeStatus } from './RuntimeStatus';
import { Icon } from './Icon';

const modules = moduleRegistry.list();
const availabilityLabels = {
  available: 'Ready',
  planned: 'Planned',
  disabled: 'Unavailable',
} as const;

export function App() {
  const [theme, setTheme] = useState<ThemeId>(defaultTheme);
  useDocumentTheme(theme);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const selectedModule = selectedId
    ? moduleRegistry.get(selectedId)
    : undefined;

  return (
    <div className="app-shell">
      <a className="skip-link" href="#main-content">
        Skip to content
      </a>
      <aside className="sidebar">
        <div className="brand">
          <span className="brand-mark" aria-hidden="true">
            E
          </span>
          <div>
            ECHELON <span className="brand-os">OS</span>
            <small>Operations workspace</small>
          </div>
        </div>
        <nav aria-label="Workspace">
          <p className="nav-label">Workspace</p>
          <button
            className="nav-item"
            aria-current={selectedId === null ? 'page' : undefined}
            onClick={() => setSelectedId(null)}
          >
            <Icon kind="overview" />
            Overview
          </button>
          <p className="nav-label modules-label">Modules</p>
          {modules.map((module) => (
            <button
              key={module.id}
              className="nav-item"
              aria-current={selectedId === module.id ? 'page' : undefined}
              onClick={() => setSelectedId(module.id)}
            >
              <Icon kind="navigation" />
              {module.name}
              <span className="nav-badge">
                {availabilityLabels[module.availability]}
              </span>
            </button>
          ))}
        </nav>
        <Appearance theme={theme} onChange={setTheme} />
        <div className="sidebar-footer">
          <span className="status-dot" aria-hidden="true" />
          Desktop foundation<small>M1-A · Development build</small>
        </div>
      </aside>
      <div className="workspace">
        <header className="topbar">
          <span>
            Workspace{' '}
            <span className="breadcrumb" aria-hidden="true">
              /
            </span>{' '}
            <strong>{selectedModule?.name ?? 'Overview'}</strong>
          </span>
          <span className="build-label">
            ECHELON OS <span className="badge">M1-A</span>
          </span>
        </header>
        <main id="main-content" tabIndex={-1}>
          <div className="page-heading">
            <p className="eyebrow">Your operations, organised</p>
            <h1>{selectedModule?.name ?? 'Workspace overview'}</h1>
            <p>
              {selectedModule
                ? 'Module information and availability.'
                : 'A foundation for your New Eden operations.'}
            </p>
          </div>
          {selectedModule ? (
            <ModuleView module={selectedModule} />
          ) : (
            <>
              <section
                className="welcome-panel"
                aria-labelledby="welcome-title"
              >
                <div>
                  <span className="eyebrow">Desktop foundation</span>
                  <h2 id="welcome-title">Your workspace starts here.</h2>
                  <p>
                    The ECHELON OS shell is ready to explore. Operational
                    modules will become available as they are built and
                    verified.
                  </p>
                </div>
                <div className="orbital-mark" aria-hidden="true">
                  <span>E</span>
                </div>
              </section>
              <section
                className="module-section"
                aria-labelledby="modules-title"
              >
                <div className="section-heading">
                  <h2 id="modules-title">Modules</h2>
                  <span className="eyebrow">
                    {
                      modules.filter(
                        (module) => module.availability === 'planned',
                      ).length
                    }{' '}
                    planned ·{' '}
                    {
                      modules.filter(
                        (module) => module.availability === 'available',
                      ).length
                    }{' '}
                    operational
                  </span>
                </div>
                <div className="module-grid">
                  {modules.length === 0 && (
                    <p>No modules are registered in this build.</p>
                  )}
                  {modules.map((module) => (
                    <article className="module-card" key={module.id}>
                      <div className="section-heading">
                        <span className="module-symbol" aria-hidden="true">
                          <Icon kind="navigation" />
                        </span>
                        <span className="badge">
                          {availabilityLabels[module.availability]}
                        </span>
                      </div>
                      <h3>{module.name}</h3>
                      <p>{module.summary}</p>
                      <p className="module-note">
                        {module.availability === 'available'
                          ? 'Ready to open'
                          : module.availability === 'planned'
                            ? 'Not yet implemented'
                            : module.reason}
                      </p>
                      <button
                        className="secondary"
                        onClick={() => setSelectedId(module.id)}
                      >
                        View module details <span aria-hidden="true">→</span>
                      </button>
                    </article>
                  ))}
                </div>
              </section>
              <RuntimeStatus />
              <p className="scope-note">
                No EVE connection is configured. Workspaces and settings are not
                saved in this build.
              </p>
            </>
          )}
        </main>
      </div>
    </div>
  );
}
