import { describe, expect, it } from 'vitest';
import { canQueryLatest, filterTools, indexLatestChecks, resolveToolSelection, splitArguments } from './tooling';
import type { LatestVersionInfo, ToolRecord } from './types';

const tool: ToolRecord = {
  id: '1',
  name: 'grok',
  displayName: 'Grok CLI',
  executable: '/tmp/grok',
  canonicalPath: '/tmp/grok',
  aliases: ['agent', 'grok'],
  relatedCommands: ['agent', 'grok'],
  version: '1.0.0',
  source: { kind: 'user', label: 'User PATH', scope: 'user' },
  category: 'ai-agent',
  isSystem: false,
  adapterId: 'grok',
  capabilities: {
    terminal: true,
    versionProbe: true,
    update: true,
    diagnose: true,
    auth: true,
    config: true,
  },
  configPaths: [],
  configPreviewAvailable: [],
  runtimeDependencies: [],
};

describe('tool list helpers', () => {
  it('keeps the current selection when it is still visible', () => {
    expect(resolveToolSelection([{ ...tool, id: 'another' }, tool], tool.id)).toBe(tool.id);
  });

  it('selects the first filtered tool instead of showing a hidden tool detail', () => {
    const docker = {
      ...tool,
      id: 'docker',
      name: 'docker',
      displayName: 'Docker CLI',
      aliases: ['docker'],
      category: 'infrastructure' as const,
    };
    const visible = filterTools([tool, docker], '', 'infrastructure');
    expect(resolveToolSelection(visible, tool.id)).toBe(docker.id);
    expect(resolveToolSelection(filterTools([tool, docker], 'grok', 'all'), docker.id)).toBe(tool.id);
  });

  it('clears the detail selection when no tool matches', () => {
    expect(resolveToolSelection([], tool.id)).toBe('');
  });

  it('allows a new query independently of a previous unsupported result', () => {
    expect(canQueryLatest(false, false)).toBe(true);
    expect(canQueryLatest(true, false)).toBe(false);
    expect(canQueryLatest(false, true)).toBe(false);
    expect(canQueryLatest(false, false, false)).toBe(false);
  });
  it('matches display names and aliases case-insensitively', () => {
    expect(filterTools([tool], 'AGENT', 'all')).toEqual([tool]);
    expect(filterTools([tool], 'Grok', 'all')).toEqual([tool]);
  });

  it('honours category filters', () => {
    expect(filterTools([tool], '', 'runtime')).toEqual([]);
  });

  it('finds catalog-only services by package and Chinese description', () => {
    const volcengine: ToolRecord = {
      ...tool,
      name: 've',
      displayName: 'Volcengine CLI',
      aliases: ['ve'],
      adapterId: undefined,
      category: 'cloud-service',
      source: { kind: 'npm', label: 'Global npm', packageName: '@volcengine/cli', scope: 'global' },
      service: { id: 'volcengine', description: '火山引擎云资源与服务管理' },
    };
    expect(filterTools([volcengine], '火山引擎', 'all')).toEqual([volcengine]);
    expect(filterTools([volcengine], '@VOLCENGINE/CLI', 'cloud-service')).toEqual([volcengine]);
    expect(filterTools([volcengine], '火山引擎', 'runtime')).toEqual([]);
  });

  it('treats every non-empty line as one literal argument', () => {
    expect(splitArguments(' --model\nclaude-sonnet\n\n')).toEqual(['--model', 'claude-sonnet']);
  });

  it('indexes the newest latest-version result for immediate row refresh', () => {
    const check = (latestVersion: string, checkedAt: string): LatestVersionInfo => ({
      targetId: 'grok',
      currentVersion: '1.0.0',
      latestVersion,
      updateAvailable: true,
      checkedAt,
      source: 'GitHub Releases',
      status: 'checked',
      message: `发现 ${latestVersion}`,
    });

    const index = indexLatestChecks([
      check('1.0.5', '2026-08-26T10:00:00Z'),
      check('1.0.3', '2026-08-13T10:00:00Z'),
    ]);

    expect(index.get('grok')?.latestVersion).toBe('1.0.5');
  });
});
