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
import { RouteTransfer } from './RouteTransfer';
import { useEveSession } from './useEveSession';
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn(), isTauri: vi.fn() }));
let system = 1;
let stale = false;
const view = () => ({
  ...disconnected,
  phase: 'connected',
  character: { id: 42, name: 'Fiction Pilot' },
  expires_at: Math.floor(Date.now() / 1000) + 600,
  location: {
    system_id: system,
    fetched_at: Math.floor(Date.now() / 1000),
    server_as_of: null,
    fresh_until: Math.floor(Date.now() / 1000) + 60,
    cached: false,
    stale,
  },
});
function Host({ systems = [1, 2, 3] }: { systems?: number[] }) {
  const session = useEveSession();
  return (
    <RouteTransfer
      key={systems.join(',')}
      systems={systems}
      session={session}
      name={(id) => `Fiction ${id}`}
    />
  );
}
const calls = () => vi.mocked(invoke).mock.calls.map((c) => c[0]);
beforeEach(() => {
  system = 1;
  stale = false;
  vi.mocked(isTauri).mockReturnValue(true);
  vi.mocked(invoke)
    .mockReset()
    .mockImplementation(async (command, args) => {
      if (command === 'eve_status') return view();
      if (command === 'eve_prepare_route')
        return {
          id: 'a'.repeat(43),
          character: { id: 42, name: 'Fiction Pilot' },
          systems: args && 'systems' in args ? args.systems : [],
          expires_at: Math.floor(Date.now() / 1000) + 30,
        };
      if (command === 'eve_send_route')
        return {
          delivery: 'all-accepted',
          total: 2,
          accepted_count: 2,
          last_accepted: 3,
          failed_system: null,
          next_unsent: null,
          reason: null,
        };
      if (command === 'eve_cancel_route') return;
      throw 'unavailable';
    });
});
it('does not prepare or write on mount/rerender; shows exact ordered native confirmation before a single send', async () => {
  const rendered = render(<Host />);
  const button = screen.getByRole('button', { name: 'Set Route in EVE…' });
  await waitFor(() => expect(button).not.toBeDisabled());
  rendered.rerender(<Host />);
  expect(calls()).toEqual(['eve_status']);
  fireEvent.click(button);
  const dialog = await screen.findByRole('alertdialog');
  expect(dialog).toHaveTextContent('Character Fiction Pilot (ID 42)');
  expect(dialog).toHaveTextContent('2 jumps · 2 ESI requests');
  expect(dialog).toHaveTextContent('clears existing EVE waypoints');
  expect(screen.getAllByRole('listitem').map((x) => x.textContent)).toEqual([
    'Fiction 2 · System ID 2',
    'Fiction 3 · System ID 3',
  ]);
  expect(calls()).not.toContain('eve_send_route');
  const confirm = screen.getByRole('button', {
    name: 'Confirm route transfer',
  });
  fireEvent.click(confirm);
  fireEvent.click(confirm);
  await screen.findByText(/2 of 2 requests accepted/);
  expect(calls().filter((c) => c === 'eve_send_route')).toHaveLength(1);
  expect(screen.getByText(/client application is not verified/)).toBeVisible();
});
it('cancels without a waypoint request and invalidates preview on route changes', async () => {
  const r = render(<Host />);
  await waitFor(() =>
    expect(
      screen.getByRole('button', { name: 'Set Route in EVE…' }),
    ).not.toBeDisabled(),
  );
  fireEvent.click(screen.getByRole('button', { name: 'Set Route in EVE…' }));
  await screen.findByRole('alertdialog');
  fireEvent.click(
    screen.getByRole('button', { name: 'Cancel route transfer' }),
  );
  await waitFor(() => expect(calls()).toContain('eve_cancel_route'));
  expect(calls()).not.toContain('eve_send_route');
  fireEvent.click(screen.getByRole('button', { name: 'Set Route in EVE…' }));
  await screen.findByRole('alertdialog');
  r.rerender(<Host systems={[1, 2]} />);
  expect(screen.queryByRole('alertdialog')).not.toBeInTheDocument();
  expect(calls()).not.toContain('eve_send_route');
});
it.each(['mismatch', 'stale', 'cap'])(
  'disables %s without silently truncating or writing',
  async (kind) => {
    system = kind === 'mismatch' ? 9 : 1;
    stale = kind === 'stale';
    render(
      <Host
        systems={
          kind === 'cap'
            ? Array.from({ length: 12 }, (_, i) => i + 1)
            : [1, 2, 3]
        }
      />,
    );
    await act(async () => {});
    expect(
      screen.getByRole('button', { name: 'Set Route in EVE…' }),
    ).toBeDisabled();
    expect(calls()).not.toContain('eve_prepare_route');
    expect(calls()).not.toContain('eve_send_route');
  },
);
it('shows partial/unknown accounting and never resumes accepted prefix', async () => {
  const base = vi.mocked(invoke).getMockImplementation()!;
  vi.mocked(invoke).mockImplementation((command, args) =>
    command === 'eve_send_route'
      ? Promise.resolve({
          delivery: 'unknown',
          total: 2,
          accepted_count: 1,
          last_accepted: 2,
          failed_system: 3,
          next_unsent: null,
          reason: 'outcome-unknown',
        })
      : base(command, args),
  );
  render(<Host />);
  await waitFor(() =>
    expect(
      screen.getByRole('button', { name: 'Set Route in EVE…' }),
    ).not.toBeDisabled(),
  );
  fireEvent.click(screen.getByRole('button', { name: 'Set Route in EVE…' }));
  await screen.findByRole('alertdialog');
  fireEvent.click(
    screen.getByRole('button', { name: 'Confirm route transfer' }),
  );
  await screen.findByText(/1 of 2 requests accepted/);
  expect(screen.getByText(/Partial\/unknown outcome/)).toHaveTextContent(
    'Attempted failed/uncertain system: 3',
  );
  expect(calls().filter((c) => c === 'eve_send_route')).toHaveLength(1);
});
