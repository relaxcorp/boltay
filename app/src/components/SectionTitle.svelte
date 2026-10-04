<script lang="ts">
  import type { Snippet } from "svelte";
  import { HEIGHT, wave, type Theme } from "../lib/wave";

  let { theme, children }: { theme: Theme; children: Snippet } = $props();

  const shape = $derived(wave(theme));
  // Drawn at its real width, one unit per pixel: a stretched line would change its rhythm.
  let width = $state(0);
  let line = $state<SVGPathElement>();
  let length = $state(0);

  $effect(() => {
    if (line) length = Math.ceil(line.getTotalLength());
  });
</script>

<h2>
  <span class="title">{@render children()}</span>
  <span class="track" bind:clientWidth={width}>
    {#if width}
      <svg
        width={width}
        height={HEIGHT}
        viewBox="0 0 {width} {HEIGHT}"
        data-level={shape.level}
        style:--len={length}
        aria-hidden="true"
      >
        <line class="base" x1="0" y1={HEIGHT / 2} x2={width} y2={HEIGHT / 2} />
        <path class="line" d={shape.d} bind:this={line} />
        {#if shape.bar}
          <rect class="bleep" x={shape.bar[0]} y="4" width={shape.bar[1]} height="7" rx="2" />
        {/if}
      </svg>
    {/if}
  </span>
</h2>

<style>
  h2 {
    display: flex;
    align-items: center;
    gap: 12px;
    margin: 30px 0 12px 2px;
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: var(--muted);
    white-space: nowrap;
  }

  h2:first-child {
    margin-top: 2px;
  }

  .track {
    flex: 1 1 0;
    width: 0;
    min-width: 0;
    height: 15px;
  }

  svg {
    display: block;
    overflow: visible;
    shape-rendering: geometricPrecision;
  }

  .base {
    stroke: var(--line);
    stroke-width: 1;
  }

  .line {
    fill: none;
    stroke: var(--accent);
    stroke-width: 1.3;
    stroke-linejoin: round;
    stroke-linecap: round;
  }

  .bleep {
    fill: var(--accent);
    opacity: 0.9;
  }

  [data-level="1"] .line {
    opacity: 0.55;
    stroke-width: 1.1;
  }

  [data-level="2"] .line {
    opacity: 0.85;
  }

  [data-level="3"] .line {
    stroke-width: 1.5;
    filter: drop-shadow(0 0 3px var(--accent-glow));
  }

  @media (prefers-reduced-motion: no-preference) {
    .line {
      stroke-dasharray: var(--len);
      stroke-dashoffset: var(--len);
      animation: draw 1s 0.12s cubic-bezier(0.45, 0.05, 0.2, 1) forwards;
    }

    .base {
      transform-origin: 0 50%;
      animation: grow 0.9s 0.2s cubic-bezier(0.3, 0.8, 0.3, 1) both;
    }

    .bleep {
      opacity: 0;
      transform-box: fill-box;
      transform-origin: center;
      animation: bleep 0.3s 1s ease-out forwards;
    }
  }

  @keyframes draw {
    to {
      stroke-dashoffset: 0;
    }
  }

  @keyframes grow {
    from {
      transform: scaleX(0);
    }
  }

  @keyframes bleep {
    from {
      opacity: 0;
      transform: scaleX(0.4);
    }
    to {
      opacity: 0.9;
    }
  }
</style>
