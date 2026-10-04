<script lang="ts">
  import { onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { api } from "../lib/api";

  let { device, gain }: { device: string | null; gain: number } = $props();

  const BARS = 14;
  let bars = $state<number[]>(Array(BARS).fill(0));

  onMount(() => {
    const unlisten = listen<{ rms: number }>("mic-level", (e) => {
      // Speech sits low on a linear scale: a square root shows quiet talk too.
      bars = [...bars.slice(1), Math.min(1, Math.sqrt(e.payload.rms) * 1.6)];
    });
    return () => {
      unlisten.then((f) => f());
      api.micMonitor(false);
    };
  });

  // Reading them here restarts the meter when the microphone or the gain changes.
  $effect(() => {
    device;
    gain;
    api.micMonitor(true);
  });
</script>

<div class="meter" aria-hidden="true">
  {#each bars as level}
    <span style:--h={Math.max(0.08, level)} class:hot={level > 0.85}></span>
  {/each}
</div>

<style>
  .meter {
    display: flex;
    align-items: center;
    gap: 2px;
    width: 64px;
    height: 22px;
  }

  span {
    flex: 1;
    min-height: 2px;
    height: calc(var(--h) * 100%);
    border-radius: 2px;
    background: var(--accent);
    opacity: 0.85;
  }

  span.hot {
    opacity: 1;
    box-shadow: 0 0 6px var(--accent-glow);
  }

  @media (prefers-reduced-motion: no-preference) {
    span {
      transition: height 0.07s linear;
    }
  }
</style>
