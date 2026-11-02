import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
} from '@testing-library/react';
import { beforeEach, expect, it, vi } from 'vitest';
import {
  canInstallRoutes,
  installRouteDatabase,
  routeInstallStatus,
} from '../../services/routeInstall';
import { readNavcomSnapshot } from '../../services/navcomSnapshot';
import { RouteDataManager } from './RouteDataManager';
import { Navcom } from './Navcom';
import { createStargateGraph } from './routing';
vi.mock('../../services/routeInstall', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../../services/routeInstall')>()),
  canInstallRoutes: vi.fn(),
  installRouteDatabase: vi.fn(),
  routeInstallStatus: vi.fn(),
}));
vi.mock('../../services/navcomSnapshot', () => ({
  NAVCOM_PIN: { build: 3662168 },
  readNavcomSnapshot: vi.fn(),
}));
beforeEach(() => {
  vi.mocked(canInstallRoutes).mockReturnValue(true);
  vi.mocked(installRouteDatabase).mockReset();
  vi.mocked(routeInstallStatus).mockReset().mockResolvedValue('downloading');
});
it('never starts downloading or polling on mount and shows source/rights/build', () => {
  render(<RouteDataManager onInstalled={vi.fn()} />);
  expect(installRouteDatabase).not.toHaveBeenCalled();
  expect(routeInstallStatus).not.toHaveBeenCalled();
  expect(screen.getByText(/approved build 3662168/)).toBeVisible();
  expect(
    screen.getByRole('link', { name: 'CCP Developer License Agreement' }),
  ).toHaveAttribute(
    'href',
    'https://developers.eveonline.com/license-agreement',
  );
});
it('guards double click, shows bounded operation and reloads only on success', async () => {
  let resolve!: (v: 'installed') => void;
  vi.mocked(installRouteDatabase).mockReturnValue(
    new Promise((r) => {
      resolve = r;
    }),
  );
  const reload = vi.fn().mockResolvedValue(undefined);
  render(<RouteDataManager onInstalled={reload} />);
  const button = screen.getByRole('button', { name: 'Download Route Data' });
  fireEvent.click(button);
  fireEvent.click(button);
  expect(button).toBeDisabled();
  expect(installRouteDatabase).toHaveBeenCalledTimes(1);
  expect(reload).not.toHaveBeenCalled();
  expect(screen.getByRole('status')).toHaveTextContent('120-second');
  await act(async () => resolve('installed'));
  await waitFor(() => expect(reload).toHaveBeenCalledTimes(1));
});
it('shows a safe failure, offers explicit retry and does not reload or auto retry', async () => {
  vi.mocked(installRouteDatabase)
    .mockRejectedValueOnce('integrity')
    .mockResolvedValueOnce('already-installed');
  const reload = vi.fn().mockResolvedValue(undefined);
  render(<RouteDataManager onInstalled={reload} />);
  fireEvent.click(screen.getByRole('button', { name: 'Download Route Data' }));
  await screen.findByText(/did not match the reviewed hashes/);
  expect(reload).not.toHaveBeenCalled();
  expect(installRouteDatabase).toHaveBeenCalledTimes(1);
  fireEvent.click(screen.getByRole('button', { name: 'Download Route Data' }));
  await waitFor(() => expect(reload).toHaveBeenCalledOnce());
});
it('reports native phases honestly through local progress IPC', async () => {
  vi.useFakeTimers();
  try {
    let resolve!: (v: 'installed') => void;
    vi.mocked(installRouteDatabase).mockReturnValue(
      new Promise((r) => {
        resolve = r;
      }),
    );
    render(
      <RouteDataManager onInstalled={vi.fn().mockResolvedValue(undefined)} />,
    );
    fireEvent.click(
      screen.getByRole('button', { name: 'Download Route Data' }),
    );
    await act(async () => vi.advanceTimersByTimeAsync(400));
    expect(screen.getByRole('status')).toHaveTextContent(
      'Downloading the pinned official',
    );
    vi.mocked(routeInstallStatus).mockResolvedValue('verifying');
    await act(async () => vi.advanceTimersByTimeAsync(400));
    expect(screen.getByRole('status')).toHaveTextContent('Verifying hashes');
    vi.mocked(routeInstallStatus).mockResolvedValue('installing');
    await act(async () => vi.advanceTimersByTimeAsync(400));
    expect(screen.getByRole('status')).toHaveTextContent(
      'Publishing the validated',
    );
    await act(async () => resolve('installed'));
  } finally {
    vi.useRealTimers();
  }
});
it('native completion immediately invokes the existing snapshot loader and reveals planner', async () => {
  vi.mocked(readNavcomSnapshot)
    .mockReset()
    .mockResolvedValueOnce({ kind: 'missing' })
    .mockResolvedValueOnce({
      kind: 'ready',
      snapshot: {
        graph: createStargateGraph([1], []),
        build: 42,
        systemCount: 1,
        hasSystem: (id) => id === 1,
        englishName: () => 'Fiction',
        searchSystems: () => [{ id: 1, name: 'Fiction' }],
      },
    });
  vi.mocked(installRouteDatabase).mockResolvedValue('installed');
  render(<Navcom />);
  expect(readNavcomSnapshot).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole('button', { name: 'Load route data' }));
  await screen.findByRole('button', { name: 'Download Route Data' });
  fireEvent.click(screen.getByRole('button', { name: 'Download Route Data' }));
  await screen.findByRole('combobox', { name: 'Origin system' });
  expect(readNavcomSnapshot).toHaveBeenCalledTimes(2);
  expect(screen.getByText(/Static CCP SDE build 42/)).toBeVisible();
  expect(
    screen.queryByRole('button', { name: 'Download Route Data' }),
  ).not.toBeInTheDocument();
});
it('browser preview disables the install control', () => {
  vi.mocked(canInstallRoutes).mockReturnValue(false);
  render(<RouteDataManager onInstalled={vi.fn()} />);
  expect(
    screen.getByRole('button', { name: 'Download Route Data' }),
  ).toBeDisabled();
});
