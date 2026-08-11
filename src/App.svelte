<script lang="ts">
  import { onMount } from 'svelte';
  import { listen } from '@tauri-apps/api/event';
  import { api } from './lib/api';
  import { filterTools, splitArguments } from './lib/tooling';
  import TerminalPane from './lib/TerminalPane.svelte';
  import type {
    AppSettings,
    ConfigPreview,
    TerminalSession,
    ToolCategory,
    ToolRecord,
    UpdatePlan,
  } from './lib/types';

  type View = 'tools' | 'terminals' | 'updates' | 'settings';

  const categoryNames: Record<ToolCategory, string> = {
    'ai-agent': 'AI / Agent',
    collaboration: '协作与项目',
    runtime: '语言与运行时',
    'cloud-container': '云与容器',
    android: 'Android',
    'developer-tool': '开发工具',
    system: '系统工具',
    other: '其他工具',
  };

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
  let error = '';
  let notice = '';
  let settings = defaultSettings;
  let sessions: TerminalSession[] = [];
  let activeSessionId = '';
  let argsText = '';
  let toolListElement: HTMLElement | undefined;
  let updatePlan: UpdatePlan | null = null;
  let configPreview: ConfigPreview | null = null;
  let confirmingExit = false;

  $: selected = tools.find((tool) => tool.id === selectedId);
  $: filtered = filterTools(tools, query, category);
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

  onMount(() => {
    let unlistenExit: (() => void) | undefined;
    void listen('exit-confirmation-requested', () => (confirmingExit = true)).then(
      (fn) => (unlistenExit = fn),
    );
    void initialize();
    return () => unlistenExit?.();
  });

  async function initialize() {
    try {
      settings = await api.getSettings();
      sessions = await api.terminalSessions();
      activeSessionId = sessions.at(-1)?.id ?? '';
      await scan();
    } catch (cause) {
      showError(cause);
    }
  }

  async function scan() {
    scanning = true;
    error = '';
    configPreview = null;
    try {
      tools = await api.scanTools(
        settings.showSystemTools,
        settings.showAllExecutables,
      );
      if (!tools.some((tool) => tool.id === selectedId)) {
        selectedId = tools[0]?.id ?? '';
      }
    } catch (cause) {
      showError(cause);
    } finally {
      scanning = false;
    }
  }

  async function probe(tool: ToolRecord) {
    try {
      const version = await api.probeVersion(tool.id);
      tools = tools.map((item) => (item.id === tool.id ? { ...item, version } : item));
      showNotice('版本信息已刷新');
    } catch (cause) {
      showError(cause);
    }
  }

  async function launch(tool: ToolRecord, args: string[] = parseArgs()) {
    try {
      const session = await api.spawnTerminal(
        tool.id,
        args,
        settings.defaultWorkingDirectory,
      );
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

  function addSession(session: TerminalSession) {
    sessions = [...sessions.filter((item) => item.id !== session.id), session];
    activeSessionId = session.id;
    view = 'terminals';
  }

  async function createUpdatePlan(tool: ToolRecord) {
    try {
      updatePlan = await api.planUpdate(tool.id);
    } catch (cause) {
      showError(cause);
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

  async function previewConfig(tool: ToolRecord, index: number) {
    try {
      configPreview = await api.configPreview(tool.id, index);
    } catch (cause) {
      showError(cause);
    }
  }

  async function saveAppSettings() {
    try {
      await api.saveSettings(settings);
      showNotice('设置已保存');
      await scan();
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

  function parseArgs() {
    return splitArguments(argsText);
  }

  function scrollToolList(direction: -1 | 1) {
    if (!toolListElement) return;
    const distance = Math.max(toolListElement.clientHeight * 0.8, 240);
    toolListElement.scrollBy({ top: distance * direction, behavior: 'smooth' });
  }

  function showError(cause: unknown) {
    error = cause instanceof Error ? cause.message : String(cause);
    window.setTimeout(() => (error = ''), 7000);
  }

  function showNotice(message: string) {
    notice = message;
    window.setTimeout(() => (notice = ''), 2500);
  }
</script>

<div class="app-shell">
  <aside class="sidebar">
    <div class="brand">
      <span class="brand-mark"><i></i><i></i><i></i></span>
      <div><strong>CLIHub</strong><small>LOCAL CONSOLE</small></div>
    </div>

    <nav aria-label="主导航">
      <button class:active={view === 'tools'} on:click={() => (view = 'tools')}>
        <span class="nav-icon">⌘</span><span>工具</span><em>{filtered.length}</em>
      </button>
      <button class:active={view === 'updates'} on:click={() => (view = 'updates')}>
        <span class="nav-icon">↟</span><span>更新</span><em>{updatable.length}</em>
      </button>
      <button class:active={view === 'terminals'} on:click={() => (view = 'terminals')}>
        <span class="nav-icon">›_</span><span>终端</span><em>{sessions.length}</em>
      </button>
      <button class:active={view === 'settings'} on:click={() => (view = 'settings')}>
        <span class="nav-icon">◇</span><span>设置</span>
      </button>
    </nav>

    <div class="sidebar-foot">
      <span class="local-dot"></span>
      <div><strong>纯本地模式</strong><small>凭据由各 CLI 管理</small></div>
    </div>
  </aside>

  <main>
    <header class="topbar">
      <div>
        <h1>{view === 'tools' ? '本地工具' : view === 'updates' ? '更新中心' : view === 'terminals' ? '终端' : '设置'}</h1>
        <p>{view === 'tools' ? '发现、检查并运行当前登录环境中的 CLI' : view === 'updates' ? '所有更新都将在确认后使用官方机制执行' : view === 'terminals' ? '交互内容仅存在内存中' : 'CLIHub 本地行为与扫描范围'}</p>
      </div>
      {#if view === 'tools' || view === 'updates'}
        <button class="scan-button" disabled={scanning} on:click={scan}>
          <span class:spinning={scanning}>↻</span>{scanning ? '扫描中' : '重新扫描'}
        </button>
      {/if}
    </header>

    {#if view === 'tools'}
      <div class="toolbar">
        <label class="search"><span>⌕</span><input bind:value={query} placeholder="搜索命令、别名或来源" /></label>
        <select bind:value={category} aria-label="按分类过滤">
          <option value="all">全部分类</option>
          {#each Object.entries(categoryNames) as [value, label]}
            <option value={value}>{label}</option>
          {/each}
        </select>
        <div class="list-scroll-controls" aria-label="工具列表滚动控制">
          <button title="向上翻页" aria-label="向上翻页" on:click={() => scrollToolList(-1)}>↑</button>
          <button title="向下翻页" aria-label="向下翻页" on:click={() => scrollToolList(1)}>↓</button>
        </div>
      </div>

      <div class="tool-layout">
        <section bind:this={toolListElement} class="tool-list" aria-label="CLI 工具列表">
          {#if scanning && tools.length === 0}
            <div class="loading-list">{#each Array(8) as _}<span></span>{/each}</div>
          {:else if filtered.length === 0}
            <div class="empty"><b>未找到匹配工具</b><span>调整搜索条件或扫描范围</span></div>
          {:else}
            {#each filtered as tool (tool.id)}
              <button class:selected={tool.id === selectedId} class="tool-row" on:click={() => { selectedId = tool.id; configPreview = null; }}>
                <span class="tool-glyph category-{tool.category}">{tool.name.slice(0, 2).toUpperCase()}</span>
                <span class="tool-main"><strong>{tool.displayName}</strong><small>{tool.aliases.join(' · ')}</small></span>
                <span class="tool-meta"><em>{tool.version?.split(' · ')[0] ?? '未探测'}</em><small>{tool.source.label}</small></span>
                <span class="chevron">›</span>
              </button>
            {/each}
          {/if}
        </section>

        <aside class="detail-panel">
          {#if selected}
            <div class="detail-title">
              <span class="tool-glyph large category-{selected.category}">{selected.name.slice(0, 2).toUpperCase()}</span>
              <div><h2>{selected.displayName}</h2><p>{categoryNames[selected.category]}</p></div>
            </div>
            <div class="detail-actions">
              <button class="primary" on:click={() => launch(selected)}>打开终端</button>
              {#if selected.capabilities.update}<button on:click={() => createUpdatePlan(selected)}>更新</button>{/if}
              {#if selected.capabilities.auth}<button on:click={() => runAction('auth', selected)}>登录</button>{/if}
              {#if selected.capabilities.diagnose}<button on:click={() => runAction('diagnostic', selected)}>诊断</button>{/if}
            </div>

            <dl class="facts">
              <div><dt>版本</dt><dd>{selected.version ?? '尚未探测'} <button class="link" on:click={() => probe(selected)}>刷新</button></dd></div>
              <div><dt>安装来源</dt><dd>{selected.source.label}{selected.source.packageName ? ` · ${selected.source.packageName}` : ''}</dd></div>
              <div><dt>可执行文件</dt><dd class="path">{selected.canonicalPath}</dd></div>
              <div><dt>别名</dt><dd>{selected.aliases.join(', ')}</dd></div>
              {#if selected.relatedCommands.length > selected.aliases.length}
                <div><dt>包含命令</dt><dd>{selected.relatedCommands.length} 个可执行入口</dd></div>
              {/if}
            </dl>

            <div class="launch-options">
              <label for="launch-args">启动参数 <small>每行一个参数，不经过 Shell 解析</small></label>
              <textarea id="launch-args" bind:value={argsText} rows="3" placeholder="例如：&#10;--version"></textarea>
              <label for="working-directory">工作目录</label>
              <input id="working-directory" bind:value={settings.defaultWorkingDirectory} />
            </div>

            {#if selected.capabilities.config}
              <div class="config-section">
                <h3>配置</h3>
                {#if selected.configPaths.length}
                  {#each selected.configPaths as path, index}
                    <div class="config-path"><code>{path}</code><span><button class="link" on:click={() => previewConfig(selected, index)}>脱敏预览</button><button class="link" on:click={() => api.openConfig(selected.id, index)}>外部打开</button></span></div>
                  {/each}
                {:else}
                  <button class="secondary wide" on:click={() => runAction('config', selected)}>通过官方命令显示配置</button>
                {/if}
              </div>
            {/if}

            {#if configPreview}
              <div class="config-preview"><header><span>{configPreview.path}</span><button on:click={() => (configPreview = null)}>×</button></header>{#if configPreview.exists}<pre>{configPreview.content || '（空文件）'}</pre>{:else}<p>配置文件尚未创建。</p>{/if}</div>
            {/if}
          {:else}
            <div class="empty"><b>选择一个 CLI</b><span>查看版本、来源与可用操作</span></div>
          {/if}
        </aside>
      </div>
    {:else if view === 'updates'}
      <section class="page-card updates-page">
        <div class="page-intro"><span>↟</span><div><h2>可管理更新</h2><p>CLIHub 不托管安装包，所有命令使用官方更新器或原包管理器。</p></div></div>
        {#each updatable as tool}
          <div class="update-row">
            <span class="tool-glyph category-{tool.category}">{tool.name.slice(0, 2).toUpperCase()}</span>
            <div><strong>{tool.displayName}</strong><small>{tool.version ?? tool.source.packageVersion ?? '当前版本未知'} · {tool.source.label}</small></div>
            <button on:click={() => createUpdatePlan(tool)}>生成更新计划</button>
          </div>
        {/each}
      </section>
    {:else if view === 'terminals'}
      <section class="terminal-workspace">
        {#if sessions.length}
          <div class="terminal-tabs">
            {#each sessions as session}
              <button class:active={session.id === activeSessionId} on:click={() => (activeSessionId = session.id)}>
                <span class:stopped={session.status === 'exited'}></span>{session.title}
              </button>
            {/each}
          </div>
          <div class="terminal-stack">
            {#each sessions as session (session.id)}
              <TerminalPane {session} active={session.id === activeSessionId} on:close={closeSession} on:status={updateSession} />
            {/each}
          </div>
        {:else}
          <div class="empty terminal-empty"><span>›_</span><b>还没有终端会话</b><p>从工具详情选择“打开终端”开始。</p><button on:click={() => (view = 'tools')}>浏览工具</button></div>
        {/if}
      </section>
    {:else}
      <section class="page-card settings-page">
        <div class="setting-group"><div><h3>默认工作目录</h3><p>新终端和管理命令默认在此目录启动。</p></div><input bind:value={settings.defaultWorkingDirectory} /></div>
        <div class="setting-group"><div><h3>显示系统工具</h3><p>在主清单中包含 macOS 或 Windows 自带命令。</p></div><label class="switch"><input type="checkbox" bind:checked={settings.showSystemTools} /><span></span></label></div>
        <div class="setting-group"><div><h3>显示全部可执行文件</h3><p>关闭时按 CLI 产品或安装包聚合；开启后显示 PATH 中每个可执行入口。</p></div><label class="switch"><input type="checkbox" bind:checked={settings.showAllExecutables} /><span></span></label></div>
        <div class="setting-group"><div><h3>驻留系统托盘</h3><p>默认关闭以减少后台内存占用；首版设置将在下次启动生效。</p></div><label class="switch"><input type="checkbox" bind:checked={settings.trayEnabled} /><span></span></label></div>
        <div class="privacy-note"><span>◈</span><div><strong>本地与隐私</strong><p>CLIHub 不读取 CLI 登录凭据，不保存完整终端输出，也不会在后台自动联网检查更新。</p></div></div>
        <button class="primary save-settings" on:click={saveAppSettings}>保存设置</button>
      </section>
    {/if}
  </main>
</div>

{#if updatePlan}
  <div class="modal-backdrop" role="presentation">
    <div class="modal" role="dialog" aria-modal="true" aria-label="确认更新">
      <span class="modal-icon">↟</span><h2>确认更新 {updatePlan.toolName}</h2>
      <p>CLIHub 将启动交互终端并执行以下官方更新命令：</p>
      <pre>{updatePlan.displayCommand}</pre>
      <dl><div><dt>当前版本</dt><dd>{updatePlan.currentVersion ?? '未知'}</dd></div><div><dt>目标</dt><dd>{updatePlan.targetVersion}</dd></div><div><dt>来源</dt><dd>{updatePlan.source}</dd></div></dl>
      <div class="modal-actions"><button on:click={() => (updatePlan = null)}>取消</button><button class="primary" on:click={confirmUpdate}>确认并打开终端</button></div>
    </div>
  </div>
{/if}

{#if confirmingExit}
  <div class="modal-backdrop">
    <section class="modal"><span class="modal-icon warn">!</span><h2>仍有终端正在运行</h2><p>退出 CLIHub 将终止所有活动的 CLI 进程。</p><div class="modal-actions"><button on:click={() => (confirmingExit = false)}>继续使用</button><button class="danger-fill" on:click={() => api.forceExit()}>终止并退出</button></div></section>
  </div>
{/if}

{#if error}<div class="toast error-toast"><span>!</span>{error}</div>{/if}
{#if notice}<div class="toast notice-toast"><span>✓</span>{notice}</div>{/if}
