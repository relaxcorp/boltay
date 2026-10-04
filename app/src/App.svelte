<script lang="ts">
  import { onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { store, type Section } from "./lib/store.svelte";
  import { t } from "./lib/i18n.svelte";
  import General from "./sections/General.svelte";
  import Models from "./sections/Models.svelte";
  import Text from "./sections/Text.svelte";
  import History from "./sections/History.svelte";
  import About from "./sections/About.svelte";
  import Transcribe from "./sections/Transcribe.svelte";
  import Onboarding from "./sections/Onboarding.svelte";
  import NavIcon from "./components/NavIcon.svelte";
  import { getCurrentWebview } from "@tauri-apps/api/webview";
  import { api } from "./lib/api";

  const sections: Section[] = ["general", "models", "text", "history", "transcribe", "about"];
  let dragging = $state(false);
  let onboarding = $state(false);
  let buttons = $state<Record<string, HTMLButtonElement>>({});
  let marker = $state({ top: 0, height: 0 });
  let placed = $state(false);

  // The glowing marker slides to the chosen item instead of jumping, except on first paint.
  $effect(() => {
    const button = buttons[store.section];
    if (!button) return;
    marker = { top: button.offsetTop, height: button.offsetHeight };
    requestAnimationFrame(() => (placed = true));
  });

  onMount(() => {
    const initial = location.hash.slice(1) as Section;
    if (sections.includes(initial)) store.section = initial;
    // Also when dictation cannot work yet: a permission missing on macOS.
    Promise.all([store.load(), api.permissions()]).then(([, p]) => {
      const missing = p.needed && (p.microphone !== "granted" || p.accessibility !== "granted");
      onboarding = (store.info?.first_run ?? false) || missing;
    });
    const unlisten = [
      listen("permissions", () => (onboarding = true)),
      listen<Section>("navigate", (e) => {
        onboarding = false;
        store.section = e.payload;
      }),
      listen("settings-changed", () => store.load()),
      // An audio file dropped anywhere on the window goes to Transcription.
      getCurrentWebview().onDragDropEvent((e) => {
        const kind = e.payload.type;
        dragging = kind === "enter" || kind === "over";
        if (kind === "drop" && e.payload.paths.length) {
          onboarding = false;
          store.section = "transcribe";
          api.transcribePath(e.payload.paths[0]).catch((err) => (store.error = String(err)));
        }
      }),
    ];
    return () => unlisten.forEach((u) => u.then((f) => f()));
  });
</script>

{#if store.settings && store.info}
  {#if onboarding}
    <Onboarding done={() => (onboarding = false)} />
  {:else}
    <div class="layout">
      <nav>
        <div class="brand">
          <img src="/icon.png" alt="" width="36" height="36" />
          <span>Boltay</span>
        </div>
        <div
          class="marker"
          class:placed
          style:transform="translateY({marker.top}px)"
          style:height="{marker.height}px"
        ></div>
        {#each sections as s}
          <button
            bind:this={buttons[s]}
            class:active={store.section === s}
            onclick={() => (store.section = s)}
          >
            <NavIcon section={s} />
            {t(`nav.${s}`)}
          </button>
        {/each}
        <div class="status">
          {#if store.error}
            <span class="error" title={store.error}>{store.error}</span>
          {/if}
        </div>
      </nav>
      <main>
        {#key store.section}
          <div class="page">
            {#if store.section === "general"}
              <General />
            {:else if store.section === "models"}
              <Models />
            {:else if store.section === "text"}
              <Text />
            {:else if store.section === "history"}
              <History />
            {:else if store.section === "transcribe"}
              <Transcribe {dragging} />
            {:else}
              <About />
            {/if}
          </div>
        {/key}
      </main>
    </div>
  {/if}
{/if}

<style>
  .layout {
    display: flex;
    height: 100%;
  }

  nav {
    position: relative;
    width: 190px;
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
    gap: 3px;
    padding: 18px 12px;
    background:
      linear-gradient(180deg, rgba(255, 255, 255, 0.02), transparent),
      var(--sidebar);
    border-right: 1px solid var(--line);
  }

  .brand {
    display: flex;
    align-items: center;
    gap: 10px;
    height: 40px;
    margin: 0 0 16px 2px;
    font-size: 17px;
    font-weight: 650;
    letter-spacing: -0.02em;
  }

  .brand img {
    border-radius: 9px;
  }

  nav button {
    position: relative;
    z-index: 1;
    display: flex;
    align-items: center;
    gap: 11px;
    padding: 8px 12px;
    border-radius: 10px;
    text-align: left;
    background: none;
    box-shadow: none;
    transform: none;
    color: var(--muted);
    font-weight: 500;
  }

  nav button:hover:not(:disabled) {
    color: var(--text);
    filter: none;
  }

  nav button:active:not(:disabled) {
    transform: none;
    box-shadow: none;
  }

  nav button.active {
    color: var(--text);
  }

  nav button.active :global(svg) {
    color: var(--accent);
    filter: drop-shadow(0 0 6px var(--accent-glow));
  }

  .marker {
    position: absolute;
    left: 12px;
    right: 12px;
    top: 0;
    border-radius: 10px;
    pointer-events: none;
    background: linear-gradient(90deg, rgba(255, 92, 72, 0.17), rgba(255, 92, 72, 0.03) 70%);
  }

  .marker::before {
    content: "";
    position: absolute;
    left: -12px;
    top: 9px;
    bottom: 9px;
    width: 3px;
    border-radius: 0 3px 3px 0;
    background: var(--accent);
    box-shadow: 0 0 12px 1px var(--accent-glow);
  }

  @media (prefers-reduced-motion: no-preference) {
    nav button {
      transition: color 0.2s;
    }

    .marker.placed {
      transition:
        transform 0.32s cubic-bezier(0.3, 0.8, 0.25, 1),
        height 0.32s;
    }
  }

  .status {
    margin-top: auto;
    font-size: 12px;
    padding: 0 10px;
  }

  .error {
    color: var(--accent);
    display: -webkit-box;
    -webkit-line-clamp: 4;
    line-clamp: 4;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }

  main {
    flex: 1;
    overflow-y: auto;
    padding: 26px 30px 34px;
  }
</style>
