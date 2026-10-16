<script lang="ts">
  import { appUpdates, updateDate } from './appUpdates';
  import { focusDialog } from './focusDialog';
  import Icon from './Icon.svelte';
  import BlockLoader from './BlockLoader.svelte';
  export let visible = false;
  export let activeSessions = 0;
  $: busy = ['downloading', 'installing'].includes($appUpdates.phase);
  $: total = $appUpdates.progress?.total;
  $: downloaded = $appUpdates.progress?.downloaded ?? 0;
  $: percent = total ? Math.min(100, Math.round(downloaded / total * 100)) : undefined;
</script>

{#if visible && $appUpdates.update}
  <div class="modal-backdrop" role="presentation">
    <div class="modal app-update-modal" role="dialog" aria-modal="true" aria-labelledby="app-update-title" aria-describedby="app-update-description" use:focusDialog>
      <span class="modal-icon"><Icon name="updates" size={24} /></span>
      <h2 id="app-update-title">CliCove {$appUpdates.update.version} 可用</h2>
      <p id="app-update-description">当前版本 {$appUpdates.currentVersion} · 安装完成后将重新启动。</p>
      {#if updateDate($appUpdates.update.publishedAt)}<p>发布时间 {updateDate($appUpdates.update.publishedAt)}</p>{/if}
      {#if $appUpdates.update.notes}<pre class="release-notes">{$appUpdates.update.notes}</pre>{/if}
      {#if activeSessions > 0}<p class="session-hint" role="status">还有 {activeSessions} 个终端正在运行，请结束会话后再更新。</p>{/if}
      {#if busy}
        <div class="download-status" role="status" aria-live="polite">
          <span><BlockLoader />{$appUpdates.phase === 'installing' ? '正在安装并重新启动…' : `正在下载更新${percent == null ? '' : ` ${percent}%`}`}</span>
          {#if $appUpdates.phase === 'downloading'}<progress max="100" value={percent} aria-label="更新下载进度"></progress><small>{(downloaded / 1048576).toFixed(1)} MB{total ? ` / ${(total / 1048576).toFixed(1)} MB` : ''}</small>{/if}
        </div>
      {/if}
      {#if $appUpdates.error}<p class="update-error" role="alert">{$appUpdates.error}</p>{/if}
      <div class="modal-actions">
        <button disabled={busy || $appUpdates.saving} on:click={appUpdates.skip}>跳过此版本</button>
        <button data-dialog-initial-focus disabled={busy || $appUpdates.saving} on:click={appUpdates.later}>稍后提醒</button>
        <button class="primary" disabled={busy || $appUpdates.saving || activeSessions > 0} on:click={appUpdates.install}>{busy ? '更新中…' : $appUpdates.error ? '重试更新' : '立即更新'}</button>
      </div>
    </div>
  </div>
{/if}

<style>
  .release-notes { max-height: 260px; overflow: auto; text-align: left; font-family: inherit; }
  .session-hint { color: #986500; }
  .update-error { color: #b33a28; overflow-wrap: anywhere; }
  .download-status { display: grid; gap: 8px; margin: 16px 0; font-size: 13px; }
  .download-status span { display: flex; align-items: center; gap: 8px; }
  progress { width: 100%; accent-color: var(--primary, #3158ef); }
  .modal-actions { flex-wrap: wrap; }
</style>
