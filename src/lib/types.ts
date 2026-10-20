export type ToolCategory =
  | 'ai-agent'
  | 'collaboration'
  | 'cloud-service'
  | 'observability'
  | 'infrastructure'
  | 'runtime'
  | 'android'
  | 'developer-tool'
  | 'system'
  | 'other';

export interface ToolSource {
  kind: string;
  label: string;
  packageName?: string;
  packageVersion?: string;
  scope: string;
}

export interface ToolCapabilities {
  terminal: boolean;
  versionProbe: boolean;
  update: boolean;
  diagnose: boolean;
  auth: boolean;
  config: boolean;
}

export interface RuntimeDependency {
  name: string;
  executable?: string;
  available: boolean;
}

export interface ToolRecord {
  id: string;
  name: string;
  displayName: string;
  executable: string;
  canonicalPath: string;
  aliases: string[];
  relatedCommands: string[];
  version?: string;
  source: ToolSource;
  category: ToolCategory;
  isSystem: boolean;
  adapterId?: string;
  service?: { id: string; description: string };
  capabilities: ToolCapabilities;
  configPaths: string[];
  configPreviewAvailable: boolean[];
  runtimeDependencies: RuntimeDependency[];
}

export interface Provider {
  id: string;
  displayName: string;
  kind: 'first-party' | 'community' | 'custom-adapter' | 'local' | string;
  category: ToolCategory;
  authOwner: 'external-cli' | string;
  website?: string;
  installationCount: number;
}

export interface Installation {
  id: string;
  providerId: string;
  toolId: string;
  displayName: string;
  command: string;
  executable: string;
  aliases: string[];
  version?: string;
  source: ToolSource;
  adapterId?: string;
  status: 'ready' | 'degraded';
  lastSeen: string;
  active: boolean;
}

export interface Capability {
  id: string;
  providerId: string;
  installationId: string;
  key: string;
  displayName: string;
  kind: string;
  description: string;
  riskLevel: 'read-only' | 'controlled' | 'interactive' | 'confirmation-required' | string;
  agentCallable: boolean;
  available: boolean;
}

export interface ResourceCatalog {
  providers: Provider[];
  installations: Installation[];
  capabilities: Capability[];
}

export interface CustomAdapterAction {
  name: string;
  command: string;
}

export interface CustomAdapterInfo {
  id: string;
  displayName: string;
  path: string;
  fingerprint: string;
  status: 'pending' | 'trusted' | 'blocked' | 'invalid';
  message: string;
  hasStoredTrust: boolean;
  actions: CustomAdapterAction[];
}

export interface UpdatePlan {
  id: string;
  operation: 'install' | 'update';
  toolId: string;
  adapterId?: string;
  toolName: string;
  currentVersion?: string;
  targetVersion: string;
  executable: string;
  args: string[];
  displayCommand: string;
  source: string;
  requiresElevation: boolean;
}

export interface LatestVersionInfo {
  targetId: string;
  currentVersion?: string;
  latestVersion?: string;
  updateAvailable?: boolean;
  checkedAt: string;
  source: string;
  status: 'checked' | 'unsupported' | 'error';
  message: string;
}

export interface CliInformation {
  id: string;
  displayName: string;
  description: string;
  introduction: string;
  documentationUrl: string;
  category: ToolCategory;
}

export interface InstallCandidate extends CliInformation {
  adapterId: string;
  packageManager: string;
  packageName: string;
  source: string;
  managerAvailable: boolean;
  installed: boolean;
  installedToolId?: string;
}

export interface UpdateHistoryRecord {
  id: string;
  operation: 'install' | 'update';
  toolId: string;
  toolName: string;
  source: string;
  displayCommand: string;
  currentVersion?: string;
  targetVersion: string;
  observedVersion?: string;
  startedAt: string;
  finishedAt?: string;
  verifiedAt?: string;
  exitCode?: number;
  status: 'running' | 'succeeded' | 'failed' | 'verification-failed' | 'interrupted';
  message: string;
}

export interface TerminalSession {
  id: string;
  toolId: string;
  title: string;
  executable: string;
  args: string[];
  cwd: string;
  startedAt: string;
  status: 'running' | 'exited';
  kind: 'interactive' | 'diagnostic' | 'auth' | 'config' | 'management' | string;
  endedAt?: string;
  exitCode?: number;
}

export interface SessionHistoryRecord {
  id: string;
  toolId: string;
  title: string;
  kind: string;
  executable: string;
  cwd: string;
  startedAt: string;
  endedAt?: string;
  exitCode?: number;
}

export interface StatusField {
  label: string;
  value: string;
  tone: 'success' | 'warning' | 'muted' | string;
}

export interface AuthenticationStatus {
  toolId: string;
  installation?: HealthInstallation;
  status: 'authenticated' | 'unauthenticated' | 'needs-refresh' | 'expired' | 'unsupported' | 'unknown';
  method?: string;
  expiresInMinutes?: number;
  checkedAt: string;
  source: string;
  message: string;
  fields: StatusField[];
}

export interface DiagnosticCheck {
  id: string;
  label: string;
  status: 'pass' | 'warn' | 'fail' | 'info';
  message: string;
}

export interface DiagnosticReport {
  toolId: string;
  installation?: HealthInstallation;
  status: 'healthy' | 'warning' | 'error';
  checkedAt: string;
  durationMs: number;
  summary: string;
  checks: DiagnosticCheck[];
}

export type HealthInstallation = Pick<ToolRecord,
  'executable' | 'canonicalPath' | 'version' | 'source' | 'runtimeDependencies'>;

export interface ConfigPreview {
  path: string;
  exists: boolean;
  content: string;
  truncated: boolean;
  notice: string;
}

export interface AppSettings {
  showSystemTools: boolean;
  showAllExecutables: boolean;
  trayEnabled: boolean;
  defaultWorkingDirectory: string;
}
