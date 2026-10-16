<script lang="ts">
  import { appUpdates, updateDate } from './appUpdates';
  import BlockLoader from './BlockLoader.svelte';
</script>

<section class="app-update-settings" aria-label="CliCove 更新">
  <div class="setting-group">
    <div><h3>CliCove 更新 <small>v{$appUpdates.currentVersion || '—'}</small></h3>
      <p>检查并安装 CliCove 的新版本。</p>
      {#if $appUpdates.preferences?.lastChecked}<p>上次检查 {updateDate($appUpdates.preferences.lastChecked)}</p>{/if}
    </div>
    <button disabled={!$appUpdates.preferences || $appUpdates.phase !== 'idle'} on:click={() => appUpdates.check(true)}>
      {#if $appUpdates.phase === 'checking'}<BlockLoader />检查中…{:else}检查 CliCove 更新{/if}
    </button>
  </div>
  <div class="setting-group setting-toggle">
    <div><h3><label for="app-auto-update">自动检查 CliCove 更新</label></h3><p id="app-auto-update-hint">启动后检查，每 6 小时最多一次。发现新版本时提醒，由你选择是否安装。</p></div>
    <label class="switch"><input id="app-auto-update" aria-describedby="app-auto-update-hint" type="checkbox" role="switch" checked={$appUpdates.preferences?.autoCheck ?? true} disabled={!$appUpdates.preferences || $appUpdates.saving} on:change={(event) => appUpdates.setAutoCheck(event.currentTarget.checked)} /><span aria-hidden="true"></span></label>
  </div>
  {#if $appUpdates.preferences?.skippedVersion}<p class="update-message">已跳过版本 {$appUpdates.preferences.skippedVersion}；手动检查仍可安装。</p>{/if}
  {#if $appUpdates.error}<p class="update-error" role="alert">{$appUpdates.error}</p>
  {:else if $appUpdates.message}<p class="update-message" role="status">{$appUpdates.message}</p>{/if}
</section>

<style>
  h3 small { margin-left: 8px; font-weight: 400; color: var(--text-muted); }
  .update-message, .update-error { font-size: 12px; margin: 10px 0; line-height: 1.7; }
  .update-error { color: #b33a28; }
</style>
