import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { invoke, isTauri } from '@tauri-apps/api/core';
import { App } from '../shell/App';
import { themes } from './themes';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn(), isTauri: vi.fn() }));
beforeEach(() => {
  vi.mocked(isTauri).mockReturnValue(false);
});
afterEach(() => {
  cleanup();
  document.documentElement.removeAttribute('data-theme');
  vi.restoreAllMocks();
});

function selector() {
  return screen.getByRole<HTMLSelectElement>('combobox', {
    name: 'Appearance',
  });
}

describe('live appearance', () => {
  it('offers five named native options with keyboard focus and preview guidance', async () => {
    render(<App />);
    await screen.findByText(/Browser preview/);
    const select = selector();
    expect(
      screen.getAllByRole('option').map((option) => option.textContent),
    ).toEqual(themes.map(({ name }) => name));
    expect(select).toHaveValue('echelon');
    expect(select).toHaveAccessibleDescription(
      'Browser-only preview · changes are not saved',
    );
    expect(select.closest('.sidebar-footer')).toBeNull();
    select.focus();
    expect(select).toHaveFocus();
    // Do not intercept the native select's keyboard handling. JSDOM does not emulate its popup.
    for (const key of ['ArrowDown', 'Enter', ' ', 'Tab']) {
      expect(fireEvent.keyDown(select, { key })).toBe(true);
    }
  });

  it('switches the root immediately on every palette without losing NAVCOM selection', async () => {
    render(<App />);
    expect(await screen.findByText(/Browser preview/)).toHaveTextContent(
      'Desktop connection unavailable',
    );
    fireEvent.click(screen.getByRole('button', { name: /NAVCOM/ }));
    for (const { id } of themes) {
      fireEvent.change(selector(), { target: { value: id } });
      expect(document.documentElement).toHaveAttribute('data-theme', id);
      expect(selector()).toHaveValue(id);
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
    }
    fireEvent.change(selector(), { target: { value: 'unknown' } });
    expect(document.documentElement).toHaveAttribute('data-theme', 'echelon');
    fireEvent.change(selector(), { target: { value: 'amarr' } });
    fireEvent.click(screen.getByRole('button', { name: /Overview/ }));
    expect(await screen.findByText(/Browser preview/)).toBeVisible();
    expect(document.documentElement).toHaveAttribute('data-theme', 'amarr');
    expect(invoke).not.toHaveBeenCalled();
  });

  it('resets on a fresh mount, restores document ownership and never persists or fetches', async () => {
    const read = vi.spyOn(Storage.prototype, 'getItem');
    const write = vi.spyOn(Storage.prototype, 'setItem');
    const cookie = vi.spyOn(document, 'cookie', 'set');
    const fetch = vi.spyOn(globalThis, 'fetch');
    document.documentElement.setAttribute('data-theme', 'outer-owner');
    const app = render(<App />);
    await screen.findByText(/Browser preview/);
    fireEvent.change(selector(), { target: { value: 'gallente' } });
    expect(document.documentElement).toHaveAttribute('data-theme', 'gallente');
    app.unmount();
    expect(document.documentElement).toHaveAttribute(
      'data-theme',
      'outer-owner',
    );
    render(<App />);
    expect(selector()).toHaveValue('echelon');
    expect(document.documentElement).toHaveAttribute('data-theme', 'echelon');
    expect(await screen.findByText(/Browser preview/)).toBeVisible();
    expect(read).not.toHaveBeenCalled();
    expect(write).not.toHaveBeenCalled();
    expect(cookie).not.toHaveBeenCalled();
    expect(fetch).not.toHaveBeenCalled();
    expect(invoke).not.toHaveBeenCalled();
  });
});

describe('native portable Appearance', () => {
  beforeEach(() => {
    vi.mocked(isTauri).mockReturnValue(true);
    vi.mocked(invoke).mockImplementation(async (command) => {
      if (command === 'get_theme') return 'gallente';
      if (command === 'get_app_metadata')
        return {
          name: 'ECHELON OS',
          version: '0.1.0',
          platform: 'windows',
          milestone: 'M1-B',
        };
      return undefined;
    });
  });

  it('restores a saved ID and saves a new choice only after acknowledged IPC', async () => {
    let finish: () => void = () => {};
    vi.mocked(invoke).mockImplementation(async (command) => {
      if (command === 'get_theme') return 'gallente';
      if (command === 'set_theme')
        return new Promise<void>((resolve) => {
          finish = resolve;
        });
      return {
        name: 'ECHELON OS',
        version: '0.1.0',
        platform: 'windows',
        milestone: 'M1-B',
      };
    });
    render(<App />);
    expect(await screen.findByText('Saved with this app folder')).toBeVisible();
    expect(selector()).toHaveValue('gallente');
    fireEvent.change(selector(), { target: { value: 'amarr' } });
    expect(document.documentElement).toHaveAttribute('data-theme', 'amarr');
    expect(selector()).toBeDisabled();
    expect(screen.getByText('Saving appearance…')).toBeVisible();
    fireEvent.change(selector(), { target: { value: 'caldari' } });
    expect(
      vi.mocked(invoke).mock.calls.filter(([name]) => name === 'set_theme'),
    ).toEqual([['set_theme', { theme: 'amarr' }]]);
    finish();
    expect(await screen.findByText('Saved with this app folder')).toBeVisible();
    expect(selector()).toHaveValue('amarr');
    expect(selector()).not.toBeDisabled();
  });

  it('keeps an unsaved preview honest after failure and supports explicit retry', async () => {
    vi.mocked(invoke).mockImplementation(async (command) => {
      if (command === 'get_theme') return 'gallente';
      if (command === 'set_theme')
        throw new Error('private absolute path or OS detail');
      return {
        name: 'ECHELON OS',
        version: '0.1.0',
        platform: 'windows',
        milestone: 'M1-B',
      };
    });
    render(<App />);
    await screen.findByText('Saved with this app folder');
    fireEvent.change(selector(), { target: { value: 'minmatar' } });
    expect(
      await screen.findByText(/This appearance is not saved/),
    ).toBeVisible();
    expect(selector()).toHaveValue('minmatar');
    expect(screen.queryByText(/private absolute path/)).not.toBeInTheDocument();
    vi.mocked(invoke).mockResolvedValue(undefined);
    fireEvent.click(
      screen.getByRole('button', { name: 'Save appearance again' }),
    );
    expect(await screen.findByText('Saved with this app folder')).toBeVisible();
  });

  it.each(['unknown-theme', null])(
    'fails closed on invalid saved response %j, then retries loading',
    async (invalid) => {
      vi.mocked(invoke).mockImplementation(async (command) =>
        command === 'get_theme'
          ? invalid
          : {
              name: 'ECHELON OS',
              version: '0.1.0',
              platform: 'windows',
              milestone: 'M1-B',
            },
      );
      render(<App />);
      expect(
        await screen.findByText(/Saved appearance could not be read/),
      ).toBeVisible();
      expect(selector()).toBeDisabled();
      expect(
        vi.mocked(invoke).mock.calls.some(([name]) => name === 'set_theme'),
      ).toBe(false);
      vi.mocked(invoke).mockResolvedValue('amarr');
      fireEvent.click(
        screen.getByRole('button', { name: 'Retry appearance load' }),
      );
      expect(
        await screen.findByText('Saved with this app folder'),
      ).toBeVisible();
      expect(selector()).toHaveValue('amarr');
    },
  );
});
