import { describe, expect, it } from 'vitest';
import { ModuleRegistry, type ModuleDefinition } from './modules';

const planned = {
  id: 'navcom',
  name: 'NAVCOM',
  summary: 'Navigation',
  availability: 'planned',
  reason: 'Not yet implemented',
} as const;

describe('ModuleRegistry', () => {
  it('supports registration, ordered listing and lookup without shared mutable manifests', () => {
    const source = { ...planned };
    const registry = new ModuleRegistry<string>([
      source,
      {
        id: 'test-module',
        name: 'Test',
        summary: 'Test entry',
        availability: 'available',
        entry: 'entry',
      },
    ]);
    source.name = 'changed' as typeof source.name;
    expect(registry.get('navcom')?.name).toBe('NAVCOM');
    expect(registry.get('missing')).toBeUndefined();
    expect(registry.list().map((module) => module.id)).toEqual([
      'navcom',
      'test-module',
    ]);
    expect(registry.get('test-module')).toMatchObject({ entry: 'entry' });
    expect(Object.isFrozen(registry.get('navcom'))).toBe(true);
    expect(Object.isFrozen(registry.list())).toBe(true);
  });

  it('accepts an empty registry and disabled modules with an explanation', () => {
    expect(new ModuleRegistry([]).list()).toEqual([]);
    expect(
      new ModuleRegistry([{ ...planned, availability: 'disabled' }]).get(
        'navcom',
      ),
    ).toMatchObject({ availability: 'disabled' });
  });

  it('rejects duplicate module IDs', () => {
    expect(() => new ModuleRegistry([planned, planned])).toThrow(
      'Duplicate module ID',
    );
  });

  it.each(['', 'NAVCOM', '../navcom', 'a', 'has space', 'a'.repeat(49)])(
    'rejects an invalid ID: %s',
    (id) => {
      expect(() => new ModuleRegistry([{ ...planned, id }])).toThrow(
        'Invalid module ID',
      );
    },
  );

  it.each([
    { ...planned, name: ' ' },
    { ...planned, summary: '' },
    { ...planned, reason: '' },
    { ...planned, availability: 'invented' },
    {
      id: 'test',
      name: 'Test',
      summary: 'Test',
      availability: 'available',
      entry: null,
    },
  ])(
    'rejects missing metadata, invalid states and missing entries: %j',
    (definition) => {
      expect(
        () => new ModuleRegistry([definition as ModuleDefinition<string>]),
      ).toThrow();
    },
  );
});
