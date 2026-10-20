import { invoke } from '@tauri-apps/api/core';
import type {
  AppSettings,
  AuthenticationStatus,
  ConfigPreview,
  CliInformation,
  CustomAdapterInfo,
  InstallCandidate,
  LatestVersionInfo,
  DiagnosticReport,
  ResourceCatalog,
  SessionHistoryRecord,
  TerminalSession,
  ToolRecord,
  UpdatePlan,
  UpdateHistoryRecord,
} from './types';

export const api = {
  scanTools: (showSystem: boolean, showAllExecutables: boolean) =>
    invoke<ToolRecord[]>('scan_tools', { showSystem, showAllExecutables }),
  probeVersion: (toolId: string) => invoke<string>('probe_version', { toolId }),
  authenticationStatuses: () => invoke<AuthenticationStatus[]>('authentication_statuses'),
  diagnosticReports: () => invoke<DiagnosticReport[]>('diagnostic_reports'),
  checkAuthenticationStatus: (toolId: string) =>
    invoke<AuthenticationStatus>('check_authentication_status', { toolId }),
  runStructuredDiagnostic: (toolId: string) =>
    invoke<DiagnosticReport>('run_structured_diagnostic', { toolId }),
  listResourceCatalog: () => invoke<ResourceCatalog>('list_resource_catalog'),
  listCustomAdapters: () => invoke<CustomAdapterInfo[]>('list_custom_adapters'),
  openAdapterDirectory: () => invoke<void>('open_adapter_directory'),
  trustCustomAdapter: (path: string, fingerprint: string) =>
    invoke<CustomAdapterInfo[]>('trust_custom_adapter', { path, fingerprint }),
  revokeCustomAdapter: (path: string, fingerprint: string) =>
    invoke<CustomAdapterInfo[]>('revoke_custom_adapter', { path, fingerprint }),
  planUpdate: (toolId: string) => invoke<UpdatePlan>('plan_update', { toolId }),
  listInstallCandidates: () => invoke<InstallCandidate[]>('list_install_candidates'),
  listCliInformation: () => invoke<CliInformation[]>('list_cli_information'),
  openCliDocumentation: (serviceId: string) => invoke<void>('open_cli_documentation', { serviceId }),
  latestVersionChecks: () => invoke<LatestVersionInfo[]>('latest_version_checks'),
  checkLatestVersion: (toolId: string) =>
    invoke<LatestVersionInfo>('check_latest_version', { toolId }),
  checkInstallVersion: (candidateId: string) =>
    invoke<LatestVersionInfo>('check_install_version', { candidateId }),
  planInstall: (candidateId: string) => invoke<UpdatePlan>('plan_install', { candidateId }),
  cancelUpdatePlan: (planId: string) => invoke<boolean>('cancel_update_plan', { planId }),
  executeUpdate: (planId: string, cwd?: string) =>
    invoke<TerminalSession>('execute_update', { planId, cwd }),
  finalizeUpdate: (sessionId: string) =>
    invoke<UpdateHistoryRecord>('finalize_update', { sessionId }),
  listUpdateHistory: (limit = 100) =>
    invoke<UpdateHistoryRecord[]>('list_update_history', { limit }),
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
  terminalRename: (sessionId: string, title: string) =>
    invoke<TerminalSession>('terminal_rename', { sessionId, title }),
  sessionHistory: (limit = 50) => invoke<SessionHistoryRecord[]>('session_history', { limit }),
  configPreview: (toolId: string, pathIndex: number) =>
    invoke<ConfigPreview>('config_preview', { toolId, pathIndex }),
  openConfig: (toolId: string, pathIndex: number) =>
    invoke<void>('open_config', { toolId, pathIndex }),
  getSettings: () => invoke<AppSettings>('get_settings'),
  saveSettings: (settings: AppSettings) => invoke<void>('save_settings', { settings }),
  saveScanSettings: (showSystem: boolean, showAllExecutables: boolean) =>
    invoke<void>('save_scan_settings', { showSystem, showAllExecutables }),
  forceExit: () => invoke<void>('force_exit'),
};
