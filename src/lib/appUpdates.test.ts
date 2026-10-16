import { describe, expect, it, vi } from 'vitest';
import { get } from 'svelte/store';
import { createAppUpdater, type AppUpdateClient, type UpdatePreferences, type UpdateProgress } from './appUpdates';

const preferences: UpdatePreferences = { autoCheck: true, skippedVersion: null, lastAttempt: null, lastChecked: null };
const update = { version: '0.6.13', notes: '新版说明', publishedAt: '2026-10-16T10:00:00Z' };
function setup() {
  const api: AppUpdateClient = {
    status: vi.fn(async () => ({ currentVersion: '0.6.12', preferences })),
    check: vi.fn(async () => ({ checked: true, update, preferences })),
    setAutoCheck: vi.fn(async (autoCheck) => ({ ...preferences, autoCheck })),
    skip: vi.fn(async (skippedVersion) => ({ ...preferences, skippedVersion })),
    install: vi.fn(async () => {}),
  };
  return { api, updater: createAppUpdater(api) };
}

describe('application update flow', () => {
  it('checks on startup without installing, and stops checks after unmount', async () => {
    vi.useFakeTimers();
    const { api, updater } = setup();
    const stop = updater.start();
    await vi.waitFor(() => expect(get(updater).open).toBe(true));
    expect(api.check).toHaveBeenCalledWith(false);
    expect(api.install).not.toHaveBeenCalled();
    stop();
    await vi.advanceTimersByTimeAsync(60 * 60 * 1000);
    expect(api.check).toHaveBeenCalledTimes(1);
    vi.useRealTimers();
  });

  it('later dismisses without persisting a skipped version; manual checks reopen it', async () => {
    const { api, updater } = setup();
    await updater.check();
    updater.later();
    expect(get(updater).open).toBe(false);
    expect(api.skip).not.toHaveBeenCalled();
    await updater.check(true);
    expect(get(updater).open).toBe(true);
    expect(api.check).toHaveBeenLastCalledWith(true);
  });

  it('persists skip and toggle independently and retains the prompt if saving fails', async () => {
    const { api, updater } = setup();
    await updater.check();
    vi.mocked(api.skip).mockRejectedValueOnce(new Error('磁盘不可写'));
    await updater.skip();
    expect(get(updater).open).toBe(true);
    expect(get(updater).error).toContain('磁盘不可写');
    await updater.skip();
    expect(get(updater).preferences?.skippedVersion).toBe('0.6.13');
    expect(get(updater).open).toBe(false);
    await updater.setAutoCheck(false);
    expect(api.setAutoCheck).toHaveBeenCalledWith(false);
  });

  it('disables reentry/dismissal during download and allows retry after signature failure', async () => {
    const { api, updater } = setup();
    let reject!: (error: Error) => void;
    let progress!: (event: UpdateProgress) => void;
    vi.mocked(api.install).mockImplementationOnce((_version, callback) => {
      progress = callback;
      return new Promise((_resolve, fail) => { reject = fail; });
    });
    await updater.check();
    const job = updater.install();
    progress({ phase: 'downloading', downloaded: 100, total: 200 });
    expect(get(updater).progress?.downloaded).toBe(100);
    updater.later();
    await updater.skip();
    await updater.install();
    await updater.check();
    expect(get(updater).open).toBe(true);
    expect(api.install).toHaveBeenCalledTimes(1);
    expect(api.skip).not.toHaveBeenCalled();
    expect(api.check).toHaveBeenCalledTimes(1);
    reject(new Error('签名校验失败'));
    await job;
    expect(get(updater).phase).toBe('idle');
    expect(get(updater).error).toContain('签名校验失败');
    await updater.install();
    expect(get(updater).phase).toBe('installing');
  });

  it('reports a failed check without claiming the installed version is current', async () => {
    const { api, updater } = setup();
    vi.mocked(api.check).mockRejectedValueOnce(new Error('无法获取更新信息'));
    await updater.check();
    expect(get(updater).message).toBe('');
    expect(get(updater).error).toContain('无法获取更新信息');
    expect(get(updater).open).toBe(false);
    await updater.check();
    expect(get(updater).error).toBe('');
    expect(get(updater).open).toBe(true);
  });
});
