import { createHash, webcrypto } from 'node:crypto';
import { fireEvent, render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { readNavcomSnapshot } from '../../services/navcomSnapshot';
import { loadStargateSnapshot } from './snapshot';
import { Navcom } from './Navcom';

vi.mock('../../services/navcomSnapshot', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../../services/navcomSnapshot')>()),
  readNavcomSnapshot: vi.fn(),
}));
vi.stubGlobal('crypto', webcrypto);
beforeEach(() => vi.mocked(readNavcomSnapshot).mockReset());

async function ready(extraSystems: [number, string | null][] = []) {
  const text = JSON.stringify({
    version: 1,
    systems: [
      [1, 'Fiction Alpha'],
      [2, 'Fiction Middle'],
      [3, 'Fiction Omega'],
      [4, null],
      ...extraSystems,
    ],
    edges: [
      [1, 2],
      [2, 3],
    ],
  });
  const snapshot = await loadStargateSnapshot(
    text,
    {
      version: 1,
      converterVersion: '1',
      build: 42,
      sourceUrl:
        'https://developers.eveonline.com/static-data/tranquility/eve-online-static-data-42-jsonl.zip',
      archiveSha256: 'a'.repeat(64),
      snapshotSha256: createHash('sha256').update(text).digest('hex'),
      tableSha256: {
        'mapSolarSystems.jsonl': 'b'.repeat(64),
        'mapStargates.jsonl': 'c'.repeat(64),
      },
      systems: 4 + extraSystems.length,
      gates: 4,
      edges: 2,
      isolatedSystems: 1 + extraSystems.length,
    },
    42,
  );
  return { kind: 'ready' as const, snapshot };
}
function select(label: string, id: number) {
  const input = screen.getByRole('combobox', { name: `${label} system` });
  fireEvent.focus(input);
  fireEvent.change(input, { target: { value: String(id) } });
  fireEvent.keyDown(input, { key: 'ArrowDown' });
  fireEvent.keyDown(input, { key: 'Enter' });
}

describe('NAVCOM explicit static route interface', () => {
  it('routes through the real validator and engine, clears stale routes and handles isolated/same-system endpoints', async () => {
    vi.mocked(readNavcomSnapshot).mockResolvedValue(await ready());
    render(<Navcom />);
    expect(readNavcomSnapshot).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole('button', { name: 'Load route data' }));
    expect(await screen.findByText(/Static CCP SDE build 42/)).toBeVisible();
    select('Origin', 1);
    select('Destination', 3);
    fireEvent.click(
      screen.getByRole('button', { name: 'Calculate minimum-hop route' }),
    );
    expect(screen.getByText('2 jumps · 3 systems')).toBeVisible();
    expect(
      screen.getAllByRole('listitem').map((row) => row.textContent),
    ).toEqual([
      'Fiction Alpha System ID 1',
      'Fiction Middle System ID 2',
      'Fiction Omega System ID 3',
    ]);
    select('Destination', 4);
    expect(screen.queryByText('2 jumps · 3 systems')).not.toBeInTheDocument();
    fireEvent.click(
      screen.getByRole('button', { name: 'Calculate minimum-hop route' }),
    );
    expect(screen.getByText(/No static stargate route connects/)).toBeVisible();
    select('Destination', 1);
    fireEvent.click(
      screen.getByRole('button', { name: 'Calculate minimum-hop route' }),
    );
    expect(
      screen.getByText(/Already in the destination system/),
    ).toHaveTextContent('0 jumps');
    expect(screen.getByText(/not a safest-route calculation/)).toBeVisible();
  });
  it('filters progressively with a bounded accessible popup and preserves duplicate IDs', async () => {
    vi.mocked(readNavcomSnapshot).mockResolvedValue(
      await ready(
        Array.from({ length: 25 }, (_, i): [number, string | null] => [
          i + 5,
          'Fiction Alpha',
        ]),
      ),
    );
    render(<Navcom />);
    fireEvent.click(screen.getByRole('button', { name: 'Load route data' }));
    await screen.findByText(/Static CCP SDE build 42/);
    const input = screen.getByRole('combobox', { name: 'Origin system' });
    expect(input).toHaveAttribute('maxlength', '64');
    fireEvent.focus(input);
    expect(input).toHaveAttribute('aria-expanded', 'false');
    fireEvent.change(input, { target: { value: 'Fiction' } });
    const list = screen.getByRole('listbox', { name: 'Origin matches' });
    expect(input).toHaveAttribute('aria-controls', list.id);
    expect(input).toHaveAttribute('aria-expanded', 'true');
    expect(screen.getAllByRole('option')).toHaveLength(20);
    fireEvent.change(input, { target: { value: 'Middle' } });
    expect(screen.getAllByRole('option')).toHaveLength(1);
    expect(screen.getByRole('option')).toHaveTextContent('Fiction Middle — 2');
    fireEvent.change(input, { target: { value: 'Alpha' } });
    const option = screen.getByRole('option', { name: 'Fiction Alpha — 5' });
    expect(fireEvent.pointerDown(option, { pointerType: 'touch' })).toBe(false);
    fireEvent.click(option);
    expect(input).toHaveValue('Fiction Alpha — 5');
    expect(input).toHaveFocus();
    expect(input).toHaveAttribute('aria-expanded', 'false');
    expect(screen.queryByRole('listbox')).not.toBeInTheDocument();
    expect(screen.getByText('Selected system ID 5.')).toBeVisible();
  });

  it('supports up/down, Enter and Escape without implicit selection or focus traps', async () => {
    vi.mocked(readNavcomSnapshot).mockResolvedValue(await ready());
    render(<Navcom />);
    fireEvent.click(screen.getByRole('button', { name: 'Load route data' }));
    await screen.findByText(/Static CCP SDE build 42/);
    const input = screen.getByRole('combobox', { name: 'Origin system' });
    fireEvent.focus(input);
    fireEvent.change(input, { target: { value: 'Fiction' } });
    fireEvent.keyDown(input, { key: 'Enter' });
    expect(input).not.toHaveAttribute('aria-activedescendant');
    expect(screen.queryByText(/Selected system ID/)).not.toBeInTheDocument();
    fireEvent.keyDown(input, { key: 'ArrowDown' });
    expect(
      document.getElementById(input.getAttribute('aria-activedescendant')!),
    ).toHaveTextContent('Fiction Alpha');
    fireEvent.keyDown(input, { key: 'ArrowDown' });
    expect(
      document.getElementById(input.getAttribute('aria-activedescendant')!),
    ).toHaveTextContent('Fiction Middle');
    fireEvent.keyDown(input, { key: 'ArrowUp' });
    expect(
      document.getElementById(input.getAttribute('aria-activedescendant')!),
    ).toHaveTextContent('Fiction Alpha');
    fireEvent.keyDown(input, { key: 'Escape' });
    expect(screen.queryByRole('listbox')).not.toBeInTheDocument();
    expect(input).not.toHaveAttribute('aria-activedescendant');
    expect(input).toHaveValue('Fiction');
    fireEvent.keyDown(input, { key: 'ArrowUp' });
    expect(
      document.getElementById(input.getAttribute('aria-activedescendant')!),
    ).toHaveTextContent('Fiction Omega');
    fireEvent.keyDown(input, { key: 'Enter' });
    expect(input).toHaveValue('Fiction Omega — 3');
    expect(input).toHaveFocus();
    fireEvent.focus(input);
    expect(
      screen.getByRole('option', { name: 'Fiction Omega — 3' }),
    ).toHaveAttribute('aria-selected', 'true');
    fireEvent.keyDown(input, { key: 'Escape' });
    expect(screen.getByText('Selected system ID 3.')).toBeVisible();
  });

  it('keeps endpoints independent and invalidates IDs/routes on text edits or clearing', async () => {
    vi.mocked(readNavcomSnapshot).mockResolvedValue(await ready());
    render(<Navcom />);
    fireEvent.click(screen.getByRole('button', { name: 'Load route data' }));
    await screen.findByText(/Static CCP SDE build 42/);
    select('Origin', 1);
    select('Destination', 3);
    const calculate = screen.getByRole('button', {
      name: 'Calculate minimum-hop route',
    });
    fireEvent.click(calculate);
    expect(screen.getByText('2 jumps · 3 systems')).toBeVisible();
    const origin = screen.getByRole('combobox', { name: 'Origin system' });
    const destination = screen.getByRole('combobox', {
      name: 'Destination system',
    });
    fireEvent.focus(origin);
    fireEvent.keyDown(origin, { key: 'Escape' });
    expect(calculate).not.toBeDisabled();
    expect(screen.getByText('2 jumps · 3 systems')).toBeVisible();
    fireEvent.change(origin, { target: { value: 'Middle' } });
    expect(calculate).toBeDisabled();
    expect(screen.queryByText('2 jumps · 3 systems')).not.toBeInTheDocument();
    expect(destination).toHaveValue('Fiction Omega — 3');
    fireEvent.mouseDown(
      screen.getByRole('option', { name: 'Fiction Middle — 2' }),
    );
    fireEvent.click(screen.getByRole('option', { name: 'Fiction Middle — 2' }));
    fireEvent.click(calculate);
    expect(screen.getByText('1 jumps · 2 systems')).toBeVisible();
    fireEvent.change(destination, { target: { value: '' } });
    expect(calculate).toBeDisabled();
    expect(screen.queryByText('1 jumps · 2 systems')).not.toBeInTheDocument();
    fireEvent.change(destination, {
      target: { value: 'No such fictional system' },
    });
    expect(
      screen.getByText('No matching systems in this snapshot.'),
    ).toBeVisible();
    fireEvent.keyDown(destination, { key: 'ArrowDown' });
    fireEvent.keyDown(destination, { key: 'Enter' });
    expect(calculate).toBeDisabled();
    expect(destination).not.toHaveAttribute('aria-activedescendant');
    fireEvent.blur(destination);
    expect(screen.queryByRole('listbox')).not.toBeInTheDocument();
    select('Destination', 4);
    expect(destination).toHaveValue('4');
    expect(screen.getByText('Selected system ID 4.')).toBeVisible();
  });

  it.each(['preview', 'missing'] as const)(
    'keeps %s honest with no selectable or fictional data',
    async (kind) => {
      vi.mocked(readNavcomSnapshot).mockResolvedValue({ kind });
      render(<Navcom />);
      fireEvent.click(screen.getByRole('button', { name: 'Load route data' }));
      expect(
        await screen.findByRole('button', { name: 'Retry loading' }),
      ).toBeVisible();
      expect(screen.queryByRole('combobox')).not.toBeInTheDocument();
      expect(
        screen.getByText(
          kind === 'preview'
            ? /Browser preview cannot access/
            : /Route data is not provisioned/,
        ),
      ).toBeVisible();
    },
  );
  it('shows loading, safe errors, retry and clears verified data while reloading', async () => {
    let resolve!: (
      result: Awaited<ReturnType<typeof readNavcomSnapshot>>,
    ) => void;
    vi.mocked(readNavcomSnapshot).mockReturnValueOnce(
      new Promise((r) => {
        resolve = r;
      }),
    );
    render(<Navcom />);
    fireEvent.click(screen.getByRole('button', { name: 'Load route data' }));
    expect(
      screen.getByRole('button', { name: 'Loading route data…' }),
    ).toBeDisabled();
    resolve({ kind: 'error', code: 'invalid-snapshot' });
    expect(await screen.findByRole('alert')).toHaveTextContent(
      'failed integrity',
    );
    vi.mocked(readNavcomSnapshot).mockResolvedValueOnce(await ready());
    fireEvent.click(screen.getByRole('button', { name: 'Retry loading' }));
    expect(await screen.findByText(/Static CCP SDE build 42/)).toBeVisible();
    vi.mocked(readNavcomSnapshot).mockResolvedValueOnce({ kind: 'missing' });
    fireEvent.click(screen.getByRole('button', { name: 'Reload route data' }));
    expect(screen.queryByRole('combobox')).not.toBeInTheDocument();
    expect(
      await screen.findByText(/Route data is not provisioned/),
    ).toBeVisible();
  });
});
