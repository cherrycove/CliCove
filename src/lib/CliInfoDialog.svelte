<script lang="ts">
  import { api } from './api';
  import { focusDialog } from './focusDialog';
  import ResourceMark from './ResourceMark.svelte';
  import type { CliInformation } from './types';

  export let candidate: CliInformation;
  export let categoryLabel: string;
  export let trigger: HTMLElement | null = null;
  export let onClose: () => void;

  let opening = false;
  let error = '';
  $: paragraphs = candidate.introduction.split(/\n\s*\n/).filter((paragraph) => paragraph.trim());

  async function openDocumentation() {
    if (opening) return;
    opening = true;
    error = '';
    try {
      await api.openCliDocumentation(candidate.id);
    } catch (cause) {
      error = `无法打开官方说明：${cause instanceof Error ? cause.message : String(cause)}`;
    } finally {
      opening = false;
    }
  }
</script>

<div class="modal-backdrop" role="presentation">
  <div class="modal cli-info-modal" role="dialog" aria-modal="true" aria-labelledby="cli-info-title" use:focusDialog={{ trigger }}>
    <header>
      <ResourceMark resource={candidate} size={48} />
      <div><span class="category">{categoryLabel}</span><h2 id="cli-info-title">{candidate.displayName}</h2></div>
    </header>
    <div class="introduction">
      {#each paragraphs as paragraph}<p>{paragraph}</p>{/each}
    </div>
    <a class="documentation-link" href={candidate.documentationUrl} rel="noopener noreferrer" aria-busy={opening} aria-disabled={opening} on:click|preventDefault={openDocumentation}>
      <span><strong>{opening ? '正在打开…' : '查看官方说明'}</strong><span aria-hidden="true">↗</span></span>
      <small>{candidate.documentationUrl}</small>
    </a>
    {#if error}<p class="documentation-error" role="alert">{error}</p>{/if}
    <div class="modal-actions"><button data-dialog-initial-focus on:click={onClose}>关闭</button></div>
  </div>
</div>

<style>
  .cli-info-modal { width: min(560px, calc(100vw - 48px)); }
  header { display: flex; align-items: center; gap: 14px; }
  .category { color: var(--text-tertiary); font-size: 11px; }
  h2 { margin: 3px 0 0; line-height: 1.35; }
  .introduction { margin: 24px 0; }
  .introduction p { margin: 0 0 14px; color: var(--text-secondary); font-size: 13px; line-height: 1.9; white-space: pre-line; }
  .introduction p:last-child { margin-bottom: 0; }
  .documentation-link { display: block; padding: 14px 16px; border: 1px solid var(--border); border-radius: 4px; background: var(--paper); color: var(--brand-blue); text-decoration: none; }
  .documentation-link:hover { background: #e7ecfc; border-color: #bdcaef; }
  .documentation-link > span { display: flex; justify-content: space-between; gap: 12px; font-size: 13px; }
  .documentation-link small { display: block; margin-top: 5px; color: var(--text-tertiary); font-size: 11px; line-height: 1.7; overflow-wrap: anywhere; }
  .documentation-error { margin-top: 12px; color: var(--danger); font-size: 12px; line-height: 1.6; }
</style>
