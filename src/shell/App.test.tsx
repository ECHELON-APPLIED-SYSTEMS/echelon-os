import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { readAppMetadata } from '../services/appMetadata';
import { App } from './App';
import { readNavcomSnapshot } from '../services/navcomSnapshot';
import { ModuleView } from './ModuleView';

vi.mock('../services/navcomSnapshot', () => ({ readNavcomSnapshot: vi.fn() }));
vi.mock('../services/appMetadata', () => ({ readAppMetadata: vi.fn() }));
beforeEach(() => {
  vi.mocked(readAppMetadata).mockResolvedValue({ kind: 'preview' });
  vi.mocked(readNavcomSnapshot).mockResolvedValue({ kind: 'preview' });
});

describe('application shell', () => {
  it('opens NAVCOM without automatically reading private route data', async () => {
    render(<App />);
    expect(
      screen.getByRole('heading', { level: 1, name: 'Workspace overview' }),
    ).toBeVisible();
    expect(await screen.findByText(/Browser preview/)).toBeVisible();
    expect(screen.getByText('Ready to open')).toBeVisible();
    fireEvent.click(
      screen.getByRole('button', { name: /View module details/ }),
    );
    expect(
      screen.getByRole('heading', { level: 1, name: 'NAVCOM' }),
    ).toBeVisible();
    expect(readNavcomSnapshot).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole('button', { name: 'Load route data' }));
    expect(
      await screen.findByText(/Browser preview cannot access private desktop/),
    ).toBeVisible();
    expect(screen.getByRole('button', { name: /NAVCOM/ })).toHaveAttribute(
      'aria-current',
      'page',
    );
    fireEvent.click(screen.getByRole('button', { name: /Overview/ }));
    expect(
      screen.getByRole('heading', { level: 1, name: 'Workspace overview' }),
    ).toBeVisible();
  });

  it('renders metadata returned from the desktop boundary', async () => {
    vi.mocked(readAppMetadata).mockResolvedValue({
      kind: 'desktop',
      metadata: {
        name: 'ECHELON OS',
        version: '0.1.0',
        platform: 'windows',
        milestone: 'M1-B',
      },
    });
    render(<App />);
    expect(await screen.findByText('Desktop connected')).toBeVisible();
    expect(screen.getByText('windows')).toBeVisible();
    expect(screen.getByText('0.1.0')).toBeVisible();
    // Integrated metadata must coexist with the ephemeral M1-C selector.
    const appearance = screen.getByRole('combobox', { name: 'Appearance' });
    fireEvent.change(appearance, { target: { value: 'caldari' } });
    expect(document.documentElement).toHaveAttribute('data-theme', 'caldari');
    expect(screen.getByText('Desktop connected')).toBeVisible();
    expect(screen.getAllByText('M1-B').length).toBeGreaterThanOrEqual(2);
    expect(screen.getByText(/Workspace choices reset/)).toHaveTextContent(
      'desktop Appearance is saved',
    );
  });

  it('renders a pending state, then an accessible error and a working explicit retry', async () => {
    let rejectRead: (reason: Error) => void = () => {};
    vi.mocked(readAppMetadata).mockReturnValueOnce(
      new Promise((_, reject) => {
        rejectRead = reject;
      }),
    );
    render(<App />);
    expect(screen.getByRole('status')).toHaveTextContent(
      'Checking desktop connection',
    );
    rejectRead(new Error('private technical error'));
    expect(await screen.findByRole('alert')).toHaveTextContent(
      'Desktop status could not be read',
    );
    expect(
      screen.queryByText('private technical error'),
    ).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Retry connection' }));
    expect(await screen.findByText(/Browser preview/)).toBeVisible();
    await waitFor(() => expect(readAppMetadata).toHaveBeenCalledTimes(2));
  });

  it('hosts a registered available module entry', () => {
    render(
      <ModuleView
        module={{
          id: 'test',
          name: 'Test',
          summary: 'Test module',
          availability: 'available',
          entry: () => <h2>Module entry</h2>,
        }}
      />,
    );
    expect(screen.getByRole('heading', { name: 'Module entry' })).toBeVisible();
  });

  it('keeps a disabled module unavailable with its reason', () => {
    render(
      <ModuleView
        module={{
          id: 'test',
          name: 'Test',
          summary: 'Test module',
          availability: 'disabled',
          reason: 'Disabled for maintenance',
        }}
      />,
    );
    expect(screen.getByText('Unavailable')).toBeVisible();
    expect(screen.getByText('Disabled for maintenance')).toBeVisible();
  });
});
