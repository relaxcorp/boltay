<script lang="ts">
  import type { Section } from "../lib/store.svelte";

  let { section }: { section: Section } = $props();
</script>

<svg
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  stroke-width="1.8"
  stroke-linecap="round"
  stroke-linejoin="round"
  aria-hidden="true"
>
  {#if section === "general"}
    <path d="M4 7h9M17 7h3M4 17h3M11 17h9" />
    <circle cx="15" cy="7" r="2" />
    <circle cx="9" cy="17" r="2" />
  {:else if section === "models"}
    <path d="M12 3l8 4.5v9L12 21l-8-4.5v-9z" />
    <path d="M4 7.5l8 4.5 8-4.5M12 12v9" />
  {:else if section === "text"}
    <path d="M5 7V5h14v2M12 5v14M9 19h6" />
  {:else if section === "transcribe"}
    <path d="M4 10v4M8 7v10M12 4v16M16 8v8M20 11v2" />
  {:else if section === "history"}
    <circle cx="12" cy="12" r="8" />
    <path d="M12 8v4l3 2" />
  {:else}
    <path d="M12 3.5c-3.4 0-6 4.9-6 9.1a6 6 0 0012 0c0-4.2-2.6-9.1-6-9.1z" />
    <path d="M8.6 13.4l1.6-1.4 1.3 1.6 1.5-1.8 1.3 1.6 1.1-1" />
  {/if}
</svg>

<style>
  svg {
    width: 18px;
    height: 18px;
    flex: none;
  }
</style>
