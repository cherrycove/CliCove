export type ToolCategory =
  | 'ai-agent'
  | 'collaboration'
  | 'runtime'
  | 'cloud-container'
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
  capabilities: ToolCapabilities;
  configPaths: string[];
}

export interface UpdatePlan {
  id: string;
  toolId: string;
  toolName: string;
  currentVersion?: string;
  targetVersion: string;
  executable: string;
  args: string[];
  displayCommand: string;
  source: string;
  requiresElevation: boolean;
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
}

export interface ConfigPreview {
  path: string;
  exists: boolean;
  content: string;
  truncated: boolean;
}

export interface AppSettings {
  showSystemTools: boolean;
  showAllExecutables: boolean;
  trayEnabled: boolean;
  defaultWorkingDirectory: string;
}
