import { describe, expect, it } from 'vitest';
import { filterTools, splitArguments } from './tooling';
import type { ToolRecord } from './types';

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
};

describe('tool list helpers', () => {
  it('matches display names and aliases case-insensitively', () => {
    expect(filterTools([tool], 'AGENT', 'all')).toEqual([tool]);
    expect(filterTools([tool], 'Grok', 'all')).toEqual([tool]);
  });

  it('honours category filters', () => {
    expect(filterTools([tool], '', 'runtime')).toEqual([]);
  });

  it('treats every non-empty line as one literal argument', () => {
    expect(splitArguments(' --model\nclaude-sonnet\n\n')).toEqual(['--model', 'claude-sonnet']);
  });
});
