<script lang="ts">
  import { onMount, tick } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { api, megabytes, plain, type Direction, type ModelId, type Opened, type Outcome, type Piece } from "./lib/api";
  import { locale, t } from "./lib/i18n.svelte";

  type Mode = "selection" | "typing" | "reply";

  let opened = $state<Opened | null>(null);
  let mode = $state<Mode>("typing");
  let input = $state("");
  let outcome = $state<Outcome | null>(null);
  let busy = $state(false);
  let error = $state("");
  let showOriginal = $state(false);
  let copied = $state(false);
  /// Download progress by model, from 0 to 1.
  let progress = $state<Partial<Record<ModelId, number>>>({});
  let plate: HTMLDivElement;
  let field = $state<HTMLTextAreaElement>();
  let timer: ReturnType<typeof setTimeout> | undefined;
  /// Results of older requests that come back late are dropped.
  let request = 0;

  const done = $derived(outcome?.state === "done" ? outcome : null);
  const translation = $derived(done ? plain(done.translation) : "");

  async function run(text: string, direction: Direction | null) {
    const id = ++request;
    error = "";
    if (!text.trim()) {
      outcome = null;
      return;
    }
    busy = true;
    try {
      const result = await api.translate(text, direction);
      if (id === request) outcome = result;
    } catch (e) {
      if (id === request) error = String(e);
    }
    if (id === request) busy = false;
  }

  function open(o: Opened) {
    opened = o;
    showOriginal = false;
    copied = false;
    outcome = null;
    error = "";
    progress = {};
    if (o.text) {
      mode = "selection";
      input = "";
      run(o.text, null);
    } else {
      mode = "typing";
      input = "";
      tick().then(() => field?.focus());
    }
  }

  // A field translates as the user types, a moment after the last key.
  function typed() {
    clearTimeout(timer);
    timer = setTimeout(() => run(input, mode === "reply" ? "ru_en" : null), 350);
  }

  function reply() {
    if (mode === "reply") {
      mode = "selection";
      if (opened?.text) run(opened.text, null);
    } else {
      mode = "reply";
      input = "";
      outcome = null;
      api.translatorKeyboard().then(() => tick()).then(() => field?.focus());
    }
  }

  function retry() {
    if (mode === "selection" && opened?.text) run(opened.text, null);
    else run(input, mode === "reply" ? "ru_en" : null);
  }

  async function fetchModel(quality: boolean) {
    if (outcome?.state !== "need_model") return;
    try {
      await api.translatorFetch(outcome.direction, quality);
      retry();
    } catch (e) {
      error = String(e);
    }
  }

  async function copy() {
    await api.copy(translation);
    copied = true;
    setTimeout(() => (copied = false), 1300);
  }

  async function insert() {
    try {
      await api.translatorInsert(translation);
    } catch (e) {
      error = String(e);
    }
  }

  const close = () => api.translatorClose();

  const tip = (p: Piece): string => {
    if (p.mark === "low_confidence") return t("translator.tip_doubt");
    if (p.mark === "rare") return t("translator.tip_rare");
    return `${p.text} — ${p.note ?? ""}`;
  };

  const percent = (id: ModelId) => Math.round((progress[id] ?? 0) * 100);

  const heading = $derived.by(() => {
    if (mode === "typing" && !done) return [t("translator.translation"), t("translator.typing")];
    const direction = done?.direction ?? (mode === "reply" ? "ru_en" : "en_ru");
    return [mode === "reply" ? t("translator.reply") : t("translator.translation"), t(`translator.${direction}`)];
  });

  // The window follows the plate's height.
  $effect(() => {
    outcome;
    showOriginal;
    mode;
    error;
    busy;
    progress;
    tick().then(() => {
      if (plate) api.translatorFit(plate.offsetHeight + 34);
    });
  });

  // The plate moves by its header, the close button aside.
  function drag(e: MouseEvent) {
    if (e.button !== 0 || (e.target as HTMLElement).closest("button")) return;
    e.preventDefault();
    api.translatorMoved();
    getCurrentWindow().startDragging();
  }

  onMount(() => {
    api.uiLanguage().then((lang) => (locale.lang = lang));
    api.translatorOpened().then((o) => o && open(o));
    const keydown = (e: KeyboardEvent) => {
      if (e.key === "Escape") close();
    };
    window.addEventListener("keydown", keydown);
    const unlisten = [
      listen<Opened>("translator-open", (e) => {
        api.uiLanguage().then((lang) => (locale.lang = lang));
        open(e.payload);
      }),
      listen<{ id: ModelId; done: number; total: number }>("model-progress", (e) => {
        progress[e.payload.id] = e.payload.done / Math.max(1, e.payload.total);
      }),
      // A model that just arrived is used right away.
      listen<{ id: ModelId; error: string | null }>("model-done", (e) => {
        delete progress[e.payload.id];
        if (e.payload.error) error = e.payload.error;
        else retry();
      }),
    ];
    return () => {
      window.removeEventListener("keydown", keydown);
      unlisten.forEach((u) => u.then((f) => f()));
    };
  });
</script>

{#snippet marked(pieces: Piece[])}
  {#each pieces as p}{#if p.mark === "low_confidence"}<span class="doubt" data-tip={tip(p)}>{p.text}</span>{:else if p.mark === "rare"}<span class="rare" data-tip={tip(p)}>{p.text}</span>{:else if p.mark}<span class="slang" data-tip={tip(p)}>{p.text}</span>{:else}{p.text}{/if}{/each}
{/snippet}

<div class="plate" bind:this={plate}>
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="head" onmousedown={drag}>
    <img src="/icon.png" alt="" />
    <span class="dir">{heading[0]} · <b>{heading[1]}</b></span>
    <button class="x ghost" title={t("translator.close")} onclick={close}>✕</button>
  </div>

  <div class="body">
    {#if mode !== "selection"}
      <textarea
        bind:this={field}
        bind:value={input}
        oninput={typed}
        placeholder={mode === "reply" ? t("translator.reply_placeholder") : t("translator.placeholder")}
      ></textarea>
    {/if}

    {#if error}
      <p class="error">{t("translator.failed")}: {error}</p>
    {:else if outcome?.state === "need_model"}
      <p class="need">{t("translator.need_model")}</p>
      <div class="choices">
        <button onclick={() => fetchModel(false)}>{t("translator.base_model", { size: megabytes(outcome.base) })}</button>
        <button class="primary" onclick={() => fetchModel(true)}>
          {t("translator.big_model", { size: megabytes(outcome.big) })}
        </button>
      </div>
    {:else if outcome?.state === "fetching"}
      <p class="muted">{t("translator.fetching", { n: percent(outcome.id) })}</p>
      <div class="bar"><div style:width="{percent(outcome.id)}%"></div></div>
    {:else if done}
      <p class="tr">{@render marked(done.translation)}</p>
      {#if done.chips.length && mode !== "reply"}
        <div class="chips">
          {#each done.chips as chip}
            <span>{chip.term} — {chip.meaning}</span>
          {/each}
        </div>
      {/if}
      {#if mode !== "reply" && done.translation.some((p) => p.mark) || done.chips.length}
        <p class="legend"><i>{t("translator.legend_doubt")}</i> · <u>{t("translator.legend_slang")}</u></p>
      {/if}
      {#if done.upgrading}
        <p class="muted small">{t("translator.upgrading", { n: percent(done.upgrading) })}</p>
      {/if}
      {#if done.cut}
        <p class="muted small">{t("translator.cut")}</p>
      {/if}
      {#if mode === "selection"}
        <button class="toggle ghost" onclick={() => (showOriginal = !showOriginal)}>
          {showOriginal ? "▾" : "▸"} {t("translator.original")}
        </button>
        {#if showOriginal}
          <p class="orig">{@render marked(done.original)}</p>
        {/if}
      {/if}
    {:else if busy}
      <p class="muted">{t("translator.translating")}</p>
    {:else if mode !== "selection"}
      <p class="muted">{t("translator.waiting")}</p>
    {/if}
  </div>

  {#if done || mode !== "selection"}
  <div class="foot">
    <button class="primary" disabled={!translation} onclick={copy}>{t("translator.copy")}</button>
    {#if mode === "selection" || mode === "reply"}
      {#if opened?.text && (mode === "reply" || done?.direction === "en_ru")}
        <button onclick={reply}>{mode === "reply" ? t("translator.back") : t("translator.answer")}</button>
      {/if}
      <!-- Half a translation in place of the whole selection would lose the rest of it. -->
      {#if !done?.cut}
        <button disabled={!translation} onclick={insert}>
          {mode === "reply" ? t("translator.insert") : t("translator.replace")}
        </button>
      {/if}
    {/if}
    <span class="copied" class:on={copied}>{t("translator.copied")}</span>
  </div>
  {/if}
</div>

<style>
  :global(html:has(body.translator)),
  :global(body.translator) {
    background: transparent;
    overflow: hidden;
  }

  .plate {
    position: relative;
    width: 440px;
    margin: 6px 15px 28px;
    border-radius: 18px;
    background: var(--plate);
    box-shadow:
      inset 0 1px 0 var(--plate-hi),
      0 0 0 1px var(--line),
      0 4px 0 var(--plate-edge),
      0 18px 26px -14px rgba(0, 0, 0, 0.75),
      0 0 30px -16px var(--accent-glow);
  }

  .head {
    cursor: grab;
    user-select: none;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 10px 9px 16px;
    border-bottom: 1px solid var(--line);
  }

  .head img {
    width: 20px;
    height: 20px;
    border-radius: 5px;
  }

  .dir {
    font-size: 12px;
    color: var(--muted);
    letter-spacing: 0.04em;
  }

  .dir b {
    color: var(--text);
    font-weight: 600;
    letter-spacing: 0;
  }

  .x {
    margin-left: auto;
    width: 28px;
    height: 28px;
    padding: 0;
    border-radius: 8px;
  }

  .body {
    padding: 14px 16px 6px;
    user-select: text;
  }

  .tr {
    font-size: 16px;
    line-height: 1.55;
    margin: 0 0 12px;
    cursor: text;
  }

  textarea {
    width: 100%;
    min-height: 84px;
    resize: none;
    margin-bottom: 12px;
  }

  .doubt,
  .slang,
  .rare {
    position: relative;
    cursor: help;
  }

  .doubt {
    text-decoration: underline wavy rgba(255, 92, 72, 0.85);
    text-decoration-thickness: 1.5px;
    text-underline-offset: 4px;
  }

  .slang {
    border-bottom: 1.5px dashed rgba(245, 181, 68, 0.9);
  }

  .rare {
    border-bottom: 1.5px dotted var(--muted);
  }

  .doubt:hover::after,
  .slang:hover::after,
  .rare:hover::after {
    content: attr(data-tip);
    position: absolute;
    left: 0;
    bottom: calc(100% + 8px);
    z-index: 5;
    width: max-content;
    max-width: 260px;
    padding: 7px 10px;
    border-radius: 10px;
    font-size: 12px;
    line-height: 1.4;
    white-space: normal;
    color: var(--text);
    background: var(--field);
    box-shadow:
      0 0 0 1px var(--line),
      0 10px 24px -8px rgba(0, 0, 0, 0.8);
  }

  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin: 0 0 12px;
  }

  .chips span {
    font-size: 11.5px;
    padding: 3px 8px;
    border-radius: 999px;
    background: rgba(245, 181, 68, 0.1);
    color: var(--warn);
    box-shadow: inset 0 0 0 1px rgba(245, 181, 68, 0.3);
  }

  .toggle {
    padding: 0;
    margin-bottom: 8px;
    font-size: 12px;
  }

  .orig {
    margin: 0 0 10px;
    padding: 10px 12px;
    border-radius: 12px;
    background: var(--field);
    box-shadow:
      var(--field-shadow),
      0 0 0 1px var(--line);
    font-size: 13px;
    color: var(--muted);
  }

  .need {
    font-size: 15px;
    margin: 4px 0 12px;
  }

  .choices {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    margin-bottom: 12px;
  }

  .bar {
    height: 6px;
    margin: 8px 0 14px;
    border-radius: 3px;
    background: var(--switch-off);
    overflow: hidden;
  }

  .bar div {
    height: 100%;
    background: var(--yolk);
  }

  .error {
    color: var(--accent);
    margin-bottom: 12px;
  }

  .foot {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 10px 8px;
    padding: 10px 14px 14px;
    border-top: 1px solid var(--line);
  }

  .foot button {
    white-space: nowrap;
    padding: 6px 12px;
    font-size: 13px;
  }

  .copied {
    font-size: 12px;
    color: var(--ok);
    opacity: 0;
    transition: opacity 0.2s;
  }

  .copied.on {
    opacity: 1;
  }

  .legend {
    margin: -4px 0 10px;
    font-size: 11px;
    color: var(--muted);
  }

  .legend i {
    font-style: normal;
    text-decoration: underline wavy rgba(255, 92, 72, 0.85);
    text-underline-offset: 3px;
  }

  .legend u {
    text-decoration: none;
    border-bottom: 1.5px dashed rgba(245, 181, 68, 0.9);
  }
</style>
