<script lang="ts">
  // Display metadata only. These abbreviations never participate in adapter or
  // provider resolution; all calls continue to use the resource's existing ID.
  export let resource: {
    id?: string;
    adapterId?: string | null;
    name?: string;
    displayName?: string;
  };
  export let provider = false;
  export let size = 40;
  export let inverse = false;

  const cliMarks: Record<string, string> = {
    claude: 'CL', codex: 'CX', gemini: 'GM', 'gemini-cli': 'GM',
    lark: 'LK', 'lark-cli': 'LK', aws: 'AWS', ve: 'VE', volcengine: 'VE',
    owl: 'OW', wrangler: 'WR', cloudflared: 'CF', meegle: 'MG',
    cursor: 'CU', agent: 'CU', gcloud: 'GC', az: 'AZ', aliyun: 'AL',
    opencode: 'OC', trellis: 'TR', grok: 'GK', gh: 'GH', glab: 'GL',
  };
  const providerMarks: Record<string, string> = {
    anthropic: 'AN', openai: 'OA', aws: 'AWS', azure: 'AZ', cloudflare: 'CF',
    cursor: 'CU', 'google-cloud': 'GC', guance: 'GU', lark: 'LK',
    opencode: 'OC', trellis: 'TR', xai: 'XA', kubernetes: 'K8S',
  };
  function fallback(label: string): string {
    const words = label.normalize('NFKC').replace(/\bCLI\b/gi, '').match(/[\p{L}\p{N}]+/gu) ?? [];
    if (!words.length) return 'CLI';
    return (words.length > 1 ? words.slice(0, 2).map(word => Array.from(word)[0]) : Array.from(words[0] ?? '').slice(0, 2)).join('').toUpperCase();
  }
  $: abbreviation = provider
    ? providerMarks[resource.id ?? ''] ?? fallback(resource.displayName ?? resource.name ?? 'CLI')
    : cliMarks[resource.adapterId ?? ''] ?? cliMarks[resource.name ?? ''] ?? cliMarks[resource.id ?? ''] ?? fallback(resource.displayName ?? resource.name ?? 'CLI');
</script>

<span class="resource-mark" class:inverse data-length={Array.from(abbreviation).length} style:--mark-size={`${size}px`} aria-hidden="true">
  <span class="mark-letters">{abbreviation}</span><span class="mark-corner"></span>
</span>

<style>
  .resource-mark {
    --mark-background: #305ae8; --mark-foreground: #fffef9;
    display: inline-grid; place-items: center; position: relative; flex: none;
    width: var(--mark-size); height: var(--mark-size); overflow: hidden;
    border-radius: calc(var(--mark-size) * .1);
    background: var(--mark-background); color: var(--mark-foreground);
    font: 800 calc(var(--mark-size) * .43)/1 ui-monospace, 'SFMono-Regular', Menlo, Consolas, 'PingFang SC', monospace;
    letter-spacing: -.065em; text-align: center; user-select: none;
  }
  .resource-mark.inverse { --mark-background: #fffef9; --mark-foreground: #305ae8; }
  .resource-mark[data-length='3'] { font-size: calc(var(--mark-size) * .33); letter-spacing: -.075em; }
  .mark-letters { transform: translate(-.015em, -.005em); }
  .mark-corner { position: absolute; bottom: 0; right: 0; width: 19%; height: 19%; border-radius: 22% 0 0; background: #f2cb4b; }
</style>
