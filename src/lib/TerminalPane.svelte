<script lang="ts">
  import { onMount, createEventDispatcher } from 'svelte';
  import { listen } from '@tauri-apps/api/event';
  import { Terminal } from '@xterm/xterm';
  import { FitAddon } from '@xterm/addon-fit';
  import { api } from './api';
  import type { TerminalSession } from './types';

  export let session: TerminalSession;
  export let active = false;

  const dispatch = createEventDispatcher<{ close: string; status: TerminalSession }>();
  let container: HTMLDivElement;
  let exited = session.status === 'exited';

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
        background: '#090d12',
        foreground: '#dbe5ee',
        cursor: '#78d9c0',
        selectionBackground: '#255b6555',
        black: '#10171f',
        red: '#ff7b83',
        green: '#73daca',
        yellow: '#e0af68',
        blue: '#7aa2f7',
        magenta: '#bb9af7',
        cyan: '#7dcfff',
        white: '#c0caf5',
      },
    });
    const fit = new FitAddon();
    terminal.loadAddon(fit);
    terminal.open(container);
    terminal.write(`\x1b[38;2;120;217;192mCLIHub\x1b[0m · ${session.title}\r\n`);
    terminal.write(`\x1b[38;2;115;128;143m${session.cwd}\x1b[0m\r\n\r\n`);

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

    let unlistenOutput: (() => void) | undefined;
    let unlistenExit: (() => void) | undefined;
    void listen<{ sessionId: string; data: string }>('terminal-output', ({ payload }) => {
      if (payload.sessionId === session.id) terminal.write(payload.data);
    }).then((fn) => (unlistenOutput = fn));
    void listen<{ sessionId: string; exitCode?: number }>('terminal-exit', ({ payload }) => {
      if (payload.sessionId !== session.id) return;
      exited = true;
      session = { ...session, status: 'exited' };
      terminal.write(
        `\r\n\x1b[38;2;115;128;143m[进程已退出${payload.exitCode == null ? '' : ` · code ${payload.exitCode}`} ]\x1b[0m\r\n`,
      );
      dispatch('status', session);
    }).then((fn) => (unlistenExit = fn));

    requestAnimationFrame(resize);
    return () => {
      observer.disconnect();
      dataDisposable.dispose();
      unlistenOutput?.();
      unlistenExit?.();
      terminal.dispose();
    };
  });

  async function terminate() {
    if (!exited) await api.terminalTerminate(session.id);
  }

  async function close() {
    await api.terminalClose(session.id);
    dispatch('close', session.id);
  }
</script>

<section class:active class="terminal-pane" aria-label={`${session.title} 终端`}>
  <header>
    <div>
      <span class:stopped={exited} class="status-dot"></span>
      <strong>{session.title}</strong>
      <span class="session-path">{session.cwd}</span>
    </div>
    <div class="terminal-actions">
      {#if !exited}
        <button class="ghost danger" on:click={terminate}>终止</button>
      {/if}
      <button class="ghost" on:click={close}>关闭标签</button>
    </div>
  </header>
  <div class="terminal-mount" bind:this={container}></div>
</section>

