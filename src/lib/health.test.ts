import { describe, expect, it } from 'vitest';
import { authenticationView, diagnosticView, HEALTH_MAX_AGE_MS, staleReason } from './health';
import type { AuthenticationStatus, DiagnosticReport, ToolRecord } from './types';

const now = Date.parse('2026-09-07T07:30:00Z');
const tool: ToolRecord = {
  id: 'cli', name: 'cli', displayName: 'CLI', executable: '/bin/cli', canonicalPath: '/opt/cli/1.0/cli',
  aliases: ['cli'], relatedCommands: ['cli'], version: '1.0',
  source: { kind: 'user', label: 'User PATH', scope: 'user' }, category: 'ai-agent', isSystem: false,
  capabilities: { terminal: true, auth: true, config: true, diagnose: true, update: true, versionProbe: true },
  configPaths: [], configPreviewAvailable: [], runtimeDependencies: [],
};
const auth: AuthenticationStatus = { toolId: tool.id, installation: tool, status: 'authenticated', checkedAt: new Date(now).toISOString(), source: 'cli status', message: '已登录', fields: [] };
const report: DiagnosticReport = { toolId: tool.id, installation: tool, status: 'healthy', checkedAt: auth.checkedAt, durationMs: 100, summary: '通过', checks: [
  { id: 'executable', label: '可执行文件', status: 'pass', message: '存在' },
  { id: 'version', label: '版本响应', status: 'pass', message: '1.0' },
  { id: 'authentication', label: '认证状态', status: 'pass', message: '已登录' },
] };

describe('installation-aware CLI health', () => {
  it('does not equate installed and dependency-ready with authenticated or checked', () => {
    const state = diagnosticView(tool, undefined, undefined, now);
    expect(state.title).toBe('等待检查');
    expect(state.tone).toBe('muted');
    expect(state.authentication.label).toBe('尚未检查');
  });

  it('accepts matching installation evidence despite JSON field order and absent optionals', () => {
    const cached = { ...report, installation: { ...tool, source: { scope: 'user', label: 'User PATH', kind: 'user', packageName: undefined } } };
    expect(staleReason(tool, cached, now)).toBeUndefined();
    expect(diagnosticView(tool, auth, cached, now).tone).toBe('success');
  });

  it.each([
    { canonicalPath: '/opt/cli/2.0/cli' }, { executable: '/usr/local/bin/cli' }, { version: '2.0' },
    { source: { ...tool.source, packageName: 'different-package' } },
    { runtimeDependencies: [{ name: 'node', available: true, executable: '/bin/node' }] },
  ])('invalidates diagnostic and auth evidence after an installation change: %j', (change) => {
    const changed = { ...tool, ...change };
    expect(staleReason(changed, report, now)).toContain('已变化');
    expect(authenticationView(changed, auth, now).current).toBeUndefined();
    expect(diagnosticView(changed, auth, report, now).title).toBe('需要重新检查');
  });

  it('keeps legacy, expired, malformed and future-dated results out of current health', () => {
    for (const cached of [
      { ...report, installation: undefined },
      { ...report, checkedAt: new Date(now - HEALTH_MAX_AGE_MS).toISOString() },
      { ...report, checkedAt: 'invalid' },
      { ...report, checkedAt: new Date(now + 120_000).toISOString() },
      { ...report, toolId: 'another' },
    ]) {
      const state = diagnosticView(tool, undefined, cached, now);
      expect(state.current).toBeUndefined();
      expect(state.tone).not.toBe('success');
    }
  });

  it('surfaces missing dependencies even without a diagnostic report', () => {
    const state = diagnosticView({ ...tool, runtimeDependencies: [{ name: 'node', available: false }] }, undefined, undefined, now);
    expect(state.tone).toBe('danger');
    expect(state.issues[0].message).toContain('node');
  });

  it('uses a newer auth check to replace an earlier successful auth check', () => {
    const state = diagnosticView(tool, { ...auth, status: 'unauthenticated', message: '已退出登录' }, report, now);
    expect(state.tone).toBe('danger');
    expect(state.issues[0].message).toBe('已退出登录');
    expect(state.passed.some((check) => check.id === 'authentication')).toBe(false);
  });

  it('does not retain a diagnostic auth failure after a successful recheck', () => {
    const failed = { ...report, status: 'error' as const, checks: report.checks.map((check) => check.id === 'authentication' ? { ...check, status: 'fail' as const } : check) };
    expect(diagnosticView(tool, auth, failed, now).tone).toBe('success');
  });

  it('expires authentication at its deadline, without rounding a valid last minute down to zero', () => {
    const expiring = { ...auth, expiresInMinutes: 2 };
    expect(authenticationView(tool, expiring, now + 90_000).current?.status).toBe('authenticated');
    expect(authenticationView(tool, expiring, now + 90_000).current?.expiresInMinutes).toBe(1);
    expect(authenticationView(tool, expiring, now + 120_000).current?.status).toBe('expired');
    expect(diagnosticView(tool, expiring, report, now + 120_000).tone).toBe('danger');
  });

  it('does not call unsupported or unknown authentication healthy', () => {
    for (const status of ['unsupported', 'unknown', 'needs-refresh'] as const) {
      expect(diagnosticView(tool, { ...auth, status }, report, now).tone).toBe('warning');
    }
  });

  it('does not require authentication for CLIs without declared auth capability', () => {
    const local = { ...tool, capabilities: { ...tool.capabilities, auth: false } };
    const localReport = { ...report, installation: local, checks: report.checks.filter((check) => check.id !== 'authentication') };
    expect(diagnosticView(local, undefined, localReport, now).tone).toBe('success');
    expect(authenticationView(local, undefined, now).label).toBe('未声明认证');
  });
});
