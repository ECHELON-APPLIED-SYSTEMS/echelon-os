import { createHash, webcrypto } from 'node:crypto';
import { fireEvent, render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { readNavcomSnapshot } from '../../services/navcomSnapshot';
import { loadStargateSnapshot } from './snapshot';
import { Navcom } from './Navcom';

vi.mock('../../services/navcomSnapshot', () => ({
  readNavcomSnapshot: vi.fn(),
}));
vi.stubGlobal('crypto', webcrypto);
beforeEach(() => vi.mocked(readNavcomSnapshot).mockReset());

async function ready() {
  const text = JSON.stringify({
    version: 1,
    systems: [
      [1, 'Fiction Alpha'],
      [2, 'Fiction Middle'],
      [3, 'Fiction Omega'],
      [4, null],
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
      systems: 4,
      gates: 4,
      edges: 2,
      isolatedSystems: 1,
    },
    42,
  );
  return { kind: 'ready' as const, snapshot };
}
function select(label: string, id: number) {
  fireEvent.change(
    screen.getByRole('searchbox', {
      name: `Search ${label.toLowerCase()} by name or ID`,
    }),
    { target: { value: String(id) } },
  );
  fireEvent.change(screen.getByRole('combobox', { name: `${label} system` }), {
    target: { value: String(id) },
  });
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
