<script lang="ts">
  import { onMount } from 'svelte';
  import Icon from './Icon.svelte';
  import BlockLoader from './BlockLoader.svelte';
  import { checkAdvice, checkTime, diagnosticView, sourceLabel } from './health';
  import type { AuthenticationStatus, DiagnosticReport, ToolRecord } from './types';

  export let tool: ToolRecord;
  export let authentication: AuthenticationStatus | undefined;
  export let report: DiagnosticReport | undefined;
  export let checking = false;
  export let checkingAuth = false;
  export let probing = false;
  export let scanning = false;
  export let oncheck: () => void;
  export let onauthcheck: () => void;
  export let onprobe: () => void;
  export let onrescan: () => void;
  export let onlogin: () => void;
  export let ondiagnose: () => void;

  let now = Date.now();
  let copyMessage = '';
  let copyTimer: ReturnType<typeof setTimeout> | undefined;
  $: state = diagnosticView(tool, authentication, report, now);
  $: auth = state.authentication;
  $: runtimeReady = tool.runtimeDependencies.every((item) => item.available);
  $: executableCheck = state.checks.find((item) => item.id === 'executable');
  $: versionCheck = state.checks.find((item) => item.id === 'version');
  $: busy = checking || checkingAuth || probing || scanning;

  onMount(() => {
    const timer = setInterval(() => (now = Date.now()), 15_000);
    return () => { clearInterval(timer); clearTimeout(copyTimer); };
  });

  async function copyPath() {
    try {
      await navigator.clipboard.writeText(tool.canonicalPath);
      copyMessage = '路径已复制';
    } catch {
      copyMessage = '复制失败，可选中路径手动复制';
    }
    clearTimeout(copyTimer);
    copyTimer = setTimeout(() => (copyMessage = ''), 3000);
  }
</script>

<section class="cli-status" aria-label="CLI 状态与诊断" aria-busy={busy}>
  <header class="status-heading">
    <h3>CLI 状态与诊断</h3>
    <button class="check-button" disabled={busy} aria-busy={checking} on:click={oncheck}>
      {#if checking}<BlockLoader inverse />{:else}<Icon name="activity" size={22} />{/if}
      {checking ? '正在检查…' : '运行检查'}
    </button>
  </header>

  <div class="status-overview tone-{checking ? 'muted' : state.tone}" role="status" aria-live="polite">
    <span class="status-emblem" aria-hidden="true">
      {#if checking}<BlockLoader size={36} />{:else}<Icon name="activity" size={36} />{/if}
    </span>
    <div><strong>{checking ? '正在检查本机 CLI' : state.title}</strong><p>{checking ? '检查可执行文件、运行依赖、版本响应和认证' : state.description}</p></div>
  </div>

  <ol class="installation-trail" aria-label="安装与运行状态">
    <li class="source-step">
      <span class="trail-node"><Icon name="package" size={16} /></span>
      <div class="trail-content">
        <div class="trail-label"><span>安装来源</span><small>本机扫描</small></div>
        <strong class="trail-value">{sourceLabel(tool.source)}</strong>
        {#if tool.source.packageName}<code class="supporting">{tool.source.packageName}</code>{/if}
      </div>
    </li>
    <li class="path-step">
      <span class="trail-node tone-{executableCheck?.status === 'pass' ? 'success' : executableCheck?.status === 'fail' ? 'danger' : 'muted'}"><Icon name="terminal" size={16} /></span>
      <div class="trail-content">
        <div class="trail-label"><span>实际路径</span><button class="trail-action" aria-label="复制实际路径" on:click={copyPath}><Icon name="copy" size={13} />复制</button></div>
        <code class="executable-path">{tool.canonicalPath}</code>
        <div class="supporting">命令 <code>{tool.aliases.join(' · ') || tool.name}</code></div>
        {#if tool.executable !== tool.canonicalPath}<div class="supporting">启动入口 <code class="executable-path">{tool.executable}</code></div>{/if}
        {#if copyMessage}<span class="copy-message" role="status">{copyMessage}</span>{/if}
      </div>
    </li>
    <li class="version-step">
      <span class="trail-node tone-{versionCheck?.status === 'pass' ? 'success' : versionCheck ? 'warning' : 'muted'}"><Icon name="command" size={16} /></span>
      <div class="trail-content">
        <div class="trail-label"><span>版本</span><button class="trail-action" disabled={busy || !runtimeReady} aria-busy={probing} on:click={onprobe}>{#if probing}<BlockLoader size={18} />{:else}<Icon name="refresh" size={18} />{/if}{probing ? '探测中…' : '刷新版本'}</button></div>
        <strong class="trail-value version-value">{tool.version ?? '尚未探测'}</strong>
        <span class="supporting">{versionCheck?.status === 'pass' ? '已确认版本响应' : versionCheck ? '未能确认版本响应，见下方原因' : tool.version ? '扫描记录 · 运行检查可确认当前响应' : '运行检查以获取当前版本'}</span>
      </div>
    </li>
    <li class="authentication-step tone-{auth.tone}">
      <span class="trail-node tone-{auth.tone}"><Icon name="shield" size={16} /></span>
      <div class="trail-content">
        <div class="trail-label"><span>认证</span>{#if tool.capabilities.auth}<button class="trail-action" disabled={busy || !runtimeReady} aria-busy={checkingAuth} on:click={onauthcheck}>{#if checkingAuth}<BlockLoader size={18} />{/if}{checkingAuth ? '检查中…' : '检查认证'}</button>{/if}</div>
        <strong class="trail-value tone-{auth.tone}">{auth.label}</strong>
        <p class="supporting">{auth.message}</p>
        {#if auth.current}
          <div class="auth-fields">
            {#if auth.current.method}<span>方式 <b>{auth.current.method}</b></span>{/if}
            {#if auth.current.expiresInMinutes != null}<span>剩余约 <b>{auth.current.expiresInMinutes} 分钟</b></span>{/if}
            {#each auth.current.fields as field}<span>{field.label} <b>{field.value}</b></span>{/each}
          </div>
          <small class="checked-time">检查于 {checkTime(auth.current.checkedAt)}</small>
        {/if}
      </div>
    </li>
  </ol>

  {#if state.issues.length}
    <section class="issue-section" aria-label="问题原因与处理建议">
      <h4>需要处理 <span>{state.issues.length}</span></h4>
      {#each state.issues as issue}
        <div class="issue-row tone-{issue.status === 'fail' ? 'danger' : 'warning'}">
          <span class="issue-mark" aria-hidden="true">{issue.status === 'fail' ? '!' : '·'}</span>
          <div><strong>{issue.label}</strong><p>{issue.message}</p><p class="issue-advice">{checkAdvice(issue)}</p>
            {#if issue.id === 'authentication' && issue.status === 'fail' && tool.capabilities.auth}<button class="trail-action" disabled={!runtimeReady || busy} on:click={onlogin}>前往登录<Icon name="chevron-right" size={13} /></button>{/if}
            {#if issue.id === 'dependencies' || issue.id === 'executable'}<button class="trail-action" disabled={busy} aria-busy={scanning} on:click={onrescan}>{#if scanning}<BlockLoader size={18} />正在扫描…{:else}重新扫描<Icon name="chevron-right" size={13} />{/if}</button>{/if}
          </div>
        </div>
      {/each}
    </section>
  {/if}

  {#if state.current}
    <div class="report-footer">
      <span>检查于 {checkTime(state.current.checkedAt)} · {state.current.durationMs} ms</span>
      {#if state.passed.length}
        <details class="check-details"><summary>{state.passed.length} 项检查通过<Icon name="chevron-right" size={13} /></summary>
          {#each state.passed as check}<div class="passed-check"><Icon name="check" size={14} /><div><strong>{check.label}</strong><p>{check.message}</p></div></div>{/each}
        </details>
      {/if}
    </div>
  {:else if report}
    <details class="historical-report"><summary>上次检查 · {checkTime(report.checkedAt)} · 已失效<Icon name="chevron-right" size={13} /></summary>
      <p>{state.stale}</p>
      {#each report.checks as check}<div><strong>{check.label}</strong><p>{check.message}</p></div>{/each}
    </details>
  {/if}

  {#if tool.capabilities.diagnose}<button class="official-diagnostic" disabled={busy || !runtimeReady} on:click={ondiagnose}><Icon name="terminal" size={14} />在终端运行官方诊断<Icon name="chevron-right" size={13} /></button>{/if}
  <p class="check-privacy">仅做只读检查，凭据由 CLI 管理。</p>
</section>

<style>
  .cli-status { --signal: var(--text-tertiary); min-width: 0; margin: 0; padding: 23px 24px 20px; background: var(--white); }
  .tone-success { --signal: var(--success); }
  .tone-warning { --signal: var(--warning); }
  .tone-danger { --signal: var(--danger); }
  .tone-muted { --signal: var(--text-tertiary); }
  .status-heading, .trail-label { display: flex; align-items: center; justify-content: space-between; gap: 10px; }
  h3 { margin: 0; font-size: 15px; font-weight: 650; letter-spacing: -.2px; }
  .check-button { min-height: 34px; padding: 7px 10px; border: 0; background: var(--text-primary); color: var(--white); font-size: 12px; border-radius: 4px; }
  .check-button:hover:not(:disabled) { background: #424b3c; }
  .check-button :global(.ui-icon) { --icon-accent: #f2cb4b; --icon-secondary: #fffef9; --icon-cutout: #232722; }
  .status-overview { display: flex; align-items: center; gap: 12px; margin: 18px 0; padding: 0; background: transparent; }
  .status-emblem { display: flex; flex: none; color: var(--signal); }
  .status-emblem :global(.ui-icon) { --icon-accent: currentColor; --icon-secondary: currentColor; --icon-tertiary: currentColor; }
  .status-overview strong { color: var(--signal); font-size: 15px; font-weight: 620; }
  .status-overview p { margin: 4px 0 0; color: var(--text-secondary); font-size: 12px; line-height: 1.65; }
  .installation-trail { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 3px; list-style: none; margin: 0; padding: 0; border-radius: 5px; overflow: hidden; }
  .installation-trail > li { position: relative; min-width: 0; min-height: 148px; padding: 16px 14px; background: #eeeee6; }
  .installation-trail > .source-step { background: var(--brand-yellow); }
  .installation-trail > .authentication-step.tone-success { background: #e8f0e7; }
  .installation-trail > .authentication-step.tone-warning { background: #f4e8cb; }
  .installation-trail > .authentication-step.tone-danger { background: #f7e3dc; }
  .trail-node { display: grid; place-items: center; position: absolute; top: 20px; left: 14px; width: 15px; height: 16px; color: var(--signal); }
  .trail-node :global(.ui-icon) { width: 15px; height: 15px; --icon-accent: currentColor; --icon-secondary: currentColor; --icon-tertiary: currentColor; }
  .source-step .trail-node { color: #63521b; }
  .trail-content { min-width: 0; }
  .trail-label { min-height: 25px; margin-bottom: 14px; padding-left: 23px; gap: 4px; flex-wrap: wrap; }
  .trail-label > span { font-size: 12px; color: var(--text-secondary); white-space: nowrap; }
  .trail-label > small { font-size: 12px; color: #63521b; }
  .trail-value { display: block; color: var(--text-primary); font-size: 18px; font-weight: 600; line-height: 1.4; overflow-wrap: anywhere; }
  .trail-value[class*='tone-'] { color: var(--signal); }
  .version-value { font: 600 15px/1.5 ui-monospace, SFMono-Regular, Menlo, monospace; }
  .executable-path { display: block; color: var(--text-secondary); font: 12px/1.75 ui-monospace, SFMono-Regular, Menlo, monospace; overflow-wrap: anywhere; user-select: text; }
  .supporting { display: block; margin: 7px 0 0; color: var(--text-secondary); font-size: 12px; line-height: 1.65; overflow-wrap: anywhere; }
  .source-step .supporting { color: #63521b; }
  code.supporting, .supporting code { font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-size: 12px; }
  .trail-action { display: inline-flex; align-items: center; gap: 4px; min-height: 25px; padding: 2px 0; border: 0; background: transparent; color: var(--accent); font-size: 12px; box-shadow: none; white-space: nowrap; }
  .trail-action:hover:not(:disabled) { background: transparent; color: var(--accent-hover); text-decoration: underline; }
  .auth-fields { display: flex; flex-wrap: wrap; gap: 5px 12px; margin-top: 8px; font-size: 12px; color: var(--text-tertiary); }
  .auth-fields b { color: var(--text-secondary); font-weight: 500; overflow-wrap: anywhere; }
  .checked-time { display: block; margin-top: 7px; font-size: 12px; color: var(--text-tertiary); }
  .copy-message { display: block; margin-top: 5px; font-size: 12px; color: var(--text-secondary); }
  .issue-section { margin-top: 18px; padding: 17px; border: 0; border-radius: 4px; background: #f7efdc; }
  h4 { display: flex; align-items: center; gap: 8px; margin: 0 0 14px; color: var(--text-primary); font-size: 13px; font-weight: 600; }
  h4 > span { font-size: 12px; color: var(--text-tertiary); font-variant-numeric: tabular-nums; }
  .issue-row { display: grid; grid-template-columns: 20px minmax(0, 1fr); gap: 8px; margin-top: 15px; }
  .issue-mark { display: grid; place-items: center; align-self: start; width: 18px; height: 18px; border: 1px solid currentColor; border-radius: 3px; color: var(--signal); font-size: 13px; font-weight: 700; }
  .issue-row strong { color: var(--signal); font-size: 13px; font-weight: 600; }
  .issue-row p { margin: 5px 0; color: var(--text-secondary); font-size: 12px; line-height: 1.65; overflow-wrap: anywhere; }
  .issue-row .issue-advice { color: var(--text-tertiary); }
  .report-footer { margin-top: 18px; padding-top: 13px; border-top: 1px solid var(--border); }
  .report-footer > span, .check-privacy { color: var(--text-tertiary); font-size: 12px; line-height: 1.6; }
  details { margin-top: 10px; color: var(--text-secondary); }
  summary { display: flex; align-items: center; justify-content: space-between; gap: 8px; min-height: 30px; font-size: 12px; cursor: pointer; list-style: none; }
  summary::-webkit-details-marker { display: none; }
  summary:hover { color: var(--text-primary); }
  details[open] > summary :global(.ui-icon) { transform: rotate(90deg); }
  .passed-check { display: flex; gap: 9px; margin: 12px 0; font-size: 12px; }
  .passed-check > :global(.ui-icon) { flex: none; margin-top: 2px; color: var(--success); }
  .passed-check strong { font-weight: 500; }
  .passed-check p, .historical-report p { margin: 4px 0; color: var(--text-tertiary); font-size: 12px; line-height: 1.6; overflow-wrap: anywhere; }
  .historical-report { padding-top: 10px; border-top: 1px solid var(--border); }
  .historical-report > div { margin-top: 12px; font-size: 12px; }
  .official-diagnostic { display: flex; width: 100%; justify-content: flex-start; gap: 8px; margin-top: 16px; min-height: 32px; padding: 7px 0; border: 0; background: transparent; color: var(--text-secondary); font-size: 12px; }
  .official-diagnostic > :global(.ui-icon:last-child) { margin-left: auto; }
  .check-privacy { margin: 10px 0 0; }
  @media (max-width: 1200px) {
    .cli-status { padding: 22px 20px 20px; }
    .installation-trail > li { padding: 15px 12px; }
    .trail-node { left: 12px; top: 19px; }
    .trail-value { font-size: 16px; }
    .version-value { font-size: 14px; }
  }
</style>
