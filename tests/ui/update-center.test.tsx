import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';

const mocks = vi.hoisted(() => ({
  checkForAppUpdate: vi.fn(),
  downloadAndInstallAppUpdate: vi.fn(),
  getCurrentAppVersionSafe: vi.fn(),
  relaunchAppSafely: vi.fn(),
  resolveUpdateChannel: vi.fn()
}));

vi.mock('../../src/lib/updater', () => ({
  checkForAppUpdate: mocks.checkForAppUpdate,
  downloadAndInstallAppUpdate: mocks.downloadAndInstallAppUpdate,
  getCurrentAppVersionSafe: mocks.getCurrentAppVersionSafe,
  relaunchAppSafely: mocks.relaunchAppSafely,
  resolveUpdateChannel: mocks.resolveUpdateChannel
}));

import UpdateCenterCard from '../../src/components/UpdateCenterCard';

describe('UpdateCenterCard', () => {
  beforeEach(() => {
    mocks.getCurrentAppVersionSafe.mockResolvedValue('0.1.0');
    mocks.resolveUpdateChannel.mockReturnValue('stable');
    mocks.checkForAppUpdate.mockResolvedValue({
      currentVersion: '0.1.0',
      supported: true,
      channel: 'stable',
      error: null,
      update: {
        version: '0.2.0',
        date: '2026-03-23T00:00:00Z',
        body: '- Added in-app updater\n- Improved HTTP engine'
      }
    });
    mocks.downloadAndInstallAppUpdate.mockImplementation(async (_update, onProgress) => {
      onProgress?.({ phase: 'started', downloadedBytes: 0, totalBytes: 1024 });
      onProgress?.({ phase: 'progress', downloadedBytes: 1024, totalBytes: 1024 });
      onProgress?.({ phase: 'finished', downloadedBytes: 1024, totalBytes: 1024 });
    });
    mocks.relaunchAppSafely.mockResolvedValue(undefined);
  });

  it('muestra un update disponible y permite instalarlo y reiniciar', async () => {
    const user = userEvent.setup();
    render(<UpdateCenterCard autoCheck />);

    await user.click(screen.getByRole('button', { name: /check updates/i }));
    expect(await screen.findByText(/update disponible/i)).toBeInTheDocument();
    expect(screen.getByText(/0\.1\.0 → 0\.2\.0/)).toBeInTheDocument();
    expect(screen.getByText(/added in-app updater/i)).toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: /download & install/i }));

    await waitFor(() => {
      expect(mocks.downloadAndInstallAppUpdate).toHaveBeenCalledTimes(1);
      expect(screen.getByRole('button', { name: /relaunch/i })).toBeInTheDocument();
    });

    await user.click(screen.getByRole('button', { name: /relaunch/i }));
    expect(mocks.relaunchAppSafely).toHaveBeenCalledTimes(1);
  });
});
