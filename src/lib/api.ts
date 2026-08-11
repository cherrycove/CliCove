import { invoke } from '@tauri-apps/api/core';
import type {
  AppSettings,
  ConfigPreview,
  TerminalSession,
  ToolRecord,
  UpdatePlan,
} from './types';

export const api = {
  scanTools: (showSystem: boolean, showAllExecutables: boolean) =>
    invoke<ToolRecord[]>('scan_tools', { showSystem, showAllExecutables }),
  probeVersion: (toolId: string) => invoke<string>('probe_version', { toolId }),
  planUpdate: (toolId: string) => invoke<UpdatePlan>('plan_update', { toolId }),
  executeUpdate: (planId: string, cwd?: string) =>
    invoke<TerminalSession>('execute_update', { planId, cwd }),
  spawnTerminal: (toolId: string, args: string[], cwd: string, cols = 110, rows = 32) =>
    invoke<TerminalSession>('spawn_terminal', {
      request: { toolId, args, cwd, cols, rows },
    }),
  runDiagnostic: (toolId: string) => invoke<TerminalSession>('run_diagnostic', { toolId }),
  runAuth: (toolId: string) => invoke<TerminalSession>('run_auth', { toolId }),
  showConfig: (toolId: string) => invoke<TerminalSession>('show_config', { toolId }),
  terminalWrite: (sessionId: string, data: string) =>
    invoke<void>('terminal_write', { sessionId, data }),
  terminalResize: (sessionId: string, cols: number, rows: number) =>
    invoke<void>('terminal_resize', { sessionId, cols, rows }),
  terminalTerminate: (sessionId: string) => invoke<void>('terminal_terminate', { sessionId }),
  terminalClose: (sessionId: string) => invoke<void>('terminal_close', { sessionId }),
  terminalSessions: () => invoke<TerminalSession[]>('terminal_sessions'),
  configPreview: (toolId: string, pathIndex: number) =>
    invoke<ConfigPreview>('config_preview', { toolId, pathIndex }),
  openConfig: (toolId: string, pathIndex: number) =>
    invoke<void>('open_config', { toolId, pathIndex }),
  getSettings: () => invoke<AppSettings>('get_settings'),
  saveSettings: (settings: AppSettings) => invoke<void>('save_settings', { settings }),
  forceExit: () => invoke<void>('force_exit'),
};
