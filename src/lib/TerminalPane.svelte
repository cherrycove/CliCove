<script lang="ts">
  import { onMount, createEventDispatcher } from 'svelte';
  import { listen } from '@tauri-apps/api/event';
  import { Terminal } from '@xterm/xterm';
  import { FitAddon } from '@xterm/addon-fit';
  import { api } from './api';
  import Icon from './Icon.svelte';
  import type { TerminalSession } from './types';

  export let session: TerminalSession;
  export let active = false;

  const dispatch = createEventDispatcher<{
    close: string;
    status: TerminalSession;
    restart: TerminalSession;
  }>();
  let container: HTMLDivElement;
  let exited = session.status === 'exited';
  let copied = false;
  let renaming = false;
  let draftTitle = session.title;
  let exitCode = session.exitCode;
  let terminalRef: Terminal | undefined;
  let selectionAvailable = false;

  onMount(() => {
    const terminal = new Terminal({
      allowProposedApi: false,
      cursorBlink: true,
      cursorStyle: 'bar',
      fontFamily: '"SFMono-Regular", Consolas, "Liberation Mono", monospace',
      fontSize: 13,
      lineHeight: 1.35,
      scrollback: 10_000,
      theme: {
        background: '#252c23',
        foreground: '#e4eadc',
        cursor: '#f2cb4b',
        selectionBackground: '#8eabe24a',
        black: '#111722',
        red: '#ff6b7a',
        green: '#58d39b',
        yellow: '#e7b75b',
        blue: '#6e8bff',
        magenta: '#b695ff',
        cyan: '#63c7e6',
        white: '#d5dce8',
      },
    });
    const fit = new FitAddon();
    terminal.loadAddon(fit);
    terminal.open(container);
    terminalRef = terminal;
    terminal.write(`\x1b[38;2;110;139;255mCliCove\x1b[0m · ${session.title}\r\n`);
    terminal.write(`\x1b[38;2;127;139;156m${session.cwd}\x1b[0m\r\n\r\n`);

    const resize = () => {
      if (!active || !container.clientWidth || !container.clientHeight) return;
      fit.fit();
      void api.terminalResize(session.id, terminal.cols, terminal.rows);
    };
    const observer = new ResizeObserver(resize);
    observer.observe(container);
    const dataDisposable = terminal.onData((data) => {
      if (!exited) void api.terminalWrite(session.id, data);
    });
    const selectionDisposable = terminal.onSelectionChange(() => {
      selectionAvailable = terminal.hasSelection();
    });

    let unlistenOutput: (() => void) | undefined;
    let unlistenExit: (() => void) | undefined;
    void listen<{ sessionId: string; data: string }>('terminal-output', ({ payload }) => {
      if (payload.sessionId === session.id) terminal.write(payload.data);
    }).then((fn) => (unlistenOutput = fn));
    void listen<{ sessionId: string; exitCode?: number }>('terminal-exit', ({ payload }) => {
      if (payload.sessionId !== session.id) return;
      exited = true;
      exitCode = payload.exitCode;
      session = {
        ...session,
        status: 'exited',
        exitCode: payload.exitCode,
        endedAt: new Date().toISOString(),
      };
      terminal.write(
        `\r\n\x1b[38;2;127;139;156m[进程已退出${payload.exitCode == null ? '' : ` · code ${payload.exitCode}`} ]\x1b[0m\r\n`,
      );
      dispatch('status', session);
    }).then((fn) => (unlistenExit = fn));

    requestAnimationFrame(resize);
    return () => {
      observer.disconnect();
      dataDisposable.dispose();
      selectionDisposable.dispose();
      unlistenOutput?.();
      unlistenExit?.();
      terminal.dispose();
      terminalRef = undefined;
    };
  });

  async function terminate() {
    if (!exited) await api.terminalTerminate(session.id);
  }

  async function close() {
    await api.terminalClose(session.id);
    dispatch('close', session.id);
  }

  async function copySelection() {
    const selection = terminalRef?.getSelection();
    if (!selection) return;
    await navigator.clipboard.writeText(selection);
    copied = true;
    window.setTimeout(() => (copied = false), 1800);
  }

  async function rename() {
    if (!draftTitle.trim()) return;
    session = await api.terminalRename(session.id, draftTitle);
    renaming = false;
    dispatch('status', session);
  }

  function restart() {
    dispatch('restart', session);
  }

  function fitAndFocus() {
    terminalRef?.focus();
  }
</script>

<section class:active class="terminal-pane" aria-label={`${session.title} 终端`}>
  <header>
    <div>
      <span class:stopped={exited} class="status-dot"></span>
      <span class="session-kind">{session.kind === 'management' ? '管理' : session.kind === 'diagnostic' ? '诊断' : session.kind === 'auth' ? '登录' : session.kind === 'config' ? '配置' : '交互'}</span>
      {#if renaming}
        <input class="session-title-input" bind:value={draftTitle} maxlength="80" on:keydown={(event) => event.key === 'Enter' && rename()} on:blur={rename} />
      {:else}
        <button class="session-title-button" title="点击重命名会话" on:click={() => (renaming = true)}>{session.title}</button>
      {/if}
      <span class="session-path">{session.cwd}</span>
    </div>
    <div class="terminal-actions">
      <button class="ghost" disabled={!selectionAvailable} title="复制当前选区" on:click={copySelection}><Icon name={copied ? 'check' : 'copy'} size={14} />{copied ? '已复制' : '复制'}</button>
      <button class="ghost" title="聚焦终端" on:click={fitAndFocus}><Icon name="focus" size={14} />聚焦</button>
      {#if !exited}
        <button class="ghost danger" on:click={terminate}><Icon name="stop" size={13} />终止</button>
      {:else}
        <span class:success={exitCode === 0} class:error={exitCode !== 0 && exitCode != null} class="exit-badge">{exitCode == null ? '已退出' : `exit ${exitCode}`}</span>
        <button class="ghost" on:click={restart}><Icon name="restart" size={14} />重新运行</button>
      {/if}
      <button class="ghost icon-only" title="关闭标签" aria-label="关闭标签" on:click={close}><Icon name="close" size={14} /></button>
    </div>
  </header>
  <div class="terminal-mount" bind:this={container}></div>
</section>
