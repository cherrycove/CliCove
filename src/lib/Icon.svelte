<script lang="ts">
  export let name: string;
  export let size = 18;
  export let strokeWidth = 1.8;
  export let label: string | undefined = undefined;
</script>

<svg class="ui-icon" width={size} height={size} viewBox="0 0 24 24" fill="currentColor" stroke="none" stroke-width={strokeWidth} xmlns="http://www.w3.org/2000/svg" aria-hidden={label ? undefined : 'true'} aria-label={label}>
  {#if name === 'library'}
    <rect x="2" y="2" width="9" height="9" rx="0" fill="var(--icon-accent, #305ae8)"/><path d="M14 2h8v9h-8z" fill="var(--icon-secondary, #e9644b)" /><path d="M2 14h9v8H2z" fill="currentColor" /><path d="M14 22v-8h8a8 8 0 0 1-8 8" fill="var(--icon-tertiary, #f2cb4b)" />
  {:else if name === 'resources'}
    <path d="M4 2h7v7H4z" fill="var(--icon-accent, #305ae8)" /><path d="M14 7h7v7h-7z" fill="var(--icon-secondary, #e9644b)" /><path d="M4 15h7v7H4z" fill="var(--icon-tertiary, #f2cb4b)" /><path d="M6 10h3v3h4v-3h3v7H9v-2H6z" fill="currentColor" />
  {:else if name === 'updates'}
    <path d="M12 2 3 11h7v6h4v-6h7z" fill="var(--icon-accent, #305ae8)" /><rect x="3" y="19" width="12" height="3" rx="0" fill="currentColor"/><rect x="17" y="19" width="4" height="3" rx="0" fill="var(--icon-secondary, #e9644b)"/>
  {:else if name === 'terminal'}
    <path d="M3 3h10v3H5v12h14V6h-4V3h6v18H2V3z" fill="currentColor" /><rect x="15" y="3" width="6" height="3" rx="0" fill="var(--icon-accent, #305ae8)"/><path d="M7 8 12 12l-5 4v-3l2-1-2-1z" fill="var(--icon-accent, #305ae8)" /><rect x="13" y="14" width="4" height="2" rx="0" fill="var(--icon-secondary, #e9644b)"/>
  {:else if name === 'settings'}
    <rect x="2" y="5" width="20" height="3" rx="0" fill="currentColor"/><rect x="2" y="16" width="20" height="3" rx="0" fill="currentColor"/><rect x="6" y="2" width="5" height="9" rx="0" fill="var(--icon-accent, #305ae8)"/><rect x="15" y="13" width="5" height="9" rx="0" fill="var(--icon-secondary, #e9644b)"/>
  {:else if name === 'search'}
    <path d="M10 2a8 8 0 1 0 0 16v-4a4 4 0 1 1 0-8z" fill="currentColor" /><path d="M12 2.3a8 8 0 0 1 5.7 9.7h-4.4A4 4 0 0 0 12 6.5z" fill="var(--icon-accent, #305ae8)" /><path d="m16 14 7 6-3 3-6-7z" fill="var(--icon-secondary, #e9644b)" />
  {:else if name === 'refresh'}
    <path d="M11 2v4a6 6 0 0 0-5 6H2A10 10 0 0 1 11 2" fill="var(--icon-accent, #305ae8)" /><path d="M13 2a10 10 0 0 1 9 10h-4a6 6 0 0 0-5-6z" fill="currentColor" /><path d="M22 14a10 10 0 0 1-9 8v-4a6 6 0 0 0 5-4z" fill="var(--icon-secondary, #e9644b)" /><path d="M2 14h4a6 6 0 0 0 5 4v4a10 10 0 0 1-9-8z" fill="currentColor" /><path d="M1 9h8l-4 5z" fill="var(--icon-accent, #305ae8)" />
  {:else if name === 'arrow-up'}
    <path d="m2 14 9-9v6l-6 6z"/><path d="m13 5 9 9-3 3-6-6z" fill="var(--icon-accent)"/>
  {:else if name === 'arrow-down'}
    <path d="m2 10 3-3 6 6v6z"/><path d="m13 13 6-6 3 3-9 9z" fill="var(--icon-accent)"/>
  {:else if name === 'command'}
    <path d="m7 4-7 8 7 8v-6l-2-2 2-2z" fill="var(--icon-accent, #305ae8)" /><path d="m17 4 7 8-7 8v-6l2-2-2-2z" fill="var(--icon-secondary, #e9644b)" /><path d="M12 3h4l-4 18H8z" fill="currentColor" />
  {:else if name === 'shield'}
    <path d="M3 4 11 1v21C6 19 3 15 3 10z" fill="var(--icon-accent, #305ae8)" /><path d="M13 1 21 4v6c0 5-3 9-8 12z" fill="currentColor" />
  {:else if name === 'activity'}
    <rect x="2" y="12" width="5" height="10" rx="0" fill="var(--icon-accent, #305ae8)"/><rect x="9" y="2" width="5" height="20" rx="0" fill="currentColor"/><rect x="16" y="7" width="6" height="15" rx="0" fill="var(--icon-secondary, #e9644b)"/>
  {:else if name === 'sliders'}
    <rect x="2" y="5" width="20" height="3" rx="0" fill="currentColor"/><rect x="2" y="16" width="20" height="3" rx="0" fill="currentColor"/><rect x="6" y="2" width="5" height="9" rx="0" fill="var(--icon-accent, #305ae8)"/><rect x="15" y="13" width="5" height="9" rx="0" fill="var(--icon-secondary, #e9644b)"/>
  {:else if name === 'plus'}
    <path d="M10 2h4v8h8v4h-8v8h-4v-8H2v-4h8z"/><path d="M10 2h4v8h-4z" fill="var(--icon-accent)"/>
  {:else if name === 'copy'}
    <path d="M2 2h13v4H6v9H2z" fill="var(--icon-accent, #305ae8)" /><path d="M8 8h14v14H8z" fill="currentColor" /><rect x="11" y="11" width="8" height="8" rx="0" fill="var(--icon-cutout, #fffef9)"/><rect x="17" y="8" width="5" height="5" rx="0" fill="var(--icon-secondary, #e9644b)"/>
  {:else if name === 'focus'}
    <path d="M2 2h8v3H5v5H2zM14 2h8v8h-3V5h-5zM2 14h3v5h5v3H2z"/><path d="M19 14h3v8h-8v-3h5z" fill="var(--icon-accent)"/>
  {:else if name === 'stop'}
    <path d="M4 4h10v16H4z"/><path d="M16 4h4v16h-4z" fill="var(--icon-accent)"/>
  {:else if name === 'restart'}
    <path d="M11 2v4a6 6 0 0 0-5 6H2A10 10 0 0 1 11 2" fill="var(--icon-accent, #305ae8)" /><path d="M13 2a10 10 0 0 1 9 10h-4a6 6 0 0 0-5-6z" fill="currentColor" /><path d="M22 14a10 10 0 0 1-9 8v-4a6 6 0 0 0 5-4z" fill="var(--icon-secondary, #e9644b)" /><path d="M2 14h4a6 6 0 0 0 5 4v4a10 10 0 0 1-9-8z" fill="currentColor" /><path d="M1 9h8l-4 5z" fill="var(--icon-accent, #305ae8)" />
  {:else if name === 'close'}
    <path d="m3 6 3-3 6 6-3 3z" fill="var(--icon-accent, #305ae8)" /><path d="m15 12 6 6-3 3-6-6z" fill="currentColor" /><path d="m18 3 3 3L6 21l-3-3z" fill="currentColor" />
  {:else if name === 'check'}
    <path d="m3 11 4-4 5 5-4 4z" fill="currentColor" /><path d="m10 17 10-12 3 3-13 14-3-3z" fill="currentColor" />
  {:else if name === 'package'}
    <rect x="2" y="3" width="9" height="8" rx="0" fill="var(--icon-accent, #305ae8)"/><rect x="13" y="3" width="9" height="8" rx="0" fill="var(--icon-secondary, #e9644b)"/><path d="M2 13h9v9H2z" fill="currentColor" /><path d="M13 13h9v9h-9z" fill="var(--icon-tertiary, #f2cb4b)" />
  {:else if name === 'history'}
    <path d="M11 1v4a7 7 0 1 0 8 8h4A11 11 0 1 1 11 1" fill="currentColor" /><path d="M13 1a11 11 0 0 1 10 10h-4a7 7 0 0 0-6-6z" fill="var(--icon-accent, #305ae8)" /><path d="M10 7h3v4l4 2-2 3-5-3z" fill="currentColor" />
  {:else if name === 'lock'}
    <path d="M6 9V7a6 6 0 0 1 12 0v2h-4V7a2 2 0 0 0-4 0v2z" fill="currentColor" /><rect x="3" y="11" width="8" height="11" rx="0" fill="var(--icon-accent, #305ae8)"/><rect x="13" y="11" width="8" height="11" rx="0" fill="var(--icon-secondary, #e9644b)"/>
  {:else if name === 'chevron-right'}
    <path d="m7 2 5 5v5L7 7z" fill="var(--icon-accent, #305ae8)" /><path d="m14 7 5 5-12 12v-6l7-7z" fill="currentColor" />
  {:else if name === 'info'}
    <path d="M11 1v4a7 7 0 1 0 8 8h4A11 11 0 1 1 11 1"/><path d="M13 1a11 11 0 0 1 10 10h-4a7 7 0 0 0-6-6z" fill="var(--icon-accent)"/><path d="M10 10h4v8h-4zM10 6h4v2h-4z"/>
  {/if}
</svg>
