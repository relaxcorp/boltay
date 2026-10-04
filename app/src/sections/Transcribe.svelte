<script lang="ts">
  import { onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import SectionTitle from "../components/SectionTitle.svelte";
  import { api, megabytes, plain, type ModelId, type Outcome, type TranscribeJob } from "../lib/api";
  import { t } from "../lib/i18n.svelte";
  import { models } from "../lib/models.svelte";
  import { store } from "../lib/store.svelte";

  let { dragging = false }: { dragging?: boolean } = $props();

  let job = $state<TranscribeJob | null>(null);
  let view = $state<"original" | "translation">("original");
  let translated = $state<Outcome | null>(null);
  let translating = $state(false);
  let copied = $state(false);

  const result = $derived(job?.result ?? null);
  const shown = $derived(
    view === "translation" && translated?.state === "done" ? plain(translated.translation) : (result?.text ?? ""),
  );

  async function translate() {
    view = "translation";
    if (!result || translating) return;
    translating = true;
    try {
      translated = await api.translate(result.text, result.language === "ru" ? "ru_en" : "en_ru");
    } catch (e) {
      store.error = String(e);
    }
    translating = false;
  }

  async function fetchModel(quality: boolean) {
    if (translated?.state !== "need_model") return;
    try {
      await api.translatorFetch(translated.direction, quality);
      translate();
    } catch (e) {
      store.error = String(e);
    }
  }

  async function pick() {
    try {
      await api.transcribePick();
    } catch (e) {
      store.error = String(e);
    }
  }

  async function copy() {
    await api.copy(shown);
    copied = true;
    setTimeout(() => (copied = false), 1300);
  }

  function duration(seconds: number): string {
    const s = Math.round(seconds);
    return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`;
  }

  const percent = (share: number) => Math.round(share * 100);

  onMount(() => {
    models.start();
    api.transcribeJob().then((j) => (job = j));
    const unlisten = [
      listen<TranscribeJob>("transcribe-changed", (e) => {
        // A new file starts over in its own language.
        if (e.payload.running) {
          view = "original";
          translated = null;
        }
        job = e.payload;
      }),
      listen<number>("transcribe-progress", (e) => {
        if (job) job.progress = e.payload;
      }),
      listen<{ id: ModelId; error: string | null }>("model-done", (e) => {
        if (!e.payload.error && view === "translation") translate();
      }),
    ];
    return () => unlisten.forEach((u) => u.then((f) => f()));
  });
</script>

<SectionTitle theme="recognition">{t("transcribe.title")}</SectionTitle>
<div class="group">
  <div class="drop" class:over={dragging}>
    {#if job?.running}
      <span>{t("transcribe.running", { file: job.file ?? "" })}</span>
      <div class="bar"><div style:width="{percent(job.progress)}%"></div></div>
      <button onclick={() => api.transcribeCancel()}>{t("transcribe.cancel")}</button>
    {:else}
      <span class="big">{t("transcribe.drop")}</span>
      <button class="primary" onclick={pick}>{t("transcribe.pick")}</button>
      <span class="muted small">{t("transcribe.formats")}</span>
    {/if}
  </div>
</div>

{#if job?.error && !job.running}
  <p class="note error">{t("transcribe.failed", { file: job.file ?? "" })}: {job.error}</p>
  <p class="note muted">
    {t("transcribe.models")}
    <button class="link" onclick={() => (store.section = "models")}>{t("models.go")}</button>
  </p>
{/if}

{#if result && !job?.running}
  <div class="group result">
    <div class="row">
      <span class="muted small">
        {job?.file} · {t(`transcribe.lang_${result.language}`)} · {duration(result.seconds)}
      </span>
      <div class="control">
        <button class="chip" class:on={view === "original"} onclick={() => (view = "original")}>
          {t("transcribe.original")}
        </button>
        <button class="chip" class:on={view === "translation"} onclick={translate}>
          {t("transcribe.translation")}
        </button>
      </div>
    </div>
    <div class="text">
      {#if !result.text}
        <p class="muted">{t("transcribe.empty")}</p>
      {:else if view === "translation" && translated?.state === "need_model"}
        <p>{t("translator.need_model")}</p>
        <div class="choices">
          <button onclick={() => fetchModel(false)}>{t("translator.base_model", { size: megabytes(translated.base) })}</button>
          <button class="primary" onclick={() => fetchModel(true)}>
            {t("translator.big_model", { size: megabytes(translated.big) })}
          </button>
        </div>
      {:else if view === "translation" && translated?.state === "fetching"}
        <p class="muted">{t("translator.fetching", { n: percent(models.progress[translated.id] ?? 0) })}</p>
      {:else if view === "translation" && (translating || !translated)}
        <p class="muted">{t("translator.translating")}</p>
      {:else}
        <p class="copyable">{shown}</p>
      {/if}
    </div>
    <div class="row">
      <span class="copied" class:on={copied}>{t("translator.copied")}</span>
      <div class="control">
        <button disabled={!shown} onclick={copy}>{t("translator.copy")}</button>
      </div>
    </div>
  </div>
{/if}

<style>
  .drop {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 12px;
    margin: 14px;
    padding: 26px 20px;
    border-radius: 12px;
    border: 1.5px dashed var(--line);
    text-align: center;
    transition:
      border-color 0.2s,
      background 0.2s;
  }

  .drop.over {
    border-color: var(--accent);
    background: var(--row-hover);
  }

  .big {
    font-size: 15px;
    font-weight: 600;
  }

  .bar {
    width: 100%;
    max-width: 360px;
    height: 6px;
    border-radius: 3px;
    background: var(--switch-off);
    overflow: hidden;
  }

  .bar div {
    height: 100%;
    background: var(--yolk);
    transition: width 0.2s linear;
  }

  .result {
    margin-top: 16px;
  }

  .text {
    padding: 4px 18px 14px;
  }

  .copyable {
    white-space: pre-wrap;
    user-select: text;
    cursor: text;
    font-size: 14px;
    line-height: 1.6;
  }

  .choices {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    margin-top: 10px;
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

  .error {
    color: var(--accent);
    margin-top: 12px;
  }
</style>
