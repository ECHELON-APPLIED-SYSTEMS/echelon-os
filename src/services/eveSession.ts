import { invoke, isTauri } from '@tauri-apps/api/core';

export const errors = [
  'unavailable',
  'unsupported',
  'busy',
  'port-occupied',
  'timeout',
  'authentication',
  'denied',
  'cancelled',
  'invalid-response',
  'disconnected',
  'expired',
  'forbidden',
  'rate-limited',
  'invalid-destination',
  'invalid-confirmation',
  'outcome-unknown',
] as const;
export type EveError = (typeof errors)[number];
export type Character = Readonly<{ id: number; name: string }>;
export type Location = Readonly<{
  system_id: number;
  fetched_at: number;
  server_as_of: number | null;
  fresh_until: number;
  cached: boolean;
  stale: boolean;
}>;
export type View = Readonly<{
  phase: 'disconnected' | 'connecting' | 'connected' | 'expired' | 'error';
  character: Character | null;
  location: Location | null;
  error: EveError | null;
  retry_at: number | null;
  expires_at: number | null;
}>;
export type Action = 'set-destination' | 'add-waypoint';
export type Confirmation = Readonly<{
  id: string;
  character: Character;
  destination: number;
  action: Action;
  expires_at: number;
}>;
export const disconnected: View = Object.freeze({
  phase: 'disconnected',
  character: null,
  location: null,
  error: null,
  retry_at: null,
  expires_at: null,
});
export const eveMessages: Record<EveError, string> = {
  unavailable: 'The EVE service is unavailable. Retry explicitly when ready.',
  unsupported: 'EVE sign-in requires the Windows desktop application.',
  busy: 'An EVE operation is already in progress.',
  'port-occupied':
    'Local callback port 43173 is occupied. Close the conflicting application before retrying.',
  timeout: 'Sign-in timed out. Connect again when ready.',
  authentication: 'EVE sign-in could not be verified. No session was retained.',
  denied: 'EVE sign-in was declined.',
  cancelled: 'EVE operation cancelled.',
  'invalid-response':
    'EVE returned an unsupported response. No success is assumed.',
  disconnected: 'Connect a character first.',
  expired: 'The EVE session expired. Connect again.',
  forbidden:
    'EVE refused this permission. The session was cleared; connect again.',
  'rate-limited':
    'EVE requests are temporarily limited. Wait until the displayed retry time.',
  'invalid-destination':
    'The destination is not in the pinned local route data. Load the matching reviewed snapshot.',
  'invalid-confirmation':
    'This confirmation expired or is no longer valid. Review a new request.',
  'outcome-unknown':
    'The request outcome is unknown. Check EVE before preparing another request; it will not be retried automatically.',
};
export function errorCode(value: unknown): EveError {
  return errors.includes(value as EveError)
    ? (value as EveError)
    : 'unavailable';
}
function check(ok: unknown): asserts ok {
  if (!ok) throw 'invalid-response';
}
function object(v: unknown, keys: string): Record<string, unknown> {
  check(v && typeof v === 'object' && !Array.isArray(v));
  check(Object.keys(v).sort().join(',') === keys.split(',').sort().join(','));
  return v as Record<string, unknown>;
}
function id(v: unknown): v is number {
  return Number.isSafeInteger(v) && Number(v) > 0;
}
function time(v: unknown): v is number {
  return (
    Number.isSafeInteger(v) && Number(v) >= 0 && Number(v) < 8_640_000_000_000
  );
}
function character(value: unknown): Character {
  const c = object(value, 'id,name');
  check(
    id(c.id) &&
      typeof c.name === 'string' &&
      c.name.trim() &&
      c.name.length <= 128,
  );
  return { id: c.id, name: c.name } as Character;
}
export function parseView(value: unknown): View {
  const v = object(value, 'phase,character,location,error,retry_at,expires_at');
  check(
    ['disconnected', 'connecting', 'connected', 'expired', 'error'].includes(
      String(v.phase),
    ),
  );
  check(v.error === null || errors.includes(v.error as EveError));
  check(v.retry_at === null || time(v.retry_at));
  check(v.expires_at === null || time(v.expires_at));
  const c = v.character === null ? null : character(v.character);
  let l: Location | null = null;
  if (v.location !== null) {
    const p = object(
      v.location,
      'system_id,fetched_at,server_as_of,fresh_until,cached,stale',
    );
    check(id(p.system_id) && time(p.fetched_at) && time(p.fresh_until));
    check(p.server_as_of === null || time(p.server_as_of));
    check(typeof p.cached === 'boolean' && typeof p.stale === 'boolean');
    l = p as Location;
  }
  check(
    v.phase === 'connected'
      ? c !== null && v.expires_at !== null
      : c === null && l === null && v.expires_at === null,
  );
  return { ...v, character: c, location: l } as View;
}
function parseConfirmation(value: unknown): Confirmation {
  const v = object(value, 'id,character,destination,action,expires_at');
  check(typeof v.id === 'string' && /^[A-Za-z0-9_-]{43}$/.test(v.id));
  check(id(v.destination) && time(v.expires_at));
  check(v.action === 'set-destination' || v.action === 'add-waypoint');
  return { ...v, character: character(v.character) } as Confirmation;
}
async function call(
  command: string,
  args?: Record<string, unknown>,
): Promise<unknown> {
  if (!isTauri()) throw 'unsupported';
  try {
    return await invoke(command, args);
  } catch (e) {
    throw errorCode(e);
  }
}
export const eveSession = {
  desktop: isTauri,
  status: async (): Promise<View> =>
    !isTauri() ? disconnected : parseView(await call('eve_status')),
  connect: async (): Promise<View> => parseView(await call('eve_connect')),
  disconnect: async (): Promise<View> =>
    parseView(await call('eve_disconnect')),
  refresh: async (): Promise<View> =>
    parseView(await call('eve_refresh_location')),
  prepare: async (
    destination: number,
    action: Action,
  ): Promise<Confirmation> => {
    check(
      id(destination) &&
        (action === 'set-destination' || action === 'add-waypoint'),
    );
    return parseConfirmation(
      await call('eve_prepare_waypoint', { destination, action }),
    );
  },
  send: async (confirmationId: string): Promise<View> => {
    check(/^[A-Za-z0-9_-]{43}$/.test(confirmationId));
    return parseView(await call('eve_send_waypoint', { confirmationId }));
  },
};
