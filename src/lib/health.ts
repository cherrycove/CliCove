import type { AuthenticationStatus, DiagnosticCheck, DiagnosticReport, HealthInstallation, ToolRecord, ToolSource } from './types';

export const HEALTH_MAX_AGE_MS = 15 * 60 * 1000;
export type HealthTone = 'success' | 'warning' | 'danger' | 'muted';

function installationKeys(value: HealthInstallation): (string | null)[] {
  const source = value.source;
  return [
    JSON.stringify([value.executable, value.canonicalPath]), value.version ?? null,
    JSON.stringify([source.kind, source.label, source.scope, source.packageName ?? null, source.packageVersion ?? null]),
    JSON.stringify(value.runtimeDependencies.map((item) => [item.name, item.executable ?? null, item.available])
      .sort((a, b) => String(a[0]).localeCompare(String(b[0])))),
  ];
}

export function staleReason(tool: ToolRecord, result: AuthenticationStatus | DiagnosticReport | undefined, now: number): string | undefined {
  if (!result) return undefined;
  if (result.toolId !== tool.id || !result.installation) return '这份结果来自旧版检查，请重新运行';
  const current = installationKeys(tool);
  const cached = installationKeys(result.installation);
  const changed = ['安装路径', '版本', '安装来源', '运行依赖'].filter((_, index) => current[index] !== cached[index]);
  if (changed.length) return `${changed.join('、')}已变化，请重新检查`;
  const age = now - Date.parse(result.checkedAt);
  if (!Number.isFinite(age) || age < -60_000 || age >= HEALTH_MAX_AGE_MS) return '结果已超过有效时间，请重新检查（15 分钟）';
  return undefined;
}

export function authenticationView(tool: ToolRecord, auth: AuthenticationStatus | undefined, now: number) {
  if (!tool.capabilities.auth) return { label: '未声明认证', tone: 'muted' as HealthTone, message: '此 CLI 未声明认证检查能力', stale: undefined, current: undefined };
  const stale = staleReason(tool, auth, now);
  if (stale || !auth) return { label: stale ? '待重新检查' : '尚未检查', tone: 'muted' as HealthTone, message: stale ?? '检查当前 CLI 的登录状态', stale, current: undefined };
  const ageMinutes = Math.max(0, (now - Date.parse(auth.checkedAt)) / 60_000);
  const remaining = auth.expiresInMinutes == null ? undefined : Math.max(0, Math.ceil(auth.expiresInMinutes - ageMinutes));
  const status = remaining === 0 && auth.status === 'authenticated' ? 'expired' : auth.status;
  const labels = { authenticated: '已认证', unauthenticated: '未登录', 'needs-refresh': '需要刷新认证', expired: '认证已过期', unsupported: '暂不支持检查', unknown: '认证状态未知' };
  const tone: HealthTone = status === 'authenticated' ? 'success' : status === 'unauthenticated' || status === 'expired' ? 'danger' : status === 'unsupported' ? 'muted' : 'warning';
  return { label: labels[status], tone, message: status !== auth.status ? '上次检查记录的认证有效期已结束，请重新检查或登录' : auth.message, stale, current: { ...auth, status, expiresInMinutes: remaining } };
}

export function diagnosticView(tool: ToolRecord, auth: AuthenticationStatus | undefined, report: DiagnosticReport | undefined, now: number) {
  const stale = staleReason(tool, report, now);
  const current = report && !stale ? report : undefined;
  const authentication = authenticationView(tool, auth, now);
  const checks: DiagnosticCheck[] = current ? current.checks.map((check) => ({ ...check })) : [];
  const missing = tool.runtimeDependencies.filter((item) => !item.available);
  if (missing.length && !checks.some((item) => item.id === 'dependencies' && item.status === 'fail')) {
    checks.push({ id: 'dependencies', label: '运行依赖缺失', status: 'fail', message: `缺少 ${missing.map((item) => item.name).join('、')}` });
  }
  if (tool.capabilities.auth) {
    const authIndex = checks.findIndex((item) => item.id === 'authentication');
    const check: DiagnosticCheck = { id: 'authentication', label: authentication.label, status: authentication.tone === 'success' ? 'pass' : authentication.tone === 'danger' ? 'fail' : 'warn', message: authentication.message };
    if (authIndex >= 0) checks[authIndex] = check;
    else if (current || authentication.current) checks.push(check);
  }
  const issues = checks.filter((check) => check.status !== 'pass');
  const failures = issues.filter((check) => check.status === 'fail');
  const passed = checks.filter((check) => check.status === 'pass');
  const tone: HealthTone = failures.length ? 'danger' : issues.length ? 'warning' : current && checks.length ? 'success' : 'muted';
  const title = failures.length ? '发现异常' : issues.length ? '需要留意' : current && checks.length ? '检查通过' : stale ? '需要重新检查' : '等待检查';
  const description = failures.length ? `${failures.length} 项异常需要处理${issues.length > failures.length ? `，另有 ${issues.length - failures.length} 项提醒` : ''}`
    : issues.length ? `${issues.length} 项提醒，查看下方原因与处理建议`
    : current && checks.length ? `${passed.length} 项检查通过，结果对应下方安装实例`
    : stale ?? '已发现本机安装，运行检查以确认版本响应、依赖和认证';
  return { current, stale, authentication, checks, issues, passed, tone, title, description };
}

export function sourceLabel(source: ToolSource): string {
  const labels: Record<string, string> = { 'User PATH': '用户 PATH', 'System PATH': '系统 PATH', 'Global npm': 'npm 全局安装', 'Desktop application': '桌面应用', 'System': '系统内置' };
  return labels[source.label] ?? source.label;
}

export function checkAdvice(check: DiagnosticCheck): string {
  switch (check.id) {
    case 'executable': return '重新扫描以定位当前可用的 CLI。';
    case 'dependencies': return '安装缺失的运行依赖后，重新扫描。';
    case 'version': return '确认 CLI 可以启动，再刷新版本或运行官方诊断。';
    case 'authentication': return check.status === 'fail' ? '登录后重新检查认证状态。' : '可重新检查；暂不支持检查时，请通过 CLI 确认登录状态。';
    case 'configuration': return '首次运行可能创建配置，也可查看下方配置入口。';
    default: return '处理后重新运行检查，确认问题是否消除。';
  }
}

export function checkTime(value: string): string {
  const date = new Date(value);
  return Number.isFinite(date.getTime()) ? date.toLocaleString('zh-CN', { month: '2-digit', day: '2-digit', hour: '2-digit', minute: '2-digit' }) : '时间未知';
}
