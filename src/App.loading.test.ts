// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import { invoke } from '@tauri-apps/api/core';
import App from './App.svelte';
import type { CliInformation, InstallCandidate, LatestVersionInfo, ToolRecord, UpdatePlan } from './lib/types';

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
const installCandidate: InstallCandidate = {
  id: 'vercel', adapterId: 'vercel', displayName: 'Vercel CLI',
  description: 'Vercel 项目部署、预览环境与托管服务管理。',
  introduction: '将本地项目部署为预览环境或正式站点。\n\n适合发布前端网站与 Web 应用。',
  documentationUrl: 'https://vercel.com/docs/cli',
  category: 'cloud-service', packageManager: 'npm', packageName: 'vercel', source: 'npm',
  managerAvailable: true, installed: false,
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
    if (command === 'app_update_status') return { currentVersion: '0.6.12', preferences: { autoCheck: false, skippedVersion: null, lastAttempt: null, lastChecked: null } };
    if (command === 'check_app_update') return { checked: false, update: null, preferences: { autoCheck: false, skippedVersion: null, lastAttempt: null, lastChecked: null } };
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


describe('CliCove update UI', () => {
  it('opens manual update details and lets the user skip the version', async () => {
    await startApp();
    await fireEvent.click(screen.getByRole('button', { name: /设置/ }));
    const job = defer('check_app_update');
    await fireEvent.click(screen.getByRole('button', { name: '检查 CliCove 更新' }));
    job.resolve({ checked: true, update: { version: '0.6.13', notes: '新的应用功能', publishedAt: '2026-10-16T10:00:00Z' }, preferences: { autoCheck: true, skippedVersion: null, lastAttempt: 100, lastChecked: 100 } });
    expect(await screen.findByRole('dialog', { name: 'CliCove 0.6.13 可用' })).not.toBeNull();
    expect(screen.getByText('新的应用功能')).not.toBeNull();
    const save = defer('skip_app_update');
    await fireEvent.click(screen.getByRole('button', { name: '跳过此版本' }));
    save.resolve({ autoCheck: true, skippedVersion: '0.6.13', lastAttempt: 100, lastChecked: 100 });
    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
    expect(vi.mocked(invoke)).toHaveBeenCalledWith('skip_app_update', { version: '0.6.13' });
  });
});

describe('scrollbar integration', () => {
  async function prepareArea(selector: string) {
    const area = document.querySelector(selector)!;
    const viewport = area.querySelector<HTMLElement>('.scroll-viewport')!;
    const rail = area.querySelector<HTMLElement>('.scroll-rail')!;
    Object.defineProperties(viewport, { clientHeight: { value: 200 }, scrollHeight: { value: 1000 } });
    Object.defineProperty(rail, 'clientHeight', { value: 200 });
    await fireEvent.scroll(viewport);
    return { viewport, thumb: area.querySelector<HTMLElement>('[role="scrollbar"]')! };
  }

  it('does not reset detail scrolling by re-publishing the bound viewport', async () => {
    await startApp();
    const { viewport, thumb } = await prepareArea('.detail-panel');
    vi.mocked(HTMLElement.prototype.scrollTo).mockClear();
    await fireEvent.keyDown(thumb, { key: 'End' });
    expect(viewport.scrollTop).toBe(800);
    expect(HTMLElement.prototype.scrollTo).not.toHaveBeenCalled();
  });

  it('keeps scrollbar keys independent from quick-open selection positioning', async () => {
    await startApp();
    await fireEvent.click(screen.getByRole('button', { name: /快速打开/ }));
    const { viewport, thumb } = await prepareArea('.command-results');
    vi.mocked(HTMLElement.prototype.scrollIntoView).mockClear();
    await fireEvent.keyDown(thumb, { key: 'End' });
    expect(viewport.scrollTop).toBe(800);
    expect(HTMLElement.prototype.scrollIntoView).not.toHaveBeenCalled();
    await fireEvent.keyDown(thumb, { key: 'Home' });
    expect(viewport.scrollTop).toBe(0);
    await fireEvent.keyDown(screen.getByRole('combobox'), { key: 'ArrowDown' });
    expect(HTMLElement.prototype.scrollIntoView).toHaveBeenCalled();
  });
});

describe('CLI information in the install catalog', () => {
  it('keeps the same information after a catalog CLI is installed', async () => {
    const installed = { ...tool, adapterId: undefined, displayName: 'Vercel CLI', service: { id: 'vercel', description: installCandidate.description } };
    pending.set('scan_tools', Promise.resolve([installed]));
    pending.set('list_install_candidates', Promise.resolve([{ ...installCandidate, installed: true, installedToolId: tool.id }]));
    pending.set('list_cli_information', Promise.resolve([installCandidate]));
    await startApp();
    await fireEvent.click(screen.getByRole('button', { name: /^安装与更新/ }));
    const trigger = await screen.findByRole('button', { name: '了解 Vercel CLI' });
    expect(screen.getAllByText(installCandidate.description)).toHaveLength(1);
    expect(screen.queryByRole('button', { name: '安装' })).toBeNull();
    await fireEvent.click(trigger);
    expect(screen.getByRole('link', { name: /查看官方说明/ }).getAttribute('href')).toBe(installCandidate.documentationUrl);
    expect(screen.getByRole('dialog', { name: 'Vercel CLI' }).textContent).toContain('将本地项目部署为预览环境或正式站点。');
    expect(vi.mocked(invoke).mock.calls.some(([command]) => ['plan_install', 'plan_update', 'execute_update'].includes(command))).toBe(false);
  });

  it('shows information for an installed native CLI without an installation recipe in both views', async () => {
    const grok: CliInformation = { id: 'grok', displayName: 'Grok CLI', category: 'ai-agent', description: '终端中的编码 Agent。', introduction: '用于理解代码和完成开发任务。', documentationUrl: 'https://docs.x.ai/build/cli/reference' };
    pending.set('scan_tools', Promise.resolve([{ ...tool, name: 'agent', displayName: 'Grok CLI', adapterId: 'grok' }]));
    pending.set('list_cli_information', Promise.resolve([grok]));
    await startApp();
    const detailTrigger = await screen.findByRole('button', { name: '了解 Grok CLI' });
    await fireEvent.click(detailTrigger);
    expect(screen.getByRole('dialog', { name: 'Grok CLI' }).textContent).toContain(grok.introduction);
    await fireEvent.keyDown(screen.getByRole('button', { name: '关闭' }), { key: 'Escape' });
    await waitFor(() => expect(document.activeElement).toBe(detailTrigger));
    await fireEvent.click(screen.getByRole('button', { name: /^安装与更新/ }));
    expect(screen.getByText(grok.description)).not.toBeNull();
    await fireEvent.click(screen.getByRole('button', { name: '了解 Grok CLI' }));
    expect(screen.getByRole('link', { name: /查看官方说明/ }).getAttribute('href')).toBe(grok.documentationUrl);
  });

  async function openCatalog() {
    pending.set('list_install_candidates', Promise.resolve([installCandidate]));
    await startApp();
    await fireEvent.click(screen.getByRole('button', { name: /^安装与更新/ }));
    return screen.findByRole('button', { name: '了解 Vercel CLI' });
  }

  it('shows the purpose and a keyboard-accessible dialog without starting management actions', async () => {
    const trigger = await openCatalog();
    expect(screen.getByText(installCandidate.description)).not.toBeNull();
    await fireEvent.click(trigger);
    const dialog = screen.getByRole('dialog', { name: 'Vercel CLI' });
    expect(dialog.textContent).toContain('将本地项目部署为预览环境或正式站点。');
    expect(dialog.textContent).toContain('适合发布前端网站与 Web 应用。');
    const link = screen.getByRole('link', { name: /查看官方说明/ });
    expect(link.getAttribute('href')).toBe(installCandidate.documentationUrl);
    expect(document.querySelector<HTMLElement>('.app-shell')?.inert).toBe(true);
    const close = screen.getByRole('button', { name: '关闭' });
    await waitFor(() => expect(document.activeElement).toBe(close));
    await fireEvent.keyDown(close, { key: 'Tab' });
    expect(document.activeElement).toBe(link);
    await fireEvent.keyDown(link, { key: 'Tab', shiftKey: true });
    expect(document.activeElement).toBe(close);
    await fireEvent.keyDown(close, { key: 'k', metaKey: true });
    expect(screen.queryByRole('combobox')).toBeNull();
    await fireEvent.keyDown(close, { key: 'Escape' });
    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
    await waitFor(() => expect(document.activeElement).toBe(trigger));
    expect(document.querySelector<HTMLElement>('.app-shell')?.inert).toBe(false);
    expect(vi.mocked(invoke).mock.calls.some(([command]) => ['plan_install', 'execute_update', 'check_install_version', 'open_cli_documentation'].includes(command))).toBe(false);
  });

  it('opens the catalog URL through the backend and allows retry after an opener failure', async () => {
    await fireEvent.click(await openCatalog());
    const opening = defer('open_cli_documentation');
    await fireEvent.click(screen.getByRole('link', { name: /查看官方说明/ }));
    expect(invoke).toHaveBeenCalledWith('open_cli_documentation', { serviceId: 'vercel' });
    const busy = screen.getByRole('link', { name: /正在打开/ });
    await fireEvent.click(busy);
    expect(vi.mocked(invoke).mock.calls.filter(([command]) => command === 'open_cli_documentation')).toHaveLength(1);
    opening.reject(new Error('浏览器不可用'));
    expect((await screen.findByRole('alert')).textContent).toContain('浏览器不可用');
    pending.set('open_cli_documentation', Promise.resolve());
    await fireEvent.click(screen.getByRole('link', { name: /查看官方说明/ }));
    await waitFor(() => expect(screen.queryByRole('alert')).toBeNull());
    expect(vi.mocked(invoke).mock.calls.filter(([command]) => command === 'open_cli_documentation')).toHaveLength(2);
    await fireEvent.click(screen.getByRole('button', { name: '关闭' }));
    expect(screen.queryByRole('dialog')).toBeNull();
  });

  it('keeps information available when the package manager is missing', async () => {
    pending.set('list_install_candidates', Promise.resolve([{ ...installCandidate, managerAvailable: false }]));
    await startApp();
    await fireEvent.click(screen.getByRole('button', { name: /^安装与更新/ }));
    expect(screen.getByRole<HTMLButtonElement>('button', { name: '安装' }).disabled).toBe(true);
    await fireEvent.click(await screen.findByRole('button', { name: '了解 Vercel CLI' }));
    expect(screen.getByRole('dialog', { name: 'Vercel CLI' })).not.toBeNull();
  });
});
