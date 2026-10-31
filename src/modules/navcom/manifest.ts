import type { ComponentType } from 'react';
import type { ModuleDefinition } from '../../core/modules';

export const navcomManifest = {
  id: 'navcom',
  name: 'NAVCOM',
  summary: 'Navigation and route planning for New Eden.',
  availability: 'planned',
  reason:
    'Not yet implemented. Route planning and EVE connectivity are unavailable.',
} as const satisfies ModuleDefinition<ComponentType>;
