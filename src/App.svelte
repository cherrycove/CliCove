<script lang="ts">
  import { onMount, tick } from 'svelte';
  import { listen } from '@tauri-apps/api/event';
  import { api } from './lib/api';
  import { canQueryLatest, filterTools, indexLatestChecks, resolveToolSelection, splitArguments } from './lib/tooling';
  import Icon from './lib/Icon.svelte';
  import BlockLoader from './lib/BlockLoader.svelte';
  import brandMarkUrl from './assets/clicove-mark.svg';
  import ResourceMark from './lib/ResourceMark.svelte';
  import CliStatus from './lib/CliStatus.svelte';
  import { sourceLabel } from './lib/health';
  import { focusDialog } from './lib/focusDialog';
  import TerminalPane from './lib/TerminalPane.svelte';
  import type {
    AppSettings,
    AuthenticationStatus,
    ConfigPreview,
    CustomAdapterInfo,
    DiagnosticReport,
    InstallCandidate,
    LatestVersionInfo,
    ResourceCatalog,
    SessionHistoryRecord,
    TerminalSession,
    ToolCategory,
    ToolRecord,
    UpdatePlan,
    UpdateHistoryRecord,
  } from './lib/types';

  type View = 'tools' | 'resources' | 'terminals' | 'updates' | 'settings';
  type ScanScope = Pick<AppSettings, 'showSystemTools' | 'showAllExecutables'>;
  type ScanRequest = { scope: ScanScope; revision: number };
  type PaletteItem = {
    id: string;
    kind: 'view' | 'tool';
    title: string;
    subtitle: string;
    icon: string;
    badge?: string;
    view?: View;
    toolId?: string;
  };

  const viewOrder: View[] = ['tools', 'resources', 'updates', 'terminals', 'settings'];
  const viewMeta: Record<View, { title: string; subtitle: string; icon: string; label: string }> = {
    tools: {
      title: '本机 CLI',
      subtitle: '查看安装状态、检查问题，并打开 CLI',
      icon: 'library',
      label: '本机 CLI',
    },
    resources: {
      title: '服务与安装',
      subtitle: '按服务查看本机安装与支持的操作',
      icon: 'resources',
      label: '服务与安装',
    },
    updates: {
      title: '安装与更新',
      subtitle: '查询最新版、确认命令，并在执行后验证实际版本',
      icon: 'updates',
      label: '安装与更新',
    },
    terminals: {
      title: '会话工作区',
      subtitle: '集中查看交互、诊断、登录和更新终端',
      icon: 'terminal',
      label: '终端',
    },
    settings: {
      title: '设置',
      subtitle: '配置 CliCove 的本地行为、扫描范围与适配器信任',
      icon: 'settings',
      label: '设置',
    },
  };

  const emptyResourceCatalog: ResourceCatalog = {
    providers: [],
    installations: [],
    capabilities: [],
  };

  const categoryNames: Record<ToolCategory, string> = {
    'ai-agent': 'Agent 能力',
    collaboration: '协作与业务服务',
    'cloud-service': '云服务',
    observability: '可观测与运维服务',
    infrastructure: '基础设施工具（高级）',
    runtime: '语言与运行时',
    android: 'Android',
    'developer-tool': '开发工具',
    system: '系统工具',
    other: '其他工具',
  };
  const categoryShortNames: Record<ToolCategory, string> = {
    'ai-agent': 'Agent',
    collaboration: '协作',
    'cloud-service': '云服务',
    observability: '可观测',
    infrastructure: '基础设施',
    runtime: '运行时',
    android: 'Android',
    'developer-tool': '开发',
    system: '系统',
    other: '其他',
  };
  const categoryDescriptions: Record<ToolCategory, string> = {
    'ai-agent': '编码 Agent 、会话与自动化执行入口',
    collaboration: '团队协作、项目与业务系统接口',
    'cloud-service': '云厂商与托管服务的命令行控制面',
    observability: '指标、日志、追踪与运维诊断工具',
    infrastructure: '容器、集群、网络与基础设施工具',
    runtime: '语言、包管理器与本地运行环境',
    android: 'Android 平台调试与构建工具',
    'developer-tool': '通用开发、构建与代码质量工具',
    system: '操作系统内置命令与管理入口',
    other: '尚未归入明确能力域的命令行工具',
  };
  const categoryIcons: Record<ToolCategory, string> = {
    'ai-agent': 'terminal',
    collaboration: 'resources',
    'cloud-service': 'package',
    observability: 'activity',
    infrastructure: 'sliders',
    runtime: 'command',
    android: 'package',
    'developer-tool': 'library',
    system: 'settings',
    other: 'package',
  };
  const capabilityCategories: ToolCategory[] = [
    'ai-agent',
    'cloud-service',
    'collaboration',
    'observability',
  ];

  const defaultSettings: AppSettings = {
    showSystemTools: false,
    showAllExecutables: false,
    trayEnabled: false,
    defaultWorkingDirectory: '',
  };

  let view: View = 'tools';
  let tools: ToolRecord[] = [];
  let selectedId = '';
  let query = '';
  let category: ToolCategory | 'all' = 'all';
  let scanning = true;
  let activeScan: Promise<void> | undefined;
  let queuedScan: ScanRequest | undefined;
  let scanRevision = 0;
  let requestedScope: ScanScope | undefined;
  let savedScope: ScanScope = scanScope(defaultSettings);
  let inventoryScope: ScanScope = scanScope(defaultSettings);
  let scanFailed = false;
  let settingsReady = false;
  let error = '';
  let notice = '';
  let settings = defaultSettings;
  let savingSettings = false;
  let sessions: TerminalSession[] = [];
  let sessionHistory: SessionHistoryRecord[] = [];
  let activeSessionId = '';
  let argsText = '';
  let toolListElement: HTMLElement | undefined;
  let detailPanelElement: HTMLElement | undefined;
  let updatePlan: UpdatePlan | null = null;
  let planningTargetId = '';
  let configPreview: ConfigPreview | null = null;
  let confirmingExit = false;
  let customAdapters: CustomAdapterInfo[] = [];
  let resourceCatalog = emptyResourceCatalog;
  let selectedProviderId = '';
  let adapterTrustCandidate: CustomAdapterInfo | null = null;
  let installCandidates: InstallCandidate[] = [];
  let latestChecks: LatestVersionInfo[] = [];
  let updateHistory: UpdateHistoryRecord[] = [];
  let authenticationStatuses: AuthenticationStatus[] = [];
  let diagnosticReports: DiagnosticReport[] = [];
  let checkingAuthenticationIds: string[] = [];
  let runningDiagnosticIds: string[] = [];
  let probingIds: string[] = [];
  let checkingTargets: string[] = [];
  let recentlyCheckedTargets: string[] = [];
  let checkingAll = false;
  let bulkCompleted = 0;
  let bulkTotal = 0;
  let bulkCurrent = '';
  let errorTimer: number | undefined;
  let noticeTimer: number | undefined;
  let noticeTone: 'success' | 'warning' = 'success';
  let paletteOpen = false;
  let paletteQuery = '';
  let paletteIndex = 0;
  let paletteInput: HTMLInputElement | undefined;
  let paletteResults: HTMLElement | undefined;
  let searchInput: HTMLInputElement | undefined;
  let paletteTrigger: HTMLElement | undefined;
  let dialogTrigger: HTMLElement | null = null;
  let hasBlockingOverlay = false;

  $: selected = tools.find((tool) => tool.id === selectedId);
  $: if (selectedId) void tick().then(() => detailPanelElement?.scrollTo({ top: 0 }));
  $: filtered = filterTools(tools, query, category);
  $: if (view === 'tools') {
    const nextId = resolveToolSelection(filtered, selectedId);
    if (nextId !== selectedId) {
      selectedId = nextId;
      configPreview = null;
    }
  }
  $: currentView = viewMeta[view];
  $: paletteItems = buildPaletteItems(paletteQuery, tools);
  $: if (paletteIndex < 0 || paletteIndex >= paletteItems.length) paletteIndex = Math.max(0, paletteItems.length - 1);
  $: if (paletteOpen && paletteItems[paletteIndex]) {
    void tick().then(() => paletteResults?.querySelector('[aria-selected="true"]')?.scrollIntoView({ block: 'nearest', behavior: 'instant' }));
  }
  $: categoryOptions = Object.entries(categoryNames).filter(
    ([value]) => inventoryScope.showAllExecutables || capabilityCategories.includes(value as ToolCategory),
  );
  $: categoryCounts = tools.reduce((counts, tool) => {
    counts.set(tool.category, (counts.get(tool.category) ?? 0) + 1);
    return counts;
  }, new Map<ToolCategory, number>());
  $: activeCategoryDescription = category === 'all'
    ? '按服务能力分类浏览已发现的本机 CLI'
    : categoryDescriptions[category];
  $: updatable = tools.filter((tool, index, all) => {
    if (!tool.capabilities.update) return false;
    const key = tool.source.packageName
      ? `${tool.source.kind}:${tool.source.packageName}`
      : `tool:${tool.id}`;
    return all.findIndex((candidate) => {
      const candidateKey = candidate.source.packageName
        ? `${candidate.source.kind}:${candidate.source.packageName}`
        : `tool:${candidate.id}`;
      return candidate.capabilities.update && candidateKey === key;
    }) === index;
  });
  $: missingCandidates = installCandidates.filter((candidate) => !candidate.installed);
  $: latestCheckByTarget = indexLatestChecks(latestChecks);
  $: availableUpdates = latestChecks.filter(
    (check) =>
      check.updateAvailable === true &&
      (updatable.some((tool) => tool.id === check.targetId) ||
        missingCandidates.some((candidate) => `install:${candidate.id}` === check.targetId)),
  ).length;
  $: unsupportedUpdates = updatable.filter(
    (tool) => latestCheckByTarget.get(tool.id)?.status === 'unsupported',
  ).length;
  $: selectedProvider = resourceCatalog.providers.find(
    (provider) => provider.id === selectedProviderId,
  );
  $: providerInstallations = resourceCatalog.installations.filter(
    (installation) => installation.providerId === selectedProviderId,
  );
  $: selectedAuthentication = selected
    ? authenticationStatuses.find((status) => status.toolId === selected.id)
    : undefined;
  $: selectedDiagnostic = selected
    ? diagnosticReports.find((report) => report.toolId === selected.id)
    : undefined;
  $: activeSessions = sessions.filter((session) => session.status === 'running');
  $: hasBlockingOverlay = Boolean(confirmingExit || adapterTrustCandidate || updatePlan);
  $: if (hasBlockingOverlay && paletteOpen) {
    paletteOpen = false;
    paletteTrigger = undefined;
  }

  function buildPaletteItems(value: string, inventory: ToolRecord[]): PaletteItem[] {
    const needle = value.trim().toLocaleLowerCase();
    const pages = viewOrder.map((target, index) => ({
      id: `view:${target}`,
      kind: 'view' as const,
      title: viewMeta[target].label,
      subtitle: viewMeta[target].subtitle,
      icon: viewMeta[target].icon,
      badge: `⌘${index + 1}`,
      view: target,
    }));
    const toolItems = inventory.map((tool) => ({
      id: `tool:${tool.id}`,
      kind: 'tool' as const,
      title: tool.displayName,
      subtitle: `${tool.aliases.join(' · ')} · ${sourceLabel(tool.source)}`,
      icon: 'terminal',
      badge: categoryNames[tool.category],
      toolId: tool.id,
    }));
    return [...pages, ...toolItems]
      .filter((item) => !needle || `${item.title} ${item.subtitle}`.toLocaleLowerCase().includes(needle))
      .slice(0, 10);
  }

  function openPalette(trigger?: HTMLElement) {
    if (hasBlockingOverlay) return;
    paletteTrigger = trigger ?? (document.activeElement instanceof HTMLElement ? document.activeElement : undefined);
    paletteQuery = '';
    paletteIndex = 0;
    paletteOpen = true;
    void tick().then(() => paletteInput?.focus());
  }

  function closePalette(restoreFocus = true) {
    const trigger = paletteTrigger;
    paletteOpen = false;
    paletteTrigger = undefined;
    if (restoreFocus && trigger) void tick().then(() => trigger.focus());
  }

  function openPaletteFromButton(event: MouseEvent) {
    openPalette(event.currentTarget as HTMLElement);
  }

  function executePaletteItem(item: PaletteItem) {
    if (item.kind === 'view' && item.view) {
      view = item.view;
    } else if (item.toolId) {
      selectTool(item.toolId);
    }
    closePalette();
  }

  function selectTool(id: string) {
    // Quick-open can select a tool outside the current search/category.
    if (!filtered.some((tool) => tool.id === id)) {
      query = '';
      category = 'all';
    }
    selectedId = id;
    configPreview = null;
    view = 'tools';
  }

  function selectCategory(next: ToolCategory | 'all') {
    category = next;
  }

  function handleGlobalKeydown(event: KeyboardEvent) {
    const target = event.target as HTMLElement | null;
    const typing = target?.isContentEditable || ['INPUT', 'TEXTAREA', 'SELECT'].includes(target?.tagName ?? '');
    const commandKey = event.metaKey || event.ctrlKey;

    if (hasBlockingOverlay) {
      if (commandKey && event.key.toLocaleLowerCase() === 'k') {
        event.preventDefault();
        return;
      }
      if (event.key === 'Escape') {
        event.preventDefault();
        if (confirmingExit) confirmingExit = false;
        else if (adapterTrustCandidate) adapterTrustCandidate = null;
        else if (updatePlan) void dismissUpdatePlan();
      }
      return;
    }

    if (commandKey && event.key.toLocaleLowerCase() === 'k') {
      event.preventDefault();
      paletteOpen ? closePalette() : openPalette();
      return;
    }
    if (paletteOpen) {
      if (event.key === 'Escape') {
        event.preventDefault();
        closePalette();
      } else if (event.key === 'ArrowDown') {
        event.preventDefault();
        paletteIndex = Math.min(paletteIndex + 1, Math.max(0, paletteItems.length - 1));
      } else if (event.key === 'ArrowUp') {
        event.preventDefault();
        paletteIndex = Math.max(paletteIndex - 1, 0);
      } else if (event.key === 'Enter' && paletteItems[paletteIndex]) {
        event.preventDefault();
        executePaletteItem(paletteItems[paletteIndex]);
      }
      return;
    }
    if (commandKey && /^[1-5]$/.test(event.key)) {
      event.preventDefault();
      view = viewOrder[Number(event.key) - 1];
      return;
    }
    if (!typing && event.key === '/') {
      event.preventDefault();
      view = 'tools';
      void tick().then(() => searchInput?.focus());
    }
  }

  onMount(() => {
    let unlistenExit: (() => void) | undefined;
    let unlistenTerminalExit: (() => void) | undefined;
    void listen('exit-confirmation-requested', () => {
      dialogTrigger = document.activeElement instanceof HTMLElement ? document.activeElement : null;
      confirmingExit = true;
    }).then(
      (fn) => (unlistenExit = fn),
    );
    void listen<{ sessionId: string; exitCode?: number }>('terminal-exit', ({ payload }) => {
      sessions = sessions.map((session) =>
        session.id === payload.sessionId
          ? {
              ...session,
              status: 'exited',
              exitCode: payload.exitCode,
              endedAt: new Date().toISOString(),
            }
          : session,
      );
      void api.sessionHistory().then((history) => (sessionHistory = history));
      void finalizeManagementSession(payload.sessionId);
    }).then((fn) => (unlistenTerminalExit = fn));
    void initialize();
    return () => {
      unlistenExit?.();
      unlistenTerminalExit?.();
    };
  });

  async function initialize() {
    try {
      query = '';
      category = 'all';
      settings = await api.getSettings();
      savedScope = scanScope(settings);
      inventoryScope = savedScope;
      settingsReady = true;
      sessions = await api.terminalSessions();
      sessionHistory = await api.sessionHistory();
      activeSessionId = sessions.at(-1)?.id ?? '';
      await scan();
      await refreshUpdateData();
      [authenticationStatuses, diagnosticReports] = await Promise.all([
        api.authenticationStatuses(),
        api.diagnosticReports(),
      ]);
      await tick();
      toolListElement?.scrollTo({ top: 0 });
    } catch (cause) {
      scanning = false;
      scanFailed = true;
      showError(cause);
    }
  }

  function scanScope(value: AppSettings): ScanScope {
    return { showSystemTools: value.showSystemTools, showAllExecutables: value.showAllExecutables };
  }

  function sameScope(left: ScanScope, right: ScanScope | undefined) {
    return left.showSystemTools === right?.showSystemTools && left.showAllExecutables === right?.showAllExecutables;
  }

  function changeScanScope(field: keyof ScanScope, checked: boolean) {
    settings = { ...settings, [field]: checked };
    void scan();
  }

  function scan() {
    const scope = scanScope(settings);
    if (activeScan && sameScope(scope, requestedScope)) return activeScan;
    requestedScope = scope;
    queuedScan = { scope, revision: ++scanRevision };
    if (activeScan) return activeScan;
    activeScan = scanInventory().finally(() => { activeScan = undefined; });
    return activeScan;
  }

  async function scanInventory() {
    scanning = true;
    scanFailed = false;
    error = '';
    configPreview = null;
    try {
      // Keep backend registry/cache writes in order, and publish only the newest scope.
      while (queuedScan) {
        const request = queuedScan;
        queuedScan = undefined;
        let savingScope = false;
        try {
          if (!sameScope(request.scope, savedScope)) {
            savingScope = true;
            await api.saveScanSettings(request.scope.showSystemTools, request.scope.showAllExecutables);
            savedScope = request.scope;
            savingScope = false;
          }
          if (request.revision !== scanRevision) continue;
          const nextAdapters = await api.listCustomAdapters();
          if (request.revision !== scanRevision) continue;
          const nextTools = await api.scanTools(
            request.scope.showAllExecutables && request.scope.showSystemTools,
            request.scope.showAllExecutables,
          );
          if (request.revision !== scanRevision) continue;
          const [nextCatalog, nextCandidates] = await Promise.all([
            api.listResourceCatalog(), api.listInstallCandidates(),
          ]);
          if (request.revision !== scanRevision) continue;
          tools = nextTools;
          customAdapters = nextAdapters;
          resourceCatalog = nextCatalog;
          installCandidates = nextCandidates;
          inventoryScope = request.scope;
          normalizeProviderSelection();
          if (category !== 'all' && !tools.some((tool) => tool.category === category)) category = 'all';
          if (!tools.some((tool) => tool.id === selectedId)) selectedId = tools[0]?.id ?? '';
        } catch (cause) {
          if (request.revision !== scanRevision) continue;
          scanFailed = true;
          if (savingScope) {
            settings = { ...settings, ...savedScope };
            showError(`扫描范围未保存，已恢复原设置：${cause}`);
          } else {
            showError(cause);
          }
        }
      }
    } finally {
      scanning = false;
    }
  }

  function normalizeProviderSelection() {
    if (!resourceCatalog.providers.some((provider) => provider.id === selectedProviderId)) {
      selectedProviderId = resourceCatalog.providers[0]?.id ?? '';
    }
  }

  async function probe(tool: ToolRecord) {
    if (probingIds.includes(tool.id)) return;
    probingIds = [...probingIds, tool.id];
    try {
      const version = await api.probeVersion(tool.id);
      tools = tools.map((item) => (item.id === tool.id ? { ...item, version } : item));
      showNotice('版本信息已刷新');
    } catch (cause) {
      showError(cause);
    } finally {
      probingIds = probingIds.filter((id) => id !== tool.id);
    }
  }

  async function launch(
    tool: ToolRecord,
    args: string[] = parseArgs(),
    cwd: string = settings.defaultWorkingDirectory,
  ) {
    try {
      const session = await api.spawnTerminal(tool.id, args, cwd);
      addSession(session);
    } catch (cause) {
      showError(cause);
    }
  }

  async function runAction(action: 'diagnostic' | 'auth' | 'config', tool: ToolRecord) {
    try {
      const session =
        action === 'diagnostic'
          ? await api.runDiagnostic(tool.id)
          : action === 'auth'
            ? await api.runAuth(tool.id)
            : await api.showConfig(tool.id);
      addSession(session);
    } catch (cause) {
      showError(cause);
    }
  }

  async function checkAuthentication(tool: ToolRecord) {
    if (checkingAuthenticationIds.includes(tool.id) || runningDiagnosticIds.includes(tool.id)) return;
    checkingAuthenticationIds = [...checkingAuthenticationIds, tool.id];
    try {
      const status = await api.checkAuthenticationStatus(tool.id);
      authenticationStatuses = [
        status,
        ...authenticationStatuses.filter((item) => item.toolId !== tool.id),
      ];
      showNotice(status.message, status.status === 'authenticated' ? 'success' : 'warning');
    } catch (cause) {
      showError(cause);
    } finally {
      checkingAuthenticationIds = checkingAuthenticationIds.filter((id) => id !== tool.id);
    }
  }

  async function runStructuredDiagnostic(tool: ToolRecord) {
    if (runningDiagnosticIds.includes(tool.id) || checkingAuthenticationIds.includes(tool.id)) return;
    runningDiagnosticIds = [...runningDiagnosticIds, tool.id];
    try {
      const report = await api.runStructuredDiagnostic(tool.id);
      if (report.installation?.version) {
        tools = tools.map((item) => item.id === tool.id && item.executable === report.installation?.executable
          && item.canonicalPath === report.installation?.canonicalPath && item.version === tool.version
          ? { ...item, version: report.installation.version } : item);
      }
      diagnosticReports = [
        report,
        ...diagnosticReports.filter((item) => item.toolId !== tool.id),
      ];
      authenticationStatuses = await api.authenticationStatuses();
      showNotice(report.summary, report.status === 'healthy' ? 'success' : 'warning');
    } catch (cause) {
      showError(cause);
    } finally {
      runningDiagnosticIds = runningDiagnosticIds.filter((id) => id !== tool.id);
    }
  }

  function addSession(session: TerminalSession) {
    sessions = [...sessions.filter((item) => item.id !== session.id), session];
    activeSessionId = session.id;
    view = 'terminals';
  }

  async function createUpdatePlan(tool: ToolRecord, trigger: HTMLElement) {
    if (planningTargetId) return;
    planningTargetId = tool.id;
    try {
      const plan = await api.planUpdate(tool.id);
      dialogTrigger = trigger;
      updatePlan = plan;
    } catch (cause) {
      showError(cause);
    } finally {
      planningTargetId = '';
      await refreshLatestChecks();
    }
  }

  async function createInstallPlan(candidate: InstallCandidate, trigger: HTMLElement) {
    if (planningTargetId) return;
    planningTargetId = `install:${candidate.id}`;
    try {
      const plan = await api.planInstall(candidate.id);
      dialogTrigger = trigger;
      updatePlan = plan;
    } catch (cause) {
      showError(cause);
    } finally {
      planningTargetId = '';
      await refreshLatestChecks();
    }
  }

  async function confirmUpdate() {
    if (!updatePlan) return;
    try {
      const session = await api.executeUpdate(
        updatePlan.id,
        settings.defaultWorkingDirectory,
      );
      updatePlan = null;
      addSession(session);
    } catch (cause) {
      updatePlan = null;
      showError(cause);
    }
  }

  async function dismissUpdatePlan() {
    if (!updatePlan) return;
    const planId = updatePlan.id;
    updatePlan = null;
    try {
      await api.cancelUpdatePlan(planId);
    } catch (cause) {
      showError(cause);
    }
  }

  async function previewConfig(tool: ToolRecord, index: number) {
    try {
      configPreview = await api.configPreview(tool.id, index);
    } catch (cause) {
      showError(cause);
    }
  }

  async function saveAppSettings() {
    if (savingSettings || scanning) return;
    savingSettings = true;
    try {
      const nextSettings = { ...settings };
      await api.saveSettings(nextSettings);
      savedScope = scanScope(nextSettings);
      showNotice('设置已保存');
      await scan();
    } catch (cause) {
      showError(cause);
    } finally {
      savingSettings = false;
    }
  }

  async function confirmAdapterTrust() {
    if (!adapterTrustCandidate) return;
    try {
      customAdapters = await api.trustCustomAdapter(
        adapterTrustCandidate.path,
        adapterTrustCandidate.fingerprint,
      );
      adapterTrustCandidate = null;
      await scan();
      showNotice('适配器已按当前内容指纹信任并启用');
    } catch (cause) {
      adapterTrustCandidate = null;
      showError(cause);
    }
  }

  async function revokeAdapter(adapter: CustomAdapterInfo) {
    try {
      customAdapters = await api.revokeCustomAdapter(adapter.path, adapter.fingerprint);
      await scan();
      showNotice('适配器信任已撤销');
    } catch (cause) {
      showError(cause);
    }
  }

  async function refreshCustomAdapters() {
    try {
      await scan();
      showNotice('自定义适配器目录已刷新');
    } catch (cause) {
      showError(cause);
    }
  }

  async function openAdapterDirectory() {
    try {
      await api.openAdapterDirectory();
    } catch (cause) {
      showError(cause);
    }
  }

  function closeSession(event: CustomEvent<string>) {
    sessions = sessions.filter((session) => session.id !== event.detail);
    activeSessionId = sessions.at(-1)?.id ?? '';
  }

  function updateSession(event: CustomEvent<TerminalSession>) {
    sessions = sessions.map((session) =>
      session.id === event.detail.id ? event.detail : session,
    );
  }

  async function restartSession(event: CustomEvent<TerminalSession>) {
    const previous = event.detail;
    const tool = tools.find((item) => item.id === previous.toolId);
    if (!tool) {
      showError('原工具不在当前扫描清单中，请重新扫描');
      return;
    }
    await launch(tool, previous.args, previous.cwd);
  }

  function selectSession(session: TerminalSession) {
    activeSessionId = session.id;
  }

  async function closeExitedSessions() {
    for (const session of sessions.filter((item) => item.status === 'exited')) {
      try {
        await api.terminalClose(session.id);
      } catch {
        // A concurrently removed tab is already in the desired state.
      }
    }
    sessions = sessions.filter((session) => session.status !== 'exited');
    activeSessionId = sessions.at(-1)?.id ?? '';
  }

  async function finalizeManagementSession(sessionId: string) {
    try {
      const record = await api.finalizeUpdate(sessionId);
      await refreshUpdateData();
      await scan();
      showNotice(
        record.status === 'succeeded'
          ? `${record.toolName} 已完成并通过版本验证`
          : `${record.toolName} 未通过更新验证`,
      );
    } catch (cause) {
      if (!String(cause).includes('不是安装或更新会话')) showError(cause);
    }
  }

  function setLatestCheck(check: LatestVersionInfo) {
    latestChecks = [check, ...latestChecks.filter((item) => item.targetId !== check.targetId)];
  }

  async function refreshLatestChecks() {
    latestChecks = await api.latestVersionChecks();
  }

  async function refreshUpdateData() {
    [installCandidates, latestChecks, updateHistory] = await Promise.all([
      api.listInstallCandidates(),
      api.latestVersionChecks(),
      api.listUpdateHistory(),
    ]);
  }

  async function checkToolLatest(tool: ToolRecord, announce = true) {
    checkingTargets = [...checkingTargets, tool.id];
    try {
      const check = await api.checkLatestVersion(tool.id);
      setLatestCheck(check);
      markRecentlyChecked(tool.id);
      if (announce) announceCheckResult(tool.displayName, check);
      return check;
    } catch (cause) {
      showError(cause);
      return undefined;
    } finally {
      checkingTargets = checkingTargets.filter((id) => id !== tool.id);
    }
  }

  async function checkCandidateLatest(candidate: InstallCandidate, announce = true) {
    const targetId = `install:${candidate.id}`;
    checkingTargets = [...checkingTargets, targetId];
    try {
      const check = await api.checkInstallVersion(candidate.id);
      setLatestCheck(check);
      markRecentlyChecked(targetId);
      if (announce) announceCheckResult(candidate.displayName, check);
      return check;
    } catch (cause) {
      showError(cause);
      return undefined;
    } finally {
      checkingTargets = checkingTargets.filter((id) => id !== targetId);
    }
  }

  async function checkAllLatest() {
    const toolsToCheck = updatable;
    const candidatesToCheck = missingCandidates.filter((candidate) => candidate.managerAvailable);
    checkingAll = true;
    bulkCompleted = 0;
    bulkTotal = toolsToCheck.length + candidatesToCheck.length;
    let failures = 0;
    try {
      for (const tool of toolsToCheck) {
        bulkCurrent = tool.displayName;
        const check = await checkToolLatest(tool, false);
        if (!check || check.status === 'error') failures += 1;
        bulkCompleted += 1;
      }
      for (const candidate of candidatesToCheck) {
        bulkCurrent = candidate.displayName;
        const check = await checkCandidateLatest(candidate, false);
        if (!check || check.status === 'error') failures += 1;
        bulkCompleted += 1;
      }
      showNotice(
        failures
          ? `最新版查询完成：${bulkCompleted - failures} 项成功，${failures} 项失败`
          : `最新版查询完成：已检查 ${bulkCompleted} 项`,
        failures ? 'warning' : 'success',
      );
    } finally {
      checkingAll = false;
      bulkCurrent = '';
    }
  }

  function queryButtonLabel(
    checking: boolean,
    check: LatestVersionInfo | undefined,
    recentlyChecked: boolean,
  ) {
    if (checking) return '正在查询';
    if (check?.status === 'unsupported') return '重新检查';
    if (recentlyChecked) return '已查询';
    if (check?.status === 'error') return '重试查询';
    return check ? '重新查询' : '查询最新版';
  }

  function checkStatusText(check: LatestVersionInfo, recentlyChecked: boolean) {
    const checkedLabel = recentlyChecked
      ? '刚刚查询'
      : `上次查询 ${formatCheckTime(check.checkedAt)}`;
    if (check.status === 'unsupported') return `未找到可靠的最新版查询方式 · ${checkedLabel}`;
    if (check.status === 'error') return `查询失败：${check.message} · ${checkedLabel}`;
    if (check.updateAvailable === true) return `发现新版本 · ${checkedLabel}`;
    if (check.updateAvailable === false) return `已是最新 · ${checkedLabel}`;
    return `查询完成 · ${checkedLabel}`;
  }

  function announceCheckResult(name: string, check: LatestVersionInfo) {
    if (check.status === 'error') {
      showError(`${name} 查询失败：${check.message}`);
    } else if (check.status === 'unsupported') {
      showNotice(`${name} 未找到只读更新检查或可靠发布源`, 'warning');
    } else if (check.updateAvailable) {
      showNotice(`${name} 查询完成：发现 ${check.latestVersion ?? '新版本'}`);
    } else {
      showNotice(`${name} 查询完成：当前已是最新版 ${check.latestVersion ?? ''}`.trim());
    }
  }

  function markRecentlyChecked(targetId: string) {
    recentlyCheckedTargets = [
      targetId,
      ...recentlyCheckedTargets.filter((id) => id !== targetId),
    ];
    window.setTimeout(() => {
      recentlyCheckedTargets = recentlyCheckedTargets.filter((id) => id !== targetId);
    }, 4000);
  }

  function formatTime(value?: string) {
    if (!value) return '—';
    return new Intl.DateTimeFormat('zh-CN', {
      month: '2-digit',
      day: '2-digit',
      hour: '2-digit',
      minute: '2-digit',
    }).format(new Date(value));
  }

  function formatCheckTime(value: string) {
    return new Intl.DateTimeFormat('zh-CN', {
      hour: '2-digit',
      minute: '2-digit',
      second: '2-digit',
    }).format(new Date(value));
  }

  function parseArgs() {
    return splitArguments(argsText);
  }

  function missingDependencies(tool: ToolRecord) {
    return tool.runtimeDependencies.filter((dependency) => !dependency.available);
  }

  function runtimeReady(tool: ToolRecord) {
    return missingDependencies(tool).length === 0;
  }

  function scrollToolList(direction: -1 | 1, event: MouseEvent) {
    if (!toolListElement) return;
    const distance = Math.max(toolListElement.clientHeight * 0.8, 240);
    const reducedMotion = window.matchMedia('(prefers-reduced-motion: reduce)').matches;
    toolListElement.scrollBy({ top: distance * direction, behavior: reducedMotion || event.detail === 0 ? 'auto' : 'smooth' });
  }

  function showError(cause: unknown) {
    error = cause instanceof Error ? cause.message : String(cause);
    if (errorTimer) window.clearTimeout(errorTimer);
    errorTimer = window.setTimeout(() => (error = ''), 7000);
  }

  function showNotice(message: string, tone: 'success' | 'warning' = 'success') {
    notice = message;
    noticeTone = tone;
    if (noticeTimer) window.clearTimeout(noticeTimer);
    noticeTimer = window.setTimeout(() => (notice = ''), 3500);
  }
</script>

<svelte:window on:keydown={handleGlobalKeydown} />

<div class="app-shell" inert={hasBlockingOverlay || paletteOpen}>
  <aside class="sidebar">
    <div class="brand">
      <span class="brand-mark"><img src={brandMarkUrl} width="36" height="36" alt="" aria-hidden="true" /></span>
      <div><strong>CliCove</strong><small>本机 CLI 工作台</small></div>
    </div>

    <nav aria-label="主导航">
      <span class="nav-label">工作台</span>
      <button aria-current={view === 'tools' ? 'page' : undefined} class:active={view === 'tools'} on:click={() => (view = 'tools')}>
        <span class="nav-icon"><Icon name="library" size={17} /></span><span>本机 CLI</span><em class="inventory-count" aria-label={scanning ? '正在发现 CLI' : scanFailed ? '清单尚未更新' : `${tools.length} 个 CLI`} title={scanFailed ? '发现失败，请重试' : undefined}>{#if scanning}<BlockLoader size={18} inverse={view === 'tools'} />{:else if scanFailed}—{:else}{tools.length}{/if}</em>
      </button>
      <button aria-current={view === 'resources' ? 'page' : undefined} class:active={view === 'resources'} on:click={() => (view = 'resources')}>
        <span class="nav-icon"><Icon name="resources" size={17} /></span><span>服务与安装</span><em>{resourceCatalog.providers.length}</em>
      </button>
      <button aria-current={view === 'updates' ? 'page' : undefined} class:active={view === 'updates'} on:click={() => (view = 'updates')}>
        <span class="nav-icon"><Icon name="updates" size={17} /></span><span>安装与更新</span><em>{availableUpdates}</em>
      </button>
      <button aria-current={view === 'terminals' ? 'page' : undefined} class:active={view === 'terminals'} on:click={() => (view = 'terminals')}>
        <span class="nav-icon"><Icon name="terminal" size={17} /></span><span>终端</span><em>{sessions.length}</em>
      </button>
      <span class="nav-label nav-label-system">系统</span>
      <button aria-current={view === 'settings' ? 'page' : undefined} class:active={view === 'settings'} on:click={() => (view = 'settings')}>
        <span class="nav-icon"><Icon name="settings" size={17} /></span><span>设置</span><kbd>⌘5</kbd>
      </button>
    </nav>

    <div class="sidebar-composition" aria-hidden="true"><i></i><i></i><i></i></div>
    <div class="sidebar-foot">
      <span class="local-shield"><Icon name="shield" size={16} /></span>
      <div><strong>纯本地模式</strong><small>凭据仍由各 CLI 管理</small></div>
    </div>
  </aside>

  <main>
    <header class="topbar">
      <div class="topbar-copy">
        <span class="page-kicker"><Icon name={currentView.icon} size={13} /> 本机工作台</span>
        <h1>{currentView.title}</h1>
        <p>{currentView.subtitle}</p>
      </div>
      <div class="topbar-actions">
        <button class="command-trigger" on:click={openPaletteFromButton} title="快速打开（⌘K）">
          <Icon name="search" size={15} /><span>快速打开</span><kbd>⌘K</kbd>
        </button>
        {#if view === 'tools' || view === 'resources' || view === 'updates'}
          <button class="scan-button" disabled={scanning} aria-busy={scanning} on:click={scan}>
            {#if scanning}<BlockLoader />{:else}<Icon name="refresh" size={22} />{/if}{scanning ? '正在扫描…' : '重新扫描'}
          </button>
        {/if}
      </div>
    </header>

    {#if view === 'tools'}
      <div class="inventory-controls">
        <div class="toolbar">
          <label class="search"><span><Icon name="search" size={16} /></span><input aria-label="搜索能力、命令或服务来源" bind:this={searchInput} bind:value={query} placeholder="搜索能力、命令或服务来源" /><kbd>/</kbd></label>
          <span class="inventory-state" role="status">{#if scanning}<BlockLoader size={20} />{:else}<i></i>{/if}{scanning ? '正在刷新清单' : scanFailed ? '发现失败 · 显示上次清单' : `${filtered.length} / ${tools.length} 个能力`}</span>
          <div class="list-scroll-controls" aria-label="工具列表滚动控制">
            <button title="向上翻页" aria-label="向上翻页" on:click={(event) => scrollToolList(-1, event)}><Icon name="arrow-up" size={16} /></button>
            <button title="向下翻页" aria-label="向下翻页" on:click={(event) => scrollToolList(1, event)}><Icon name="arrow-down" size={16} /></button>
          </div>
        </div>
        <div class="category-filters" aria-label="按能力分类过滤">
          <button aria-pressed={category === 'all'} class:active={category === 'all'} class="category-filter all-categories" on:click={() => selectCategory('all')}>
            <span class="category-filter-icon"><Icon name="library" size={14} /></span>
            <span><strong>全部</strong><small>能力目录</small></span>
            <em>{tools.length}</em>
          </button>
          {#each categoryOptions as [value, label]}
            <button aria-pressed={category === value} class:active={category === value} class="category-filter" data-category={value} title={label} on:click={() => selectCategory(value as ToolCategory)}>
              <span class="category-filter-icon"><Icon name={categoryIcons[value as ToolCategory]} size={14} /></span>
              <span><strong>{categoryShortNames[value as ToolCategory]}</strong><small>{label}</small></span>
              <em>{categoryCounts.get(value as ToolCategory) ?? 0}</em>
            </button>
          {/each}
        </div>
      </div>

      <div class="tool-layout">
        <section bind:this={toolListElement} class="tool-list" aria-label="CLI 工具列表">
          <div class="tool-list-head"><div><span>{category === 'all' ? '本机能力目录' : categoryNames[category]}</span><small>{activeCategoryDescription}</small></div><em>{filtered.length}</em></div>
          {#if scanning && tools.length === 0}
            <div class="loading-list" role="status"><BlockLoader size={56} /><strong>正在扫描本机 CLI</strong><p>正在识别命令、安装来源与服务能力…</p></div>
          {:else if filtered.length === 0}
            <div class="empty"><b>未找到匹配的能力 CLI</b><span>调整搜索条件，或在设置中启用高级全量视图</span></div>
          {:else}
            {#each filtered as tool (tool.id)}
              <button aria-current={tool.id === selectedId ? 'true' : undefined} class:selected={tool.id === selectedId} class="tool-row" on:click={() => selectTool(tool.id)}>
                <ResourceMark resource={tool} />
                <span class="tool-main"><strong>{tool.displayName}</strong><span class="tool-subline"><small>{tool.aliases.join(' · ')}</small><span class="category-tag" data-category={tool.category}><i></i>{categoryShortNames[tool.category]}</span></span></span>
                <span class="tool-meta"><em class:dependency-error={!runtimeReady(tool)}>{runtimeReady(tool) ? (tool.version?.split(' · ')[0] ?? '未探测') : `缺少 ${missingDependencies(tool).map((item) => item.name).join(', ')}`}</em><small>{sourceLabel(tool.source)}</small></span>
                <span class="chevron"><Icon name="chevron-right" size={15} /></span>
              </button>
            {/each}
          {/if}
        </section>

        <aside class="detail-panel" bind:this={detailPanelElement}>
          {#if selected}
            <div class="detail-hero" data-category={selected.category}>
              <div class="detail-title">
                <div class="detail-identity"><ResourceMark resource={selected} size={72} inverse />
                  <div><span class="detail-eyebrow">{categoryNames[selected.category]}</span><h2>{selected.displayName}</h2><p>{selected.service?.description ?? categoryDescriptions[selected.category]}</p></div>
                </div>
              </div>
            </div>
            <div class="detail-actions">
              <button class="primary" disabled={!runtimeReady(selected)} on:click={() => launch(selected)}><Icon name="terminal" size={15} />打开终端</button>
              {#if selected.capabilities.update}<button disabled={!runtimeReady(selected) || Boolean(planningTargetId)} aria-busy={planningTargetId === selected.id} on:click={(event) => createUpdatePlan(selected, event.currentTarget)}>{#if planningTargetId === selected.id}<BlockLoader />准备中…{:else}<Icon name="updates" size={15} />更新{/if}</button>{/if}
              {#if selected.capabilities.auth}<button disabled={!runtimeReady(selected)} on:click={() => runAction('auth', selected)}><Icon name="lock" size={15} />登录</button>{/if}
            </div>

            {#key selected.id}
              <CliStatus tool={selected} authentication={selectedAuthentication} report={selectedDiagnostic}
                checking={runningDiagnosticIds.includes(selected.id)} checkingAuth={checkingAuthenticationIds.includes(selected.id)}
                probing={probingIds.includes(selected.id)} {scanning}
                oncheck={() => runStructuredDiagnostic(selected)} onauthcheck={() => checkAuthentication(selected)}
                onprobe={() => probe(selected)} onrescan={scan}
                onlogin={() => runAction('auth', selected)} ondiagnose={() => runAction('diagnostic', selected)} />
            {/key}

            <details class="detail-disclosure installation-details">
              <summary><span class="disclosure-icon"><Icon name="package" size={16} /></span><span><strong>更多安装信息</strong><small>识别方式、依赖与相关命令</small></span><Icon name="chevron-right" size={15} /></summary>
              <dl class="facts">
                <div><dt>识别方式</dt><dd>{selected.adapterId ? '专用适配器' : selected.service ? '服务目录' : '命令识别'}</dd></div>
                {#if selected.runtimeDependencies.length}
                  <div><dt>运行依赖</dt><dd>{#each selected.runtimeDependencies as dependency}<span class:dependency-error={!dependency.available} class="dependency-item">{dependency.name} · {dependency.executable ?? '未找到'}</span>{/each}</dd></div>
                {:else}<div><dt>运行依赖</dt><dd>没有额外运行依赖</dd></div>{/if}
                {#if selected.relatedCommands.length > selected.aliases.length}
                  <div><dt>包含命令</dt><dd>{selected.relatedCommands.join('、')}</dd></div>
                {/if}
              </dl>
            </details>

            <details class="detail-disclosure">
              <summary><span class="disclosure-icon"><Icon name="sliders" size={16} /></span><span><strong>启动选项</strong><small>启动参数与工作目录</small></span><Icon name="chevron-right" size={15} /></summary>
              <div class="launch-options">
                <label for="launch-args">启动参数 <small>每行一个参数</small></label>
                <textarea id="launch-args" bind:value={argsText} rows="3" placeholder="例如：&#10;--version"></textarea>
                <label for="working-directory">工作目录</label>
                <input id="working-directory" bind:value={settings.defaultWorkingDirectory} />
              </div>
            </details>

            {#if selected.capabilities.config}
              <div class="config-section">
                <h3>配置</h3>
                {#if selected.configPaths.length}
                  {#each selected.configPaths as path, index}
                    <div class="config-path"><code>{path}</code><span>{#if selected.configPreviewAvailable[index]}<button class="link" on:click={() => previewConfig(selected, index)}>预览配置</button>{/if}<button class="link" on:click={() => api.openConfig(selected.id, index)}>外部打开</button></span>{#if !selected.configPreviewAvailable[index]}<small>此配置暂不支持预览，请在外部编辑器打开。</small>{/if}</div>
                  {/each}
                {:else}
                  <button class="secondary wide" on:click={() => runAction('config', selected)}>通过官方命令显示配置</button>
                {/if}
              </div>
            {/if}

            {#if configPreview}
              <div class="config-preview"><header><span>{configPreview.path}</span><button on:click={() => (configPreview = null)}>×</button></header><p class="preview-notice">{configPreview.notice}</p>{#if configPreview.exists}<pre>{configPreview.content || '{}'}</pre>{/if}</div>
            {/if}
          {:else}
            <div class="empty"><b>选择一个 CLI</b><span>查看版本、来源与可用操作</span></div>
          {/if}
        </aside>
      </div>
    {:else if view === 'resources'}
      <section class="page-card resources-page">
        <div class="resource-summary" aria-label="资源统计">
          <div><strong>{resourceCatalog.providers.length}</strong><span>服务</span><small>CLI 所属服务</small></div>
          <div><strong>{resourceCatalog.installations.length}</strong><span>本机安装</span><small>发现的安装实例</small></div>
          <div><strong>{resourceCatalog.capabilities.length}</strong><span>支持的操作</span><small>终端、认证与更新等</small></div>
        </div>

        <div class="resource-browser">
          <section class="provider-list">
            <header><h2>服务</h2><span>{resourceCatalog.providers.length}</span></header>
            {#each resourceCatalog.providers as provider (provider.id)}
              <button class:active={provider.id === selectedProviderId} on:click={() => (selectedProviderId = provider.id)}>
                <ResourceMark resource={provider} provider size={36} />
                <span><strong>{provider.displayName}</strong><small>{provider.installationCount} 个本机安装</small></span>
              </button>
            {/each}
          </section>

          <section class="provider-detail">
            {#if selectedProvider}
              <header><div><h2>{selectedProvider.displayName}</h2><p>登录与凭据由各 CLI 管理</p></div><span class="provider-category" data-category={selectedProvider.category}>{categoryNames[selectedProvider.category]}</span></header>
              {#each providerInstallations as installation (installation.id)}
                <article class="installation-card">
                  <div class="installation-head"><div><strong>{installation.displayName}</strong><code>{installation.command}</code></div><span class="resource-status status-{installation.status}">{installation.status === 'ready' ? '依赖齐全' : '依赖缺失'}</span></div>
                  <dl><div><dt>来源</dt><dd>{sourceLabel(installation.source)}</dd></div><div><dt>版本</dt><dd>{installation.version ?? '未知'}</dd></div><div class="installation-path"><dt>路径</dt><dd title={installation.executable}>{installation.executable}</dd></div></dl>
                  <div class="capability-list">
                    {#each resourceCatalog.capabilities.filter((capability) => capability.installationId === installation.id) as capability (capability.id)}
                      <span title={capability.description} data-risk={capability.riskLevel}><b>{capability.displayName}</b><small>{{ 'read-only': '只读', controlled: '受控操作', interactive: '交互操作', 'confirmation-required': '需确认' }[capability.riskLevel] ?? 'CLI 操作'}</small></span>
                    {/each}
                  </div>
                </article>
              {/each}
            {/if}
          </section>
        </div>
      </section>
    {:else if view === 'updates'}
      <section class="page-card updates-page">
        <div class="page-intro">
          <span><Icon name="package" size={23} /></span>
          <div><h2>安装与版本管理</h2><p>通过 CLI 官方更新器或原包管理器安装和更新。</p></div>
          <button class="check-all" disabled={checkingAll} aria-busy={checkingAll} on:click={checkAllLatest}>
            {#if checkingAll}<BlockLoader />正在查询 {bulkCompleted}/{bulkTotal}{:else}检查全部最新版{/if}
          </button>
        </div>

        {#if checkingAll}
          <div class="bulk-check-progress" role="status" aria-live="polite">
            <BlockLoader size={36} />
            <div><strong>正在查询 {bulkCurrent || '最新版信息'}</strong><small>已完成 {bulkCompleted} / {bulkTotal} 项，请保持 CliCove 打开</small></div>
            <progress max={Math.max(bulkTotal, 1)} value={bulkCompleted}></progress>
          </div>
        {/if}

        <section class="update-section">
          <header><div><h3>已安装</h3><p>查询不会安装任何内容；确认执行后会自动重新扫描和验证版本。</p></div><span>{updatable.length} 项{unsupportedUpdates ? ` · ${unsupportedUpdates} 项暂不支持查询` : ''}</span></header>
          {#each updatable as tool}
            {@const check = latestCheckByTarget.get(tool.id)}
            {@const checking = checkingTargets.includes(tool.id)}
            <div class:checking={checking} class:recent={recentlyCheckedTargets.includes(tool.id)} class="update-row" aria-busy={checking} aria-live="polite">
              <ResourceMark resource={tool} />
              <div class="update-description">
                <strong>{tool.displayName}</strong>
                <small>{tool.version ?? tool.source.packageVersion ?? '当前版本未知'} <b>→</b> {check?.latestVersion ?? '尚未查询'} · {check?.source ?? tool.source.label}</small>
                {#if checking}
                  <em class="check-status status-checking">正在检查官方更新方式与发布源，请稍候…</em>
                {:else if check}
                  <em title={check.message} class="check-status status-{check.status}">{checkStatusText(check, recentlyCheckedTargets.includes(tool.id))}</em>
                {:else}
                  <em class="check-status status-idle">尚未查询；点击右侧按钮开始</em>
                {/if}
              </div>
              <div class="update-actions">
                <button class:querying={checking} class:complete={recentlyCheckedTargets.includes(tool.id)} class:unsupported={check?.status === 'unsupported'} class="query-button" aria-busy={checking} title={check?.status === 'unsupported' ? check.message : '只读查询，不会执行安装或更新'} disabled={!canQueryLatest(checking, checkingAll)} on:click={() => checkToolLatest(tool)}>
                  {#if checking}<BlockLoader />{:else if recentlyCheckedTargets.includes(tool.id)}<span aria-hidden="true">✓</span>{/if}{queryButtonLabel(checking, check, recentlyCheckedTargets.includes(tool.id))}
                </button>
                <button class="primary" disabled={check?.updateAvailable === false || Boolean(planningTargetId)} aria-busy={planningTargetId === tool.id} on:click={(event) => createUpdatePlan(tool, event.currentTarget)}>{#if planningTargetId === tool.id}<BlockLoader inverse />准备中…{:else}更新{/if}</button>
              </div>
            </div>
          {/each}
        </section>

        <section class="update-section">
          <header><div><h3>可信安装目录</h3><p>以下 CLI 尚未在本机扫描中发现。查询仅获取可安装版本；安装仍需确认并使用本机包管理器。</p></div><span>{missingCandidates.length} 项</span></header>
          {#if missingCandidates.length === 0}
            <div class="update-empty">内置目录中的 CLI 均已安装。</div>
          {:else}
            {#each missingCandidates as candidate}
              {@const check = latestCheckByTarget.get(`install:${candidate.id}`)}
              {@const targetId = `install:${candidate.id}`}
              {@const checking = checkingTargets.includes(targetId)}
              <div class:checking={checking} class:recent={recentlyCheckedTargets.includes(targetId)} class="update-row" aria-busy={checking} aria-live="polite">
                <ResourceMark resource={candidate} />
                <div class="update-description"><strong>{candidate.displayName}</strong><small>{candidate.source} · {check?.latestVersion ? `可安装版本 ${check.latestVersion}` : '可安装版本尚未查询'}</small>{#if !candidate.managerAvailable}<em class="check-status status-error">缺少 {candidate.packageManager}</em>{:else if checking}<em class="check-status status-checking">正在连接 {candidate.source} 查询，请稍候…</em>{:else if check}<em title={check.message} class="check-status status-{check.status}">{checkStatusText(check, recentlyCheckedTargets.includes(targetId))}</em>{:else}<em class="check-status status-idle">尚未查询；点击右侧按钮开始</em>{/if}</div>
                <div class="update-actions">
                  <button class:querying={checking} class:complete={recentlyCheckedTargets.includes(targetId)} class:unsupported={check?.status === 'unsupported'} class="query-button" aria-busy={checking} title={check?.status === 'unsupported' ? check.message : '只读查询，不会执行安装'} disabled={!canQueryLatest(checking, checkingAll, candidate.managerAvailable)} on:click={() => checkCandidateLatest(candidate)}>
                    {#if checking}<BlockLoader />{:else if recentlyCheckedTargets.includes(targetId)}<span aria-hidden="true">✓</span>{/if}{queryButtonLabel(checking, check, recentlyCheckedTargets.includes(targetId))}
                  </button>
                  <button class="primary" disabled={!candidate.managerAvailable || Boolean(planningTargetId)} aria-busy={planningTargetId === targetId} on:click={(event) => createInstallPlan(candidate, event.currentTarget)}>{#if planningTargetId === targetId}<BlockLoader inverse />准备中…{:else}安装{/if}</button>
                </div>
              </div>
            {/each}
          {/if}
        </section>

        <section class="update-section history-section">
          <header><div><h3>安装与更新历史</h3><p>保留操作、版本和验证结果；不保存终端正文或会话参数。</p></div><span>{updateHistory.length} 条</span></header>
          {#if updateHistory.length === 0}
            <div class="update-empty">还没有安装或更新记录。</div>
          {:else}
            {#each updateHistory as record}
              <article class="history-row">
                <span class="history-mark history-{record.status}">{record.status === 'succeeded' ? '✓' : record.status === 'running' ? '…' : '!'}</span>
                <div><strong>{record.operation === 'install' ? '安装' : '更新'} {record.toolName}</strong><small>{record.currentVersion ?? '未安装'} → {record.observedVersion ?? record.targetVersion} · {formatTime(record.verifiedAt ?? record.finishedAt ?? record.startedAt)}</small><p>{record.message || '正在等待终端完成'}</p><code title={record.displayCommand}>{record.displayCommand}</code></div>
                <span class="history-status status-{record.status}">{record.status === 'succeeded' ? '验证通过' : record.status === 'running' ? '执行中' : record.status === 'failed' ? '命令失败' : record.status === 'interrupted' ? '已中断' : '验证失败'}</span>
              </article>
            {/each}
          {/if}
        </section>
      </section>
    {:else if view === 'settings'}
      <section class="page-card settings-page">
        <div class="setting-group"><div><h3><label for="settings-working-directory">默认工作目录</label></h3><p id="settings-working-directory-hint">新终端和管理命令默认在此目录启动。</p></div><input id="settings-working-directory" aria-describedby="settings-working-directory-hint" bind:value={settings.defaultWorkingDirectory} /></div>
        <div class="setting-group setting-toggle"><div><h3><label for="settings-system-tools">高级模式包含系统命令</label></h3><p id="settings-system-tools-hint">仅在全量视图中包含 macOS 或 Windows 自带命令。切换后自动保存并重新发现。</p></div><label class="switch"><input id="settings-system-tools" aria-label="高级模式包含系统命令" aria-describedby="settings-system-tools-hint" role="switch" type="checkbox" disabled={!settingsReady || savingSettings} checked={settings.showSystemTools} on:change={(event) => changeScanScope('showSystemTools', event.currentTarget.checked)} /><span aria-hidden="true"></span></label></div>
        <div class="setting-group setting-toggle"><div><h3><label for="settings-all-executables">高级：显示全部本机命令</label></h3><p id="settings-all-executables-hint">默认仅显示 Agent 可调用的服务能力 CLI；开启后自动发现语言环境、开发工具、基础设施及 PATH 中的每个可执行入口。</p></div><label class="switch"><input id="settings-all-executables" aria-label="显示全部本机命令" aria-describedby="settings-all-executables-hint" role="switch" type="checkbox" disabled={!settingsReady || savingSettings} checked={settings.showAllExecutables} on:change={(event) => changeScanScope('showAllExecutables', event.currentTarget.checked)} /><span aria-hidden="true"></span></label></div>
        <div class="settings-scan-status" role="status"><span>{#if scanning}<BlockLoader size={20} />正在按当前范围发现 CLI…{:else if scanFailed}发现未完成，清单尚未更新。{:else}已发现 {tools.length} 个本机 CLI · 扫描范围自动保存{/if}</span>{#if scanFailed && !scanning}<button on:click={scan}>重试发现</button>{/if}</div>
        <div class="setting-group setting-toggle"><div><h3><label for="settings-tray">驻留系统托盘</label></h3><p id="settings-tray-hint">关闭时减少后台内存占用。此设置在下次启动后生效。</p></div><label class="switch"><input id="settings-tray" aria-label="驻留系统托盘" aria-describedby="settings-tray-hint" role="switch" type="checkbox" bind:checked={settings.trayEnabled} /><span aria-hidden="true"></span></label></div>
        <div class="privacy-note"><span><Icon name="shield" size={20} /></span><div><strong>本地与隐私</strong><p>CliCove 仅从内置白名单提取非敏感配置字段；不会持久化终端参数或完整输出，也不会在后台自动联网检查更新。</p></div></div>
        <section class="adapter-settings">
          <header><div><h3>自定义适配器信任</h3><p>JSON 文件默认不会加载。信任与文件路径及 SHA-256 内容指纹绑定，文件变化后会自动停用。</p></div><span class="adapter-header-actions"><button disabled={scanning} aria-busy={scanning} on:click={refreshCustomAdapters}>{#if scanning}<BlockLoader />刷新中…{:else}刷新{/if}</button><button on:click={openAdapterDirectory}>打开适配器目录</button></span></header>
          {#if customAdapters.length === 0}
            <div class="adapter-empty">适配器目录中没有 JSON 文件。</div>
          {:else}
            {#each customAdapters as adapter (adapter.path)}
              <article class="adapter-card">
                <div class="adapter-card-title"><div><strong>{adapter.displayName}</strong><small>{adapter.id}</small></div><span class="adapter-status status-{adapter.status}">{adapter.status === 'trusted' ? '已信任' : adapter.status === 'pending' ? '待审查' : adapter.status === 'blocked' ? '已阻止' : '无效'}</span></div>
                <code>{adapter.path}</code>
                <p>{adapter.message}</p>
                {#if adapter.fingerprint}<div class="adapter-fingerprint"><span>SHA-256</span><code>{adapter.fingerprint}</code></div>{/if}
                <div class="adapter-card-actions">
                  {#if adapter.status === 'pending'}<button class="primary" on:click={(event) => { dialogTrigger = event.currentTarget; adapterTrustCandidate = adapter; }}>审查并信任</button>{/if}
                  {#if adapter.hasStoredTrust}<button class="ghost danger" on:click={() => revokeAdapter(adapter)}>{adapter.status === 'trusted' ? '撤销信任' : '清除旧信任'}</button>{/if}
                </div>
              </article>
            {/each}
          {/if}
        </section>
        <div class="settings-actions"><span>工作目录与托盘设置更改后点击保存</span><button class="primary save-settings" disabled={savingSettings || scanning} aria-busy={savingSettings} on:click={saveAppSettings}>{#if savingSettings}<BlockLoader inverse />{/if}{savingSettings ? '保存中…' : '保存设置'}</button></div>
      </section>
    {/if}

    <section aria-hidden={view !== 'terminals'} class:workspace-hidden={view !== 'terminals'} class="terminal-workspace">
      <header class="terminal-overview">
        <div><span class="terminal-logo"><Icon name="terminal" size={19} /></span><div><strong>会话工作区</strong><small>{activeSessions.length} 个运行中 · {sessions.length - activeSessions.length} 个已结束 · 输出仅保存在内存</small></div></div>
        <div><button disabled={!sessions.some((session) => session.status === 'exited')} on:click={closeExitedSessions}>关闭已结束</button><button class="primary" on:click={() => (view = 'tools')}><Icon name="plus" size={14} />新建会话</button></div>
      </header>
      {#if sessions.length}
        <div class="terminal-tabs">
          {#each sessions as session}
            <button class:active={session.id === activeSessionId} on:click={() => selectSession(session)}>
              <em class="terminal-tab-icon" aria-hidden="true">
                <Icon name={session.kind === 'management' ? 'updates' : session.kind === 'diagnostic' ? 'activity' : session.kind === 'auth' ? 'lock' : 'terminal'} size={24} />
                <span class="terminal-tab-status" class:stopped={session.status === 'exited'}></span>
              </em>
              <strong>{session.title}</strong><small>{session.status === 'running' ? '运行中' : session.exitCode == null ? '已退出' : `exit ${session.exitCode}`}</small>
            </button>
          {/each}
        </div>
        <div class="terminal-stack">
          {#each sessions as session (session.id)}
            <TerminalPane {session} active={session.id === activeSessionId} on:close={closeSession} on:status={updateSession} on:restart={restartSession} />
          {/each}
        </div>
      {:else}
        <div class="empty terminal-empty"><span><Icon name="terminal" size={30} /></span><b>还没有活动终端</b><p>从工具详情打开交互会话；历史只保存脱敏元数据。</p><button on:click={() => (view = 'tools')}>浏览工具</button></div>
      {/if}
      {#if sessionHistory.length}
        <aside class="recent-session-strip"><span>最近会话</span>{#each sessionHistory.slice(0, 5) as record}<div><strong>{record.title || '未命名会话'}</strong><small>{formatTime(record.startedAt)} · {record.exitCode == null ? '中断/未知' : `exit ${record.exitCode}`}</small></div>{/each}</aside>
      {/if}
    </section>
  </main>
</div>

{#if paletteOpen}
  <div class="command-backdrop" role="presentation" on:mousedown={(event) => event.target === event.currentTarget && closePalette()}>
    <div class="command-palette" role="dialog" aria-modal="true" aria-label="快速打开" use:focusDialog={{ restoreFocus: false }}>
      <label class="command-search">
        <Icon name="search" size={19} />
        <input bind:this={paletteInput} bind:value={paletteQuery} role="combobox" aria-autocomplete="list" aria-expanded="true" aria-controls="command-results" aria-activedescendant={paletteItems[paletteIndex] ? `command-option-${paletteIndex}` : undefined} placeholder="搜索页面、CLI 或动作…" aria-label="搜索命令" />
        <kbd>ESC</kbd>
      </label>
      <div id="command-results" class="command-results" role="listbox" aria-label="命令结果" bind:this={paletteResults}>
        {#if paletteItems.length}
          {#each paletteItems as item, index (item.id)}
            <button
              id={`command-option-${index}`}
              class:active={index === paletteIndex}
              role="option"
              tabindex="-1"
              aria-selected={index === paletteIndex}
              on:mousemove={() => (paletteIndex = index)}
              on:click={() => executePaletteItem(item)}
            >
              <span class="command-item-icon">
                {#if item.kind === 'tool'}<ResourceMark resource={tools.find(tool => tool.id === item.toolId) ?? { displayName: item.title }} size={32} />
                {:else}<Icon name={item.icon} size={17} />{/if}
              </span>
              <span><strong>{item.title}</strong><small>{item.subtitle}</small></span>
              {#if item.badge}<kbd>{item.badge}</kbd>{/if}
            </button>
          {/each}
        {:else}
          <div class="command-empty"><Icon name="search" size={22} /><strong>没有匹配结果</strong><small>尝试搜索 CLI 名称、命令或页面</small></div>
        {/if}
      </div>
      <footer><span><kbd>↑</kbd><kbd>↓</kbd> 选择</span><span><kbd>↵</kbd> 打开</span><span><kbd>⌘K</kbd> 关闭</span></footer>
    </div>
  </div>
{/if}

{#if confirmingExit}
  <div class="modal-backdrop" role="presentation">
    <div class="modal" role="dialog" aria-modal="true" aria-label="确认退出" use:focusDialog={{ trigger: dialogTrigger }}><span class="modal-icon warn">!</span><h2>仍有终端正在运行</h2><p>退出 CliCove 将终止所有活动的 CLI 进程。</p><div class="modal-actions"><button data-dialog-initial-focus on:click={() => (confirmingExit = false)}>继续使用</button><button class="danger-fill" on:click={() => api.forceExit()}>终止并退出</button></div></div>
  </div>
{:else if adapterTrustCandidate}
  <div class="modal-backdrop" role="presentation">
    <div class="modal adapter-trust-modal" role="dialog" aria-modal="true" aria-label="审查自定义适配器" use:focusDialog={{ trigger: dialogTrigger }}>
      <span class="modal-icon warn">!</span><h2>信任 {adapterTrustCandidate.displayName}</h2>
      <p>信任后，CliCove 可使用该文件中的版本、更新、诊断、登录和配置命令。信任仅对下方路径和当前内容指纹有效。</p>
      <dl><div><dt>文件</dt><dd>{adapterTrustCandidate.path}</dd></div><div><dt>SHA-256</dt><dd class="hash-value">{adapterTrustCandidate.fingerprint}</dd></div></dl>
      <div class="adapter-action-review">
        <strong>声明的命令</strong>
        {#if adapterTrustCandidate.actions.length}
          {#each adapterTrustCandidate.actions as action}<div><span>{action.name}</span><code>{action.command}</code></div>{/each}
        {:else}<p>没有额外管理命令，仅提供工具识别信息。</p>{/if}
      </div>
      <div class="modal-actions"><button data-dialog-initial-focus on:click={() => (adapterTrustCandidate = null)}>取消</button><button class="danger-fill" on:click={confirmAdapterTrust}>信任当前指纹</button></div>
    </div>
  </div>
{:else if updatePlan}
  <div class="modal-backdrop" role="presentation">
    <div class="modal" role="dialog" aria-modal="true" aria-label="确认更新" use:focusDialog={{ trigger: dialogTrigger }}>
      <span class="modal-icon"><Icon name="updates" size={24} /></span><h2>确认{updatePlan.operation === 'install' ? '安装' : '更新'} {updatePlan.toolName}</h2>
      <p>CliCove 将启动交互终端执行以下命令，并在退出后重新扫描验证版本：</p>
      <pre>{updatePlan.displayCommand}</pre>
      <dl><div><dt>当前版本</dt><dd>{updatePlan.currentVersion ?? '未知'}</dd></div><div><dt>目标</dt><dd>{updatePlan.targetVersion}</dd></div><div><dt>来源</dt><dd>{updatePlan.source}</dd></div></dl>
      <div class="modal-actions"><button data-dialog-initial-focus on:click={dismissUpdatePlan}>取消</button><button class="primary" on:click={confirmUpdate}>确认并执行</button></div>
    </div>
  </div>
{/if}

{#if error || notice}
  <div class="toast-region" aria-label="应用通知">
    {#if error}<div class="toast error-toast" role="alert"><span>!</span><div>{error}</div><button class="toast-dismiss" aria-label="关闭错误提示" on:click={() => (error = '')}>×</button></div>{/if}
    {#if notice}<div class:warning-toast={noticeTone === 'warning'} class="toast notice-toast" role="status"><span>{noticeTone === 'warning' ? '!' : '✓'}</span><div>{notice}</div><button class="toast-dismiss" aria-label="关闭通知" on:click={() => (notice = '')}>×</button></div>{/if}
  </div>
{/if}
