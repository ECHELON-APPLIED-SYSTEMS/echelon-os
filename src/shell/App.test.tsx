import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { readAppMetadata } from '../services/appMetadata';
import { App } from './App';
import { ModuleView } from './ModuleView';

vi.mock('../services/appMetadata', () => ({ readAppMetadata: vi.fn() }));
beforeEach(() => {
  vi.mocked(readAppMetadata).mockResolvedValue({ kind: 'preview' });
});

describe('application shell', () => {
  it('shows an honest browser state and planned NAVCOM details', async () => {
    render(<App />);
    expect(
      screen.getByRole('heading', { level: 1, name: 'Workspace overview' }),
    ).toBeVisible();
    expect(await screen.findByText(/Browser preview/)).toBeVisible();
    expect(screen.getByText('Not yet implemented')).toBeVisible();
    fireEvent.click(
      screen.getByRole('button', { name: /View module details/ }),
    );
    expect(
      screen.getByRole('heading', { level: 1, name: 'NAVCOM' }),
    ).toBeVisible();
    expect(
      screen.getByText(/Route planning and EVE connectivity are unavailable/),
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
        milestone: 'M1-A',
      },
    });
    render(<App />);
    expect(await screen.findByText('Desktop connected')).toBeVisible();
    expect(screen.getByText('windows')).toBeVisible();
    expect(screen.getByText('0.1.0')).toBeVisible();
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
