import type { ComponentType } from 'react';
import { ModuleRegistry } from '../core/modules';
import { navcomManifest } from './navcom/manifest';

export const moduleRegistry = new ModuleRegistry<ComponentType>([
  navcomManifest,
]);
