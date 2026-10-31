/** Trusted, statically imported modules only. TEntry is supplied by the UI host. */
export type ModuleDefinition<TEntry> = Readonly<{
  id: string;
  name: string;
  summary: string;
}> &
  (
    | Readonly<{ availability: 'available'; entry: TEntry }>
    | Readonly<{
        availability: 'planned' | 'disabled';
        reason: string;
      }>
  );

/** An immutable registry assembled once at startup; no discovery or code loading. */
export class ModuleRegistry<TEntry> {
  readonly #modules = new Map<string, ModuleDefinition<TEntry>>();

  constructor(definitions: readonly ModuleDefinition<TEntry>[]) {
    for (const definition of definitions) {
      if (!/^[a-z][a-z0-9-]{1,47}$/.test(definition.id)) {
        throw new Error(`Invalid module ID: ${definition.id}`);
      }
      if (!definition.name.trim() || !definition.summary.trim()) {
        throw new Error(`Missing module metadata: ${definition.id}`);
      }
      if (
        definition.availability !== 'available' &&
        definition.availability !== 'planned' &&
        definition.availability !== 'disabled'
      ) {
        throw new Error(`Invalid module availability: ${definition.id}`);
      }
      if (
        definition.availability === 'available'
          ? definition.entry == null
          : !definition.reason.trim()
      ) {
        throw new Error(
          `Missing module entry or availability reason: ${definition.id}`,
        );
      }
      if (this.#modules.has(definition.id)) {
        throw new Error(`Duplicate module ID: ${definition.id}`);
      }
      this.#modules.set(definition.id, Object.freeze({ ...definition }));
    }
  }

  get(id: string): ModuleDefinition<TEntry> | undefined {
    return this.#modules.get(id);
  }

  list(): readonly ModuleDefinition<TEntry>[] {
    return Object.freeze([...this.#modules.values()]);
  }
}
