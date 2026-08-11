import type { ToolCategory, ToolRecord } from './types';

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

