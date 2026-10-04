<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { api, detailLines } from "./lib/api";
  import { locale, t as tr } from "./lib/i18n.svelte";

  type State =
    | { state: "recognizing" | "loading" | "done" | "empty" }
    | { state: "listening"; translate: boolean }
    | { state: "fetching"; id: string }
    | { state: "fetched"; error: string | null }
    | { state: "unsent"; text: string; reason: string; copied: "elevated" | "no_access" | null }
    | { state: "error"; key: "microphone" | "model" | "recognition"; detail: string };

  const BARS = 28;
  const mac = navigator.userAgent.includes("Mac");

  let current = $state<State>({ state: "listening", translate: false });
  let fetched = $state(0);
  let levels = $state<number[]>(Array(BARS).fill(0));
  let elapsed = $state(0);
  let expanded = $state(false);

  const failure = $derived.by(() => {
    if (current.state === "error") return { title: t(current.key), detail: current.detail };
    if (current.state === "fetched" && current.error) return { title: t("fetch_failed"), detail: current.error };
    return null;
  });

  const labels = {
    en: {
      listening: "Listening",
      recognizing: "Recognizing…",
      loading: "Loading the model…",
      fetching: "Downloading the translation model",
      fetched: "Translation is ready, press the hotkey again",
      fetch_failed: "Translation model download failed",
      done: "Done",
      empty: "Nothing heard",
      unsent: "Not pasted",
      elevated: "Window runs as administrator",
      no_access: "No permission to paste",
      on_clipboard: "Text is on the clipboard, press Ctrl+V",
      on_clipboard_mac: "Text is on the clipboard, press ⌘V",
      copy: "Copy",
      microphone: "Microphone unavailable",
      model: "Model not found",
      recognition: "Recognition failed",
    },
    ru: {
      listening: "Слушаю",
      recognizing: "Распознаю…",
      loading: "Загружаю модель…",
      fetching: "Скачиваю модель перевода",
      fetched: "Перевод готов, нажмите хоткей ещё раз",
      fetch_failed: "Модель перевода не скачалась",
      done: "Готово",
      empty: "Ничего не услышал",
      unsent: "Не вставилось",
      elevated: "Окно от администратора",
      no_access: "Нет разрешения на вставку",
      on_clipboard: "Текст в буфере, нажмите Ctrl+V",
      on_clipboard_mac: "Текст в буфере, нажмите ⌘V",
      copy: "Скопировать",
      microphone: "Микрофон недоступен",
      model: "Модель не найдена",
      recognition: "Ошибка распознавания",
    },
  };
  const t = (key: keyof (typeof labels)["en"]) => labels[locale.lang][key];

  onMount(() => {
    api.uiLanguage().then((lang) => (locale.lang = lang));
    // The window may have been created for this very recording, after the first event.
    invoke<State | null>("overlay_state").then((state) => state && (current = state));
    const unlisten = [
      listen<State>("overlay", (e) => {
        current = e.payload;
        expanded = false;
        if (e.payload.state === "listening") {
          levels = Array(BARS).fill(0);
          elapsed = 0;
          api.uiLanguage().then((lang) => (locale.lang = lang));
        }
      }),
      listen<{ id: string; done: number; total: number }>("model-progress", (e) => {
        if (current.state === "fetching" && current.id === e.payload.id) {
          fetched = e.payload.done / e.payload.total;
        }
      }),
      listen<{ level: number; elapsed_ms: number }>("overlay-level", (e) => {
        // Speech peaks sit low on a linear scale, a square root makes quiet talk visible.
        levels = [...levels.slice(1), Math.sqrt(e.payload.level)];
        elapsed = e.payload.elapsed_ms;
      }),
    ];
    return () => unlisten.forEach((u) => u.then((f) => f()));
  });

  async function expand() {
    await api.expandOverlay();
    expanded = true;
  }

  function clock(ms: number): string {
    const s = Math.floor(ms / 1000);
    return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`;
  }
</script>

{#if failure && expanded}
  <div class="card">
    <div class="card-head">
      <span class="mark err">!</span>
      <span class="title">{failure.title}</span>
      <button class="close" aria-label={tr("close")} onclick={() => api.dismissOverlay()}>✕</button>
    </div>
    <pre class="detail">{detailLines(failure.detail)}</pre>
    <div class="actions">
      <button onclick={() => failure && api.copy(failure.detail)}>{tr("copy")}</button>
      <button onclick={() => api.openLogs()}>{tr("open_logs")}</button>
    </div>
  </div>
{:else}
<div class="pill" class:wide={current.state === "unsent" || failure}>
  {#if failure}
    <span class="mark err">!</span>
    <div class="unsent">
      <span class="title small-text line">{failure.title}</span>
      <button class="more" onclick={expand}>{tr("details")}</button>
    </div>
    <button class="close" aria-label={tr("close")} onclick={() => api.dismissOverlay()}>✕</button>
  {:else if current.state === "listening"}
    <span class="dot"></span>
    {#if current.translate}
      <span class="badge">RU→EN</span>
    {/if}
    <div class="wave">
      {#each levels as level}
        <span style="height: {Math.max(2, level * 26)}px"></span>
      {/each}
    </div>
    <span class="time">{clock(elapsed)}</span>
  {:else if current.state === "recognizing" || current.state === "loading"}
    <span class="spinner"></span>
    <span>{t(current.state)}</span>
  {:else if current.state === "fetching"}
    <span class="spinner"></span>
    <span>{t("fetching")} {Math.round(fetched * 100)}%</span>
  {:else if current.state === "fetched"}
    <span class="mark ok">✓</span>
    <span class="small-text">{t("fetched")}</span>
  {:else if current.state === "done"}
    <span class="mark ok">✓</span>
    <span>{t("done")}</span>
  {:else if current.state === "empty"}
    <span class="muted">{t("empty")}</span>
  {:else if current.state === "unsent" && current.copied}
    <div class="unsent">
      <span class="title">{t(current.copied)}</span>
      <span class="preview">{t(mac ? "on_clipboard_mac" : "on_clipboard")}</span>
    </div>
    <button class="close" aria-label={tr("close")} onclick={() => api.dismissOverlay()}>✕</button>
  {:else if current.state === "unsent"}
    <div class="unsent">
      <span class="title">{t("unsent")}</span>
      <span class="preview" title={current.reason}>{current.text}</span>
    </div>
    <button onclick={() => current.state === "unsent" && api.copyAndDismiss(current.text)}>{t("copy")}</button>
    <button class="close" aria-label={tr("close")} onclick={() => api.dismissOverlay()}>✕</button>
  {/if}
</div>
{/if}

<style>
  :global(html),
  :global(body) {
    margin: 0;
    height: 100%;
    background: transparent;
    overflow: hidden;
    font: 13px/1.3 "Golos Text Variable", system-ui, "Segoe UI", sans-serif;
    user-select: none;
    cursor: default;
  }

  :global(#overlay) {
    height: 100%;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .pill {
    display: flex;
    align-items: center;
    gap: 10px;
    height: 44px;
    padding: 0 16px;
    border-radius: 22px;
    background: rgba(24, 24, 27, 0.92);
    color: #f4f4f5;
    box-shadow: 0 0 0 1px rgba(255, 255, 255, 0.08);
    max-width: 296px;
  }

  .pill.wide {
    padding-right: 8px;
  }

  .card {
    display: flex;
    flex-direction: column;
    gap: 8px;
    box-sizing: border-box;
    width: 296px;
    height: calc(100% - 8px);
    padding: 10px 8px 10px 14px;
    border-radius: 14px;
    background: rgba(24, 24, 27, 0.95);
    color: #f4f4f5;
    box-shadow: 0 0 0 1px rgba(255, 255, 255, 0.08);
  }

  .card-head {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .card-head .title {
    flex: 1;
  }

  .detail {
    flex: 1;
    margin: 0;
    padding-right: 6px;
    overflow: auto;
    font: 11px/1.4 ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
    color: #d4d4d8;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    user-select: text;
  }

  .actions {
    display: flex;
    gap: 6px;
  }

  .line {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  button.more {
    align-self: flex-start;
    background: none;
    color: #a1a1aa;
    padding: 0;
    font-size: 12px;
    text-decoration: underline;
  }

  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: #ff5c48;
    animation: pulse 1.2s ease-in-out infinite;
  }

  .wave {
    display: flex;
    align-items: center;
    gap: 2px;
    height: 28px;
  }

  .wave span {
    width: 3px;
    border-radius: 2px;
    background: #f4f4f5;
    transition: height 60ms linear;
  }

  .badge {
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.02em;
    color: #18181b;
    background: #f4f4f5;
    border-radius: 4px;
    padding: 1px 5px;
  }

  .small-text {
    font-size: 12px;
  }

  .time {
    font-variant-numeric: tabular-nums;
    color: #a1a1aa;
    min-width: 32px;
  }

  .spinner {
    width: 14px;
    height: 14px;
    border: 2px solid rgba(255, 255, 255, 0.25);
    border-top-color: #f4f4f5;
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
  }

  .mark {
    font-weight: 700;
  }

  .ok {
    color: #4ade80;
  }

  .err {
    color: #ff5c48;
    width: 18px;
    text-align: center;
  }

  .muted {
    color: #a1a1aa;
  }

  .unsent {
    display: flex;
    flex-direction: column;
    min-width: 0;
    flex: 1;
  }

  .title {
    font-weight: 600;
  }

  .preview {
    color: #a1a1aa;
    font-size: 12px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  button {
    font: inherit;
    font-size: 12px;
    color: #18181b;
    background: #f4f4f5;
    border: none;
    border-radius: 14px;
    padding: 5px 12px;
    flex-shrink: 0;
  }

  button.close {
    background: none;
    color: #a1a1aa;
    padding: 5px 6px;
  }

  @keyframes pulse {
    50% {
      opacity: 0.35;
    }
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
</style>
