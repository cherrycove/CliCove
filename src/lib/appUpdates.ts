import { Channel, invoke } from '@tauri-apps/api/core';
import { get, writable } from 'svelte/store';

export interface UpdatePreferences {
  autoCheck: boolean;
  skippedVersion: string | null;
  lastAttempt: number | null;
  lastChecked: number | null;
}
export interface AppUpdate {
  version: string;
  notes: string;
  publishedAt: string | null;
}
export interface UpdateProgress {
  phase: 'downloading' | 'installing';
  downloaded: number;
  total: number | null;
}
interface UpdateStatus { currentVersion: string; preferences: UpdatePreferences }
interface CheckResult { checked: boolean; update: AppUpdate | null; preferences: UpdatePreferences }
export interface AppUpdateClient {
  status(): Promise<UpdateStatus>;
  check(manual: boolean): Promise<CheckResult>;
  setAutoCheck(enabled: boolean): Promise<UpdatePreferences>;
  skip(version: string): Promise<UpdatePreferences>;
  install(version: string, onProgress: (progress: UpdateProgress) => void): Promise<void>;
}
const client: AppUpdateClient = {
  status: () => invoke('app_update_status'),
  check: (manual) => invoke('check_app_update', { manual }),
  setAutoCheck: (enabled) => invoke('set_app_update_auto_check', { enabled }),
  skip: (version) => invoke('skip_app_update', { version }),
  install: (version, onProgress) => {
    const progress = new Channel<UpdateProgress>();
    progress.onmessage = onProgress;
    return invoke('install_app_update', { version, progress });
  },
};

export function createAppUpdater(api: AppUpdateClient = client) {
  const state = writable({
    currentVersion: '',
    preferences: null as UpdatePreferences | null,
    update: null as AppUpdate | null,
    phase: 'idle' as 'idle' | 'checking' | 'downloading' | 'installing',
    progress: null as UpdateProgress | null,
    open: false,
    saving: false,
    message: '',
    error: '',
  });
  const busy = () => ['downloading', 'installing'].includes(get(state).phase);
  let generation = 0;

  async function check(manual = true) {
    if (get(state).phase !== 'idle' || (!manual && get(state).open)) return;
    const token = generation;
    state.update((s) => ({ ...s, phase: 'checking', message: '', error: '' }));
    try {
      const result = await api.check(manual);
      if (token !== generation) return;
      state.update((s) => ({ ...s, preferences: result.preferences, update: result.update,
        open: Boolean(result.update),
        message: result.checked ? (result.update ? `发现新版本 ${result.update.version}` : manual ? '当前已是最新版本' : '自动检查完成') : s.message,
      }));
    } catch (error) {
      if (token === generation) state.update((s) => ({ ...s, error: String(error) }));
    } finally {
      if (token === generation) state.update((s) => ({ ...s, phase: 'idle' }));
    }
  }

  function start() {
    const token = ++generation;
    void api.status().then((status) => {
      if (token !== generation) return;
      state.update((s) => ({ ...s, ...status }));
      void check(false);
    }).catch((error) => {
      if (token === generation) state.update((s) => ({ ...s, error: String(error) }));
    });
    // Poll locally; the backend persists a six-hour network-check interval across launches.
    const timer = setInterval(() => void check(false), 30 * 60 * 1000);
    return () => { clearInterval(timer); generation++; };
  }

  async function setAutoCheck(enabled: boolean) {
    if (get(state).saving) return;
    state.update((s) => ({ ...s, saving: true, error: '' }));
    try {
      const preferences = await api.setAutoCheck(enabled);
      state.update((s) => ({ ...s, preferences }));
    } catch (error) {
      state.update((s) => ({ ...s, error: String(error) }));
    } finally {
      state.update((s) => ({ ...s, saving: false }));
    }
  }

  function later() {
    if (!busy() && !get(state).saving) state.update((s) => ({ ...s, open: false, error: '' }));
  }

  async function skip() {
    const update = get(state).update;
    if (!update || busy() || get(state).saving) return;
    state.update((s) => ({ ...s, saving: true, error: '' }));
    try {
      const preferences = await api.skip(update.version);
      state.update((s) => ({ ...s, preferences, open: false, message: `已跳过 ${update.version}` }));
    } catch (error) {
      state.update((s) => ({ ...s, error: String(error) }));
    } finally {
      state.update((s) => ({ ...s, saving: false }));
    }
  }

  async function install() {
    const update = get(state).update;
    if (!update || get(state).phase !== 'idle' || get(state).saving) return;
    state.update((s) => ({ ...s, phase: 'downloading', progress: null, error: '' }));
    try {
      await api.install(update.version, (progress) => state.update((s) => ({ ...s, phase: progress.phase, progress })));
      // A successful install exits/restarts the process; keep controls disabled until then.
      state.update((s) => ({ ...s, phase: 'installing' }));
    } catch (error) {
      state.update((s) => ({ ...s, phase: 'idle', progress: null, error: String(error) }));
    }
  }

  return { subscribe: state.subscribe, start, check, setAutoCheck, later, skip, install };
}
export const appUpdates = createAppUpdater();

export function updateDate(value: string | number | null | undefined): string {
  if (value == null) return '';
  const date = new Date(typeof value === 'number' ? value * 1000 : value);
  return Number.isNaN(date.getTime()) ? '' : date.toLocaleString('zh-CN', { hour12: false });
}
