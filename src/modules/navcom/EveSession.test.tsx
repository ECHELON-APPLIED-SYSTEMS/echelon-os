import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
} from '@testing-library/react';
import { beforeEach, expect, it, vi } from 'vitest';
import { invoke, isTauri } from '@tauri-apps/api/core';
import { disconnected } from '../../services/eveSession';
import type { View } from '../../services/eveSession';
import { readNavcomSnapshot } from '../../services/navcomSnapshot';
import { Navcom } from './Navcom';
import { createStargateGraph } from './routing';
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn(), isTauri: vi.fn() }));
vi.mock('../../services/navcomSnapshot', () => ({
  readNavcomSnapshot: vi.fn(),
}));
const now = () => Math.floor(Date.now() / 1000);
const connected = (): View => ({
  ...disconnected,
  phase: 'connected',
  character: { id: 42, name: 'Fiction Pilot' },
  expires_at: now() + 600,
});
let current: View;
const calls = () => vi.mocked(invoke).mock.calls.map((c) => c[0]);
function select(label: string, id: number) {
  const input = screen.getByRole('combobox', { name: `${label} system` });
  fireEvent.change(input, { target: { value: String(id) } });
  fireEvent.keyDown(input, { key: 'ArrowDown' });
  fireEvent.keyDown(input, { key: 'Enter' });
}
beforeEach(() => {
  current = connected();
  vi.mocked(isTauri).mockReturnValue(true);
  vi.mocked(invoke)
    .mockReset()
    .mockImplementation(async (command, args) => {
      if (command === 'eve_status') return current;
      if (command === 'eve_refresh_location') {
        current = {
          ...current,
          location: {
            system_id: 1,
            fetched_at: now(),
            server_as_of: now() - 3,
            fresh_until: now() + 5,
            cached: true,
            stale: false,
          },
        };
        return current;
      }
      if (command === 'eve_disconnect') {
        current = disconnected;
        return current;
      }
      if (command === 'eve_prepare_waypoint')
        return {
          id: 'a'.repeat(43),
          character: current.character,
          destination:
            args && 'destination' in args ? args.destination : undefined,
          action: args && 'action' in args ? args.action : undefined,
          expires_at: now() + 30,
        };
      if (command === 'eve_send_waypoint') return current;
      throw 'unavailable';
    });
  vi.mocked(readNavcomSnapshot).mockResolvedValue({
    kind: 'ready',
    snapshot: {
      graph: createStargateGraph([1, 2], [[1, 2]]),
      build: 42,
      systemCount: 2,
      hasSystem: (id) => id === 1 || id === 2,
      englishName: (id) => (id === 1 ? 'Fiction Alpha' : 'Fiction Omega'),
      searchSystems: (query) =>
        [1, 2]
          .filter((id) => String(id).startsWith(query))
          .map((id) => ({
            id,
            name: id === 1 ? 'Fiction Alpha' : 'Fiction Omega',
          })),
    },
  });
});
async function planner() {
  render(<Navcom />);
  await screen.findByText(/Connected: Fiction Pilot/);
  fireEvent.click(screen.getByRole('button', { name: 'Load route data' }));
  await screen.findByText(/Static CCP SDE build 42/);
}
it('reads status locally, requests location explicitly, validates origin and preserves offline routing', async () => {
  await planner();
  expect(calls()).toEqual(['eve_status']);
  select('Destination', 2);
  fireEvent.click(
    screen.getByRole('button', { name: 'Refresh current location' }),
  );
  await screen.findByText(/reused cached observation/);
  expect(screen.getByText(/server data as of/)).toHaveTextContent(
    'not instantaneous',
  );
  fireEvent.click(
    screen.getByRole('button', { name: 'Use Current System as origin' }),
  );
  expect(screen.getByRole('combobox', { name: 'Origin system' })).toHaveValue(
    'Fiction Alpha — 1',
  );
  expect(
    screen.getByRole('combobox', { name: 'Destination system' }),
  ).toHaveValue('Fiction Omega — 2');
  fireEvent.click(
    screen.getByRole('button', { name: 'Calculate minimum-hop route' }),
  );
  expect(screen.getByText('1 jumps · 2 systems')).toBeVisible();
  expect(calls()).toEqual(['eve_status', 'eve_refresh_location']);
  fireEvent.click(
    screen.getByRole('button', { name: 'Disconnect EVE character' }),
  );
  await screen.findByText('No connected character.');
  expect(screen.getByText('1 jumps · 2 systems')).toBeVisible();
  expect(
    screen.getByRole('button', { name: 'Set Destination…' }),
  ).toBeDisabled();
});
it.each([false, true])(
  'rejects unknown graph locations / stale observations (stale=%s)',
  async (stale) => {
    current = {
      ...current,
      location: {
        system_id: stale ? 1 : 999,
        fetched_at: now() - 50,
        server_as_of: null,
        fresh_until: stale ? now() - 5 : now() + 5,
        cached: true,
        stale,
      },
    };
    await planner();
    expect(
      screen.getByRole('button', { name: 'Use Current System as origin' }),
    ).toBeDisabled();
    expect(screen.getByRole('combobox', { name: 'Origin system' })).toHaveValue(
      '',
    );
  },
);
it.each(['Set Destination…', 'Add Waypoint…'])(
  'requires exact character/system confirmation for %s and sends once',
  async (action) => {
    await planner();
    select('Destination', 2);
    fireEvent.click(screen.getByRole('button', { name: action }));
    const dialog = await screen.findByRole('alertdialog');
    expect(dialog).toHaveTextContent(
      'Fiction Pilot (Character ID 42): Fiction Omega (System ID 2)',
    );
    expect(dialog).toHaveTextContent(
      action.startsWith('Set')
        ? 'clears all existing waypoints'
        : 'preserves existing waypoints',
    );
    expect(calls()).not.toContain('eve_send_waypoint');
    const confirm = screen.getByRole('button', { name: 'Confirm EVE request' });
    fireEvent.click(confirm);
    fireEvent.click(confirm);
    await screen.findByText(/Request accepted by ESI/);
    expect(calls().filter((c) => c === 'eve_send_waypoint')).toHaveLength(1);
    expect(screen.queryByRole('alertdialog')).not.toBeInTheDocument();
  },
);
it('cancel and destination edits invalidate displayed confirmations without a write', async () => {
  await planner();
  select('Destination', 2);
  fireEvent.click(screen.getByRole('button', { name: 'Set Destination…' }));
  await screen.findByRole('alertdialog');
  fireEvent.click(screen.getByRole('button', { name: 'Cancel request' }));
  expect(screen.queryByRole('alertdialog')).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole('button', { name: 'Add Waypoint…' }));
  await screen.findByRole('alertdialog');
  select('Destination', 1);
  expect(screen.queryByRole('alertdialog')).not.toBeInTheDocument();
  expect(calls()).not.toContain('eve_send_waypoint');
});
it('does not claim success or retry an ambiguous write', async () => {
  const base = vi.mocked(invoke).getMockImplementation()!;
  vi.mocked(invoke).mockImplementation((command, args) =>
    command === 'eve_send_waypoint'
      ? Promise.reject('outcome-unknown')
      : base(command, args),
  );
  await planner();
  select('Destination', 2);
  fireEvent.click(screen.getByRole('button', { name: 'Set Destination…' }));
  await screen.findByRole('alertdialog');
  fireEvent.click(screen.getByRole('button', { name: 'Confirm EVE request' }));
  await screen.findAllByText(/outcome is unknown/);
  expect(screen.queryByText(/Request accepted by ESI/)).not.toBeInTheDocument();
  expect(calls().filter((c) => c === 'eve_send_waypoint')).toHaveLength(1);
});
it('cancels a pending sign-in and ignores its late result', async () => {
  current = disconnected;
  let complete!: (v: View) => void;
  const base = vi.mocked(invoke).getMockImplementation()!;
  vi.mocked(invoke).mockImplementation((command, args) =>
    command === 'eve_connect'
      ? new Promise((resolve) => {
          complete = resolve;
        })
      : base(command, args),
  );
  render(<Navcom />);
  await waitFor(() => expect(calls()).toContain('eve_status'));
  fireEvent.click(
    screen.getByRole('button', { name: 'Connect EVE character' }),
  );
  await screen.findByText(/Waiting for EVE sign-in/);
  fireEvent.click(screen.getByRole('button', { name: 'Cancel sign-in' }));
  await screen.findByText('No connected character.');
  await act(async () => complete(connected()));
  expect(screen.getByText('No connected character.')).toBeVisible();
  expect(calls()).not.toContain('eve_refresh_location');
});
it('shows native rate limits without polling or automatic retry', async () => {
  const base = vi.mocked(invoke).getMockImplementation()!;
  vi.mocked(invoke).mockImplementation((command, args) => {
    if (command === 'eve_refresh_location') {
      current = { ...current, retry_at: now() + 60, error: 'rate-limited' };
      return Promise.reject('rate-limited');
    }
    return base(command, args);
  });
  await planner();
  fireEvent.click(
    screen.getByRole('button', { name: 'Refresh current location' }),
  );
  await screen.findByText(/ESI retry permitted after/);
  expect(
    screen.getByRole('button', { name: 'Refresh current location' }),
  ).toBeDisabled();
  expect(calls().filter((c) => c === 'eve_refresh_location')).toHaveLength(1);
});

it('refuses a confirmation for a different character before any write', async () => {
  const base = vi.mocked(invoke).getMockImplementation()!;
  vi.mocked(invoke).mockImplementation((command, args) =>
    command === 'eve_prepare_waypoint'
      ? Promise.resolve({
          id: 'a'.repeat(43),
          character: { id: 43, name: 'Other Fiction Pilot' },
          destination: 2,
          action: 'set-destination',
          expires_at: now() + 30,
        })
      : base(command, args),
  );
  await planner();
  select('Destination', 2);
  fireEvent.click(screen.getByRole('button', { name: 'Set Destination…' }));
  await screen.findByText(/confirmation expired or is no longer valid/);
  expect(screen.queryByRole('alertdialog')).not.toBeInTheDocument();
  expect(calls()).not.toContain('eve_send_waypoint');
});
