<script lang="ts">
  export let size = 22;
  export let inverse = false;
</script>

<svg class="block-loader" class:inverse width={size} height={size} viewBox="0 0 24 24" fill="none" aria-hidden="true" focusable="false">
  <path class="piece blue" d="M3 3h9v3a2.25 2.25 0 0 0 0 4.5V21H3Z" />
  <path class="piece yellow" d="M12 3h9v9h-9v-1.5a2.25 2.25 0 0 1 0-4.5Z" />
  <path class="piece red" d="M12 12h2.25a2.25 2.25 0 0 1 4.5 0H21v9h-9Z" />
</svg>

<style>
  .block-loader { display: inline-block; flex: none; vertical-align: middle; overflow: visible; pointer-events: none; }
  .piece { transform-box: fill-box; transform-origin: center; animation-duration: 1200ms; animation-timing-function: var(--ease-out); animation-iteration-count: infinite; }
  .blue { fill: var(--brand-blue); animation-name: assemble-blue; }
  .yellow { fill: var(--brand-yellow); animation-name: assemble-yellow; }
  .red { fill: var(--brand-red); animation-name: assemble-red; }
  .inverse .blue { fill: var(--white); }

  /* Each piece docks in 180 ms, staggered by 72 ms. The remaining loop is a pause,
     then a 180 ms reset; completing the operation unmounts the loader immediately. */
  @keyframes assemble-blue {
    0%, 90%, 100% { transform: translate(-30%, 10%) rotate(-6deg); opacity: .3; }
    15%, 75% { transform: translate(0, 0) rotate(0); opacity: 1; }
  }
  @keyframes assemble-yellow {
    0%, 6%, 90%, 100% { transform: translate(10%, -40%) rotate(6deg); opacity: .3; }
    21%, 75% { transform: translate(0, 0) rotate(0); opacity: 1; }
  }
  @keyframes assemble-red {
    0%, 12%, 90%, 100% { transform: translate(35%, 25%) rotate(6deg); opacity: .3; }
    27%, 75% { transform: translate(0, 0) rotate(0); opacity: 1; }
  }

  @media (prefers-reduced-motion: reduce) {
    .piece { animation: none !important; transform: none; opacity: 1; }
    .block-loader { animation: loading-breathe 1200ms linear infinite !important; }
  }
  @keyframes loading-breathe {
    0%, 100% { opacity: .72; }
    50% { opacity: 1; }
  }
</style>
