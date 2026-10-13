// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import { invoke } from '@tauri-apps/api/core';
import App from './App.svelte';
import type { LatestVersionInfo, ToolRecord, UpdatePlan } from './lib/types';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn(async () => vi.fn()) }));
vi.mock('@xterm/xterm', () => ({ Terminal: vi.fn() }));

const tool: ToolRecord = {
  id: 'test-cli', name: 'test-cli', displayName: 'Test CLI', executable: '/bin/test-cli', canonicalPath: '/bin/test-cli',
  aliases: ['test-cli'], relatedCommands: ['test-cli'], version: '1.0',
  source: { kind: 'user', label: 'User PATH', scope: 'user' }, category: 'ai-agent', isSystem: false,
  capabilities: { terminal: true, auth: false, config: false, diagnose: false, update: true, versionProbe: true },
  configPaths: [], configPreviewAvailable: [], runtimeDependencies: [],
};
const runtimeTool: ToolRecord = {
  ...tool, id: 'python', name: 'python', displayName: 'Python', aliases: ['python'],
  category: 'runtime',
};
const pending = new Map<string, Promise<unknown>>();

function defer<T>(command: string) {
  let resolve!: (value: T) => void;
  let reject!: (reason: Error) => void;
  const promise = new Promise<T>((done, fail) => { resolve = done; reject = fail; });
  pending.set(command, promise);
  return { resolve, reject };
}

beforeEach(() => {
  pending.clear();
  vi.mocked(invoke).mockReset().mockImplementation(async (command) => {
    if (pending.has(command)) return pending.get(command);
    if (command === 'get_settings') return { defaultWorkingDirectory: '/tmp', showSystemTools: false, showAllExecutables: false, trayEnabled: false };
    if (command === 'scan_tools') return [tool];
    if (command === 'list_resource_catalog') return { providers: [], installations: [], capabilities: [] };
    return [];
  });
  HTMLElement.prototype.scrollTo = vi.fn();
  HTMLElement.prototype.scrollIntoView = vi.fn();
});
afterEach(cleanup);

async function startApp() {
  render(App);
  return await screen.findByRole<HTMLButtonElement>('button', { name: '重新扫描' });
}

describe('loading feedback follows real operations', () => {
  it('shows scan activity without removing the existing list, then stops on completion', async () => {
    const scan = await startApp();
    const job = defer<ToolRecord[]>('scan_tools');
    await fireEvent.click(scan);
    const busy = screen.getByRole<HTMLButtonElement>('button', { name: '正在扫描…' });
    expect(busy.disabled).toBe(true);
    expect(busy.getAttribute('aria-busy')).toBe('true');
    expect(busy.querySelector('svg.block-loader')).not.toBeNull();
    expect(screen.getByRole('button', { name: /Test CLI test-cli/ })).not.toBeNull();
    await fireEvent.click(busy);
    expect(vi.mocked(invoke).mock.calls.filter(([name]) => name === 'scan_tools')).toHaveLength(2);
    job.resolve([tool]);
    await waitFor(() => expect(screen.getByRole<HTMLButtonElement>('button', { name: '重新扫描' }).disabled).toBe(false));
    expect(document.querySelector('svg.block-loader')).toBeNull();
  });

  it('ends scan activity after failure and lets the user retry', async () => {
    const scan = await startApp();
    const job = defer<ToolRecord[]>('scan_tools');
    await fireEvent.click(scan);
    job.reject(new Error('扫描不可用'));
    expect(await screen.findByRole('alert')).not.toBeNull();
    expect(screen.getByRole<HTMLButtonElement>('button', { name: '重新扫描' }).disabled).toBe(false);
    expect(document.querySelector('svg.block-loader')).toBeNull();
  });

  it('does not leave an endless loader when initialization fails before scanning', async () => {
    const job = defer('get_settings');
    render(App);
    expect(screen.getByRole('button', { name: '正在扫描…' }).getAttribute('aria-busy')).toBe('true');
    job.reject(new Error('无法读取设置'));
    await screen.findByRole('alert');
    expect(screen.getByRole<HTMLButtonElement>('button', { name: '重新扫描' }).disabled).toBe(false);
    expect(document.querySelector('svg.block-loader')).toBeNull();
  });

  it('shows plan preparation, prevents duplicate requests and restores the button on failure', async () => {
    await startApp();
    const job = defer('plan_update');
    await fireEvent.click(screen.getByRole('button', { name: '更新' }));
    const preparing = screen.getByRole<HTMLButtonElement>('button', { name: '准备中…' });
    expect(preparing.disabled).toBe(true);
    expect(preparing.querySelector('svg.block-loader')).not.toBeNull();
    await fireEvent.click(preparing);
    expect(vi.mocked(invoke).mock.calls.filter(([name]) => name === 'plan_update')).toHaveLength(1);
    job.reject(new Error('无法连接发布源'));
    await screen.findByRole('alert');
    expect(screen.getByRole<HTMLButtonElement>('button', { name: '更新' }).disabled).toBe(false);
    expect(document.querySelector('svg.block-loader')).toBeNull();
  });

  it('replaces plan loading with confirmation without executing an update', async () => {
    await startApp();
    const job = defer<UpdatePlan>('plan_update');
    await fireEvent.click(screen.getByRole('button', { name: '更新' }));
    job.resolve({ id: 'plan', operation: 'update', toolId: tool.id, toolName: tool.displayName, targetVersion: '2.0', executable: 'test-cli', args: ['update'], displayCommand: 'test-cli update', source: 'test', requiresElevation: false });
    await screen.findByRole('dialog', { name: '确认更新' });
    expect(document.querySelector('svg.block-loader')).toBeNull();
    expect(vi.mocked(invoke).mock.calls.some(([name]) => name === 'execute_update')).toBe(false);
    await fireEvent.click(screen.getByRole('button', { name: '取消' }));
    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
  });

  it('shows per-row activity and restores query buttons after a batch finishes', async () => {
    await startApp();
    await fireEvent.click(screen.getByRole('button', { name: /^安装与更新/ }));
    const job = defer<LatestVersionInfo>('check_latest_version');
    await fireEvent.click(screen.getByRole('button', { name: '检查全部最新版' }));
    const query = screen.getByRole<HTMLButtonElement>('button', { name: '正在查询' });
    expect(query.disabled).toBe(true);
    expect(query.querySelector('svg.block-loader')).not.toBeNull();
    job.resolve({ targetId: tool.id, latestVersion: '2.0', updateAvailable: true, checkedAt: new Date().toISOString(), source: 'test', status: 'checked', message: 'done' });
    await screen.findByRole('button', { name: '检查全部最新版' });
    await waitFor(() => expect(screen.getByRole<HTMLButtonElement>('button', { name: '已查询' }).disabled).toBe(false));
    expect(document.querySelector('svg.block-loader')).toBeNull();
  });
});

async function openScanSettings() {
  await startApp();
  await fireEvent.click(screen.getByRole('button', { name: /^设置/ }));
  return screen.getByRole<HTMLInputElement>('switch', { name: '显示全部本机命令' });
}

const scanCalls = () => vi.mocked(invoke).mock.calls.filter(([command]) => command === 'scan_tools');
const inventoryButton = () => screen.getByRole('button', { name: /^本机 CLI/ });

describe('scan scope changes discover and synchronize inventory', () => {
  it('automatically saves and discovers on both opening and closing the full inventory', async () => {
    const toggle = await openScanSettings();
    const expanded = defer<ToolRecord[]>('scan_tools');
    await fireEvent.click(toggle);
    await waitFor(() => expect(invoke).toHaveBeenCalledWith('save_scan_settings', { showSystem: false, showAllExecutables: true }));
    expect(inventoryButton().querySelector('svg.block-loader')).not.toBeNull();
    expanded.resolve([tool, runtimeTool]);
    await waitFor(() => expect(inventoryButton().textContent).toContain('2'));
    await fireEvent.click(inventoryButton());
    expect(screen.getByRole('button', { name: /Python python/ })).not.toBeNull();
    await fireEvent.click(screen.getByRole('button', { name: /^设置/ }));
    const collapsed = defer<ToolRecord[]>('scan_tools');
    await fireEvent.click(screen.getByRole('switch', { name: '显示全部本机命令' }));
    await waitFor(() => expect(scanCalls().at(-1)?.[1]).toEqual({ showSystem: false, showAllExecutables: false }));
    collapsed.resolve([tool]);
    await waitFor(() => expect(inventoryButton().textContent).toContain('1'));
    await fireEvent.click(inventoryButton());
    expect(screen.queryByRole('button', { name: /Python python/ })).toBeNull();
  });

  it('serializes rapid toggles and never publishes an obsolete expanded result', async () => {
    const toggle = await openScanSettings();
    const expanded = defer<ToolRecord[]>('scan_tools');
    await fireEvent.click(toggle);
    await waitFor(() => expect(scanCalls()).toHaveLength(2));
    const collapsed = defer<ToolRecord[]>('scan_tools');
    await fireEvent.click(toggle);
    expect(scanCalls()).toHaveLength(2);
    expanded.resolve([tool, runtimeTool]);
    await waitFor(() => expect(scanCalls()).toHaveLength(3));
    expect(scanCalls().at(-1)?.[1]).toEqual({ showSystem: false, showAllExecutables: false });
    expect(inventoryButton().textContent).not.toContain('2');
    expect(inventoryButton().querySelector('svg.block-loader')).not.toBeNull();
    collapsed.resolve([tool]);
    await waitFor(() => expect(inventoryButton().textContent).toContain('1'));
    expect(toggle.checked).toBe(false);
    expect(invoke).toHaveBeenLastCalledWith('list_install_candidates');
  });

  it('continues with the latest scope when an obsolete scan fails', async () => {
    const toggle = await openScanSettings();
    const expanded = defer<ToolRecord[]>('scan_tools');
    await fireEvent.click(toggle);
    await waitFor(() => expect(scanCalls()).toHaveLength(2));
    const collapsed = defer<ToolRecord[]>('scan_tools');
    await fireEvent.click(toggle);
    expanded.reject(new Error('过期扫描失败'));
    await waitFor(() => expect(scanCalls()).toHaveLength(3));
    expect(screen.queryByRole('alert')).toBeNull();
    collapsed.resolve([tool]);
    await waitFor(() => expect(inventoryButton().textContent).toContain('1'));
  });

  it('does not save working directory or tray drafts when a scan scope changes', async () => {
    const toggle = await openScanSettings();
    await fireEvent.input(screen.getByRole('textbox', { name: '默认工作目录' }), { target: { value: '/unsaved/directory' } });
    await fireEvent.click(screen.getByRole('switch', { name: '驻留系统托盘' }));
    await fireEvent.click(toggle);
    await waitFor(() => expect(scanCalls()).toHaveLength(2));
    expect(invoke).toHaveBeenCalledWith('save_scan_settings', { showSystem: false, showAllExecutables: true });
    expect(vi.mocked(invoke).mock.calls.some(([command]) => command === 'save_settings')).toBe(false);
    expect(screen.getByRole<HTMLInputElement>('textbox', { name: '默认工作目录' }).value).toBe('/unsaved/directory');
    expect(screen.getByRole<HTMLInputElement>('switch', { name: '驻留系统托盘' }).checked).toBe(true);
  });

  it('includes system commands only when full inventory is enabled', async () => {
    const toggle = await openScanSettings();
    await fireEvent.click(screen.getByRole('switch', { name: '高级模式包含系统命令' }));
    await waitFor(() => expect(scanCalls()).toHaveLength(2));
    await waitFor(() => expect(inventoryButton().querySelector('svg.block-loader')).toBeNull());
    expect(scanCalls().at(-1)?.[1]).toEqual({ showSystem: false, showAllExecutables: false });
    await fireEvent.click(toggle);
    await waitFor(() => expect(scanCalls()).toHaveLength(3));
    expect(scanCalls().at(-1)?.[1]).toEqual({ showSystem: true, showAllExecutables: true });
  });

  it('restores the saved switch state if auto-save fails without scanning an unsaved scope', async () => {
    const toggle = await openScanSettings();
    const save = defer('save_scan_settings');
    await fireEvent.click(toggle);
    save.reject(new Error('磁盘写入失败'));
    await screen.findByRole('alert');
    expect(toggle.checked).toBe(false);
    expect(scanCalls()).toHaveLength(1);
    expect(inventoryButton().querySelector('svg.block-loader')).toBeNull();
  });

  it('marks a failed discovery as stale and can retry the current scope from settings', async () => {
    const toggle = await openScanSettings();
    const scan = defer<ToolRecord[]>('scan_tools');
    await fireEvent.click(toggle);
    await waitFor(() => expect(scanCalls()).toHaveLength(2));
    scan.reject(new Error('发现失败'));
    await screen.findByRole('alert');
    expect(inventoryButton().textContent).not.toContain('1');
    const retry = defer<ToolRecord[]>('scan_tools');
    await fireEvent.click(screen.getByRole('button', { name: '重试发现' }));
    await waitFor(() => expect(scanCalls()).toHaveLength(3));
    expect(scanCalls().at(-1)?.[1]).toEqual({ showSystem: false, showAllExecutables: true });
    retry.resolve([tool, runtimeTool]);
    await waitFor(() => expect(inventoryButton().textContent).toContain('2'));
  });
});
