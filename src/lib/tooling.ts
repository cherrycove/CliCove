import type { LatestVersionInfo, ToolCategory, ToolRecord } from './types';

export function displayPath(path: string): string {
  const prefix = '\\\\?\\';
  if (!path.startsWith(prefix)) return path;
  const value = path.slice(prefix.length);
  if (/^[a-z]:\\/i.test(value)) return value;
  if (value.toUpperCase().startsWith('UNC\\')) return '\\\\' + value.slice(4);
  return path;
}

export function filterTools(
  tools: ToolRecord[],
  query: string,
  category: ToolCategory | 'all',
): ToolRecord[] {
  const term = query.trim().toLowerCase();
  return tools.filter((tool) => {
    const matchesCategory = category === 'all' || tool.category === category;
    const matchesQuery =
      !term ||
      tool.displayName.toLowerCase().includes(term) ||
      tool.aliases.some((alias) => alias.toLowerCase().includes(term)) ||
      (tool.source.packageName?.toLowerCase().includes(term) ?? false) ||
      (tool.service?.description.toLowerCase().includes(term) ?? false) ||
      tool.source.label.toLowerCase().includes(term);
    return matchesCategory && matchesQuery;
  });
}

export function splitArguments(value: string): string[] {
  return value
    .split('\n')
    .map((argument) => argument.trim())
    .filter(Boolean);
}

export function resolveToolSelection(visibleTools: ToolRecord[], selectedId: string): string {
  return visibleTools.some((tool) => tool.id === selectedId)
    ? selectedId
    : (visibleTools[0]?.id ?? '');
}

// Unsupported is a result of the last attempt, not a permanent capability lock.
export function canQueryLatest(checking: boolean, checkingAll: boolean, available = true): boolean {
  return available && !checking && !checkingAll;
}

export function indexLatestChecks(
  checks: LatestVersionInfo[],
): Map<string, LatestVersionInfo> {
  const index = new Map<string, LatestVersionInfo>();
  for (const check of checks) {
    // Results are newest-first; keep the first entry for each target.
    if (!index.has(check.targetId)) index.set(check.targetId, check);
  }
  return index;
}
