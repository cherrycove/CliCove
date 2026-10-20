<script context="module" lang="ts">
  let nextId = 0;
</script>

<script lang="ts">
  import { onMount } from 'svelte';

  let className = '';
  export { className as class };
  export let tag = 'div';
  export let label: string;
  export let id = `scroll-area-${++nextId}`;
  export let viewportRole: 'listbox' | undefined = undefined;
  export let viewport: HTMLElement | undefined = undefined;

  let content: HTMLDivElement;
  let rail: HTMLDivElement;
  let overflow = false;
  let percent = 0;
  let thumbHeight = 34;
  let thumbTop = 0;
  let maxScroll = 0;
  let travel = 0;
  let drag: { pointerId: number; offset: number } | null = null;

  function measure() {
    if (!viewport || !rail) return;
    maxScroll = Math.max(0, viewport.scrollHeight - viewport.clientHeight);
    const height = rail.clientHeight;
    overflow = maxScroll > 1 && viewport.clientHeight > 0 && height > 0;
    thumbHeight = Math.min(height, Math.max(34, height * viewport.clientHeight / Math.max(1, viewport.scrollHeight)));
    travel = Math.max(0, height - thumbHeight);
    const ratio = maxScroll > 0 ? Math.min(1, Math.max(0, viewport.scrollTop / maxScroll)) : 0;
    thumbTop = travel * ratio;
    percent = Math.round(ratio * 100);
  }

  function scrollTo(top: number) {
    if (!viewport) return;
    // Mutate through a local reference so Svelte does not re-publish the bound
    // viewport and re-run the parent's selection/focus scroll effects.
    const scroller = viewport;
    scroller.scrollTop = Math.max(0, Math.min(maxScroll, top));
    measure();
  }

  function moveThumb(clientY: number, offset: number) {
    if (travel > 0) scrollTo((clientY - rail.getBoundingClientRect().top - offset) / travel * maxScroll);
  }

  function pointerDown(event: PointerEvent) {
    if (!overflow || event.button !== 0 || !event.isPrimary) return;
    event.preventDefault();
    measure();
    const onThumb = (event.target as Element).closest('.scroll-thumb');
    const offset = onThumb ? event.clientY - rail.getBoundingClientRect().top - thumbTop : thumbHeight / 2;
    drag = { pointerId: event.pointerId, offset };
    rail.setPointerCapture(event.pointerId);
    if (!onThumb) moveThumb(event.clientY, offset);
  }

  function pointerMove(event: PointerEvent) {
    if (drag?.pointerId === event.pointerId) moveThumb(event.clientY, drag.offset);
  }

  function pointerEnd(event: PointerEvent) {
    if (drag?.pointerId !== event.pointerId) return;
    drag = null;
    if (rail.hasPointerCapture(event.pointerId)) rail.releasePointerCapture(event.pointerId);
  }

  function keyDown(event: KeyboardEvent) {
    if (!viewport) return;
    measure();
    const page = viewport.clientHeight * 0.85;
    const positions: Record<string, number> = {
      ArrowUp: viewport.scrollTop - 44, ArrowDown: viewport.scrollTop + 44,
      PageUp: viewport.scrollTop - page, PageDown: viewport.scrollTop + page,
      Home: 0, End: maxScroll,
    };
    if (!(event.key in positions)) return;
    event.preventDefault();
    event.stopPropagation();
    scrollTo(positions[event.key]);
  }

  function wheel(event: WheelEvent) {
    if (!overflow || !viewport || event.ctrlKey) return;
    event.preventDefault();
    const unit = event.deltaMode === 1 ? 16 : event.deltaMode === 2 ? viewport.clientHeight : 1;
    scrollTo(viewport.scrollTop + event.deltaY * unit);
  }

  onMount(() => {
    let frame = 0;
    const schedule = () => {
      cancelAnimationFrame(frame);
      frame = requestAnimationFrame(measure);
    };
    const observer = typeof ResizeObserver === 'undefined' ? null : new ResizeObserver(schedule);
    if (viewport) observer?.observe(viewport);
    observer?.observe(content);
    observer?.observe(rail);
    window.addEventListener('resize', schedule);
    measure();
    schedule();
    return () => {
      cancelAnimationFrame(frame);
      observer?.disconnect();
      window.removeEventListener('resize', schedule);
    };
  });
</script>

<svelte:element this={tag} class={`scroll-area ${className}`} class:scroll-dragging={drag !== null}>
  <div class="scroll-viewport" bind:this={viewport} {id} role={viewportRole} aria-label={viewportRole ? label : undefined} on:scroll={measure}>
    <div class="scroll-content" bind:this={content}><slot /></div>
  </div>
  <!-- The rail is decorative; its focusable thumb exposes the scrollbar interaction. -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="scroll-rail" class:inactive={!overflow} bind:this={rail} aria-hidden={!overflow ? true : undefined}
    on:pointerdown={pointerDown} on:pointermove={pointerMove} on:pointerup={pointerEnd}
    on:pointercancel={pointerEnd} on:lostpointercapture={pointerEnd} on:wheel|nonpassive={wheel}>
    <span class="scroll-ticks" aria-hidden="true">{#each Array(13) as _}<i></i>{/each}</span>
    <span class="scroll-progress" style:transform={`scaleY(${percent / 100})`} aria-hidden="true"></span>
    <div class="scroll-thumb" role="scrollbar" tabindex={overflow ? 0 : -1} aria-label={`${label}滚动条`}
      aria-controls={id} aria-orientation="vertical" aria-valuemin="0" aria-valuemax="100" aria-valuenow={percent}
      style:height={`${thumbHeight}px`} style:transform={`translateY(${thumbTop}px)`} on:keydown={keyDown}>
      <span class="scroll-thumb-face"><i class="scroll-thumb-cap"></i><i class="scroll-thumb-grip"></i></span>
      <span class="scroll-hint" aria-hidden="true">{percent}%</span>
    </div>
  </div>
</svelte:element>

<style>
  .scroll-area { position: relative; display: grid; grid-template-columns: minmax(0, 1fr) 30px; grid-template-rows: minmax(0, 1fr); min-width: 0; min-height: 0; }
  .scroll-viewport { min-width: 0; min-height: 0; overflow-y: auto; overflow-x: hidden; scrollbar-width: none; overscroll-behavior-y: contain; }
  .scroll-viewport::-webkit-scrollbar { display: none; width: 0; height: 0; }
  .scroll-content { display: flow-root; min-height: 100%; padding: var(--scroll-content-padding, 0); }
  .scroll-rail { position: relative; min-height: 0; margin: 8px 5px 8px 7px; touch-action: none; user-select: none; -webkit-user-select: none; cursor: pointer; z-index: 3; }
  .scroll-rail.inactive { visibility: hidden; pointer-events: none; }
  .scroll-rail::before { content: ''; position: absolute; inset-block: 0; right: 3px; width: 1px; background: #d1d8c6; }
  .scroll-ticks { display: flex; position: absolute; inset: 0 3px 0 0; flex-direction: column; justify-content: space-between; pointer-events: none; }
  .scroll-ticks i { display: block; width: 6px; height: 1px; margin-left: auto; background: #bac4ad; }
  .scroll-ticks i:nth-child(3n+1) { width: 11px; background: #9eaa8f; }
  .scroll-progress { position: absolute; right: 3px; top: 0; width: 2px; height: 100%; background: var(--brand-blue, #305ae8); transform-origin: top; pointer-events: none; }
  .scroll-thumb { position: absolute; top: 0; left: -3px; right: -3px; cursor: grab; touch-action: none; outline: none; }
  .scroll-dragging .scroll-thumb { cursor: grabbing; }
  .scroll-thumb:focus-visible { outline: 2px solid #849eff; outline-offset: 0; border-radius: 3px; }
  .scroll-thumb-face { position: absolute; inset-block: 0; left: calc(50% - 5px); width: 10px; overflow: hidden; border-radius: 3px 3px 2px 2px; background: var(--brand-blue, #305ae8); }
  .scroll-thumb-cap { position: absolute; inset-inline: 0; bottom: 0; height: 9px; box-sizing: content-box; border-top: 3px solid var(--scroll-surface, var(--white, #fffef9)); background: #f2cb4b; }
  .scroll-thumb-grip { position: absolute; left: 3px; right: 3px; top: calc(50% - 4px); height: 2px; background: #fffef980; box-shadow: 0 5px #fffef980; }
  .scroll-rail:hover .scroll-thumb-face, .scroll-dragging .scroll-thumb-face { background: #2448c5; }
  .scroll-hint { position: absolute; right: 26px; top: 50%; transform: translateY(-50%); white-space: nowrap; padding: 5px 7px; border-radius: 4px; background: #30392b; color: #fffef9; font-size: 12px; font-variant-numeric: tabular-nums; pointer-events: none; opacity: 0; transition: opacity 120ms ease; }
  .scroll-rail:hover .scroll-hint, .scroll-thumb:focus-visible .scroll-hint, .scroll-dragging .scroll-hint { opacity: 1; }
  @media (prefers-reduced-motion: reduce) { .scroll-hint { transition: none; } }
  @media (forced-colors: active) {
    .scroll-area { grid-template-columns: minmax(0, 1fr); }
    .scroll-viewport { scrollbar-width: auto; }
    .scroll-viewport::-webkit-scrollbar { display: block; width: auto; height: auto; }
    .scroll-rail { display: none; }
  }
</style>
