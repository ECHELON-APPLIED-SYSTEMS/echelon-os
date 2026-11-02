import { invoke, isTauri } from '@tauri-apps/api/core';
import type { Character, EveError } from './eveSession';
import { errorCode, errors } from './eveSession';
export const MAX_ROUTE_POSTS = 10;
export type RouteConfirmation = Readonly<{
  id: string;
  character: Character;
  systems: readonly number[];
  expires_at: number;
}>;
export type RouteReport = Readonly<{
  delivery: 'all-accepted' | 'stopped' | 'unknown';
  total: number;
  accepted_count: number;
  last_accepted: number | null;
  failed_system: number | null;
  next_unsent: number | null;
  reason: EveError | null;
}>;
function check(v: unknown): asserts v {
  if (!v) throw 'invalid-response';
}
function object(v: unknown, keys: string): Record<string, unknown> {
  check(v && typeof v === 'object' && !Array.isArray(v));
  check(Object.keys(v).sort().join(',') === keys.split(',').sort().join(','));
  return v as Record<string, unknown>;
}
function id(v: unknown): v is number {
  return Number.isSafeInteger(v) && Number(v) > 0;
}
async function call(
  command: string,
  args: Record<string, unknown>,
): Promise<unknown> {
  if (!isTauri()) throw 'unsupported';
  try {
    return await invoke(command, args);
  } catch (e) {
    throw errorCode(e);
  }
}
export async function prepareRoute(
  systems: readonly number[],
): Promise<RouteConfirmation> {
  check(
    systems.length >= 2 &&
      systems.length <= MAX_ROUTE_POSTS + 1 &&
      systems.every(id),
  );
  const v = object(
    await call('eve_prepare_route', { systems }),
    'id,character,systems,expires_at',
  );
  check(typeof v.id === 'string' && /^[A-Za-z0-9_-]{43}$/.test(v.id));
  const c = object(v.character, 'id,name');
  check(
    id(c.id) &&
      typeof c.name === 'string' &&
      c.name.length > 0 &&
      c.name.length <= 128,
  );
  check(
    Array.isArray(v.systems) &&
      JSON.stringify(v.systems) === JSON.stringify(systems) &&
      id(v.expires_at),
  );
  return v as RouteConfirmation;
}
export async function sendRoute(confirmationId: string): Promise<RouteReport> {
  check(/^[A-Za-z0-9_-]{43}$/.test(confirmationId));
  const v = object(
    await call('eve_send_route', { confirmationId }),
    'delivery,total,accepted_count,last_accepted,failed_system,next_unsent,reason',
  );
  check(
    ['all-accepted', 'stopped', 'unknown'].includes(String(v.delivery)) &&
      id(v.total) &&
      v.total <= MAX_ROUTE_POSTS &&
      Number.isSafeInteger(v.accepted_count) &&
      Number(v.accepted_count) >= 0 &&
      Number(v.accepted_count) <= v.total,
  );
  for (const key of ['last_accepted', 'failed_system', 'next_unsent'])
    check(v[key] === null || id(v[key]));
  check(v.reason === null || errors.includes(v.reason as EveError));
  check(
    v.delivery !== 'all-accepted' ||
      (v.accepted_count === v.total &&
        v.failed_system === null &&
        v.next_unsent === null &&
        v.reason === null),
  );
  check(
    Number(v.accepted_count) === 0
      ? v.last_accepted === null
      : v.last_accepted !== null,
  );
  return v as RouteReport;
}
export async function cancelRoute(confirmationId: string): Promise<void> {
  check(/^[A-Za-z0-9_-]{43}$/.test(confirmationId));
  await call('eve_cancel_route', { confirmationId });
}
