import type { ComponentType } from 'react';
import type { ModuleDefinition } from '../core/modules';
import { Icon } from './Icon';

export function ModuleView({
  module,
}: {
  module: ModuleDefinition<ComponentType>;
}) {
  if (module.availability === 'available') {
    const Entry = module.entry;
    return <Entry />;
  }

  return (
    <section className="empty-state" aria-labelledby="module-title">
      <div className="module-mark" aria-hidden="true">
        <Icon kind="navigation" />
      </div>
      <span className="badge">
        {module.availability === 'planned' ? 'Planned' : 'Unavailable'}
      </span>
      <h2 id="module-title">{module.name}</h2>
      <p>{module.summary}</p>
      <p className="availability-reason">{module.reason}</p>
    </section>
  );
}
