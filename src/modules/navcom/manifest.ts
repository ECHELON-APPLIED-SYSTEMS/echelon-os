import { Navcom } from './Navcom';
import type { ComponentType } from 'react';
import type { ModuleDefinition } from '../../core/modules';

export const navcomManifest = {
  id: 'navcom',
  name: 'NAVCOM',
  summary: 'Navigation and route planning for New Eden.',
  availability: 'available',
  entry: Navcom,
} as const satisfies ModuleDefinition<ComponentType>;
