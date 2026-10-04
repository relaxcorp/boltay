<script lang="ts">
  import SectionTitle from "../components/SectionTitle.svelte";
  import { onMount } from "svelte";
  import { api, detailLines, httpStatus, megabytes, SPEECH_MODEL, type ModelId } from "../lib/api";
  import { t, type Key } from "../lib/i18n.svelte";
  import { models } from "../lib/models.svelte";
  import { store } from "../lib/store.svelte";

  const s = $derived(store.settings!);
  const data = $derived(models.data);
  let confirmDelete = $state<ModelId | null>(null);
  let details = $state<ModelId | null>(null);

  onMount(() => models.start());

  // The folder setting is saved with a short delay, re-check after it lands.
  $effect(() => {
    store.savedAt;
    models.refresh();
  });

  const names: Record<ModelId, [string, Key]> = {
    "gigaam-v3-e2e-rnnt": ["GigaAM v3", "models.gigaam"],
    "parakeet-v3": ["Parakeet v3", "models.parakeet"],
    "silero-vad": ["Silero VAD", "models.vad"],
    "opus-mt-ru-en": ["Opus-MT RU→EN", "models.opus"],
    "opus-mt-tc-big-zle-en": ["Opus-MT big RU→EN", "models.same_better"],
    "opus-mt-en-ru": ["Opus-MT EN→RU", "models.opus_en_ru"],
    "opus-mt-tc-big-en-zle": ["Opus-MT big EN→RU", "models.same_better"],
  };

  function setDir(value: string) {
    s.models_dir = value.trim() || null;
    store.save(500);
  }

  function failed(error: string): string {
    const code = httpStatus(error);
    return code ? t("models.failed_http", { code }) : t("models.failed");
  }

  async function remove(id: ModelId) {
    confirmDelete = null;
    await models.remove(id);
  }
</script>

{#if data}
  <SectionTitle theme="models">{t("nav.models")}</SectionTitle>
  <div class="group">
    {#each data.items as m (m.id)}
      {@const inUse = m.id === SPEECH_MODEL[s.speech] || m.id === "silero-vad"}
      <div class="row model">
        <div class="label">
          <span>
            <b>{names[m.id][0]}</b>
            <span class="muted">— {t(names[m.id][1])}</span>
          </span>
          {#if models.downloading(m.id)}
            <div class="bar"><div style="width: {models.share(m.id) * 100}%"></div></div>
            <span class="muted small">
              {megabytes(models.share(m.id) * m.size)} / {megabytes(m.size)}
            </span>
          {:else if models.errors[m.id]}
            {@const error = models.errors[m.id]!}
            <span class="small">
              <span class="error">{failed(error)}</span>
              <button class="link more" onclick={() => (details = details === m.id ? null : m.id)}>
                {details === m.id ? t("hide_details") : t("details")}
              </button>
            </span>
            {#if details === m.id}
              <pre class="details mono small">{detailLines(error)}</pre>
              <span class="small">
                <button class="link" onclick={() => api.copy(error)}>{t("copy")}</button>
                ·
                <button class="link" onclick={() => api.openLogs()}>{t("open_logs")}</button>
              </span>
            {/if}
          {:else if m.ready}
            <span class="muted small">
              {megabytes(m.size)}
              {#if inUse && m.id !== "silero-vad"}
                · {data.loading ? t("models.loading") : data.loaded ? t("models.loaded") : t("models.in_use")}
              {/if}
            </span>
          {:else if m.done > 0}
            <span class="muted small">{t("models.partial", { done: megabytes(m.done), size: megabytes(m.size) })}</span>
          {:else}
            <span class="muted small">{megabytes(m.size)}{inUse ? ` · ${t("models.needed")}` : ""}</span>
          {/if}
        </div>
        <div class="control">
          {#if models.downloading(m.id)}
            <button onclick={() => models.cancel(m.id)}>{t("models.cancel")}</button>
          {:else if m.ready}
            {#if confirmDelete === m.id}
              <button class="primary" onclick={() => remove(m.id)}>{t("remove")}</button>
              <button onclick={() => (confirmDelete = null)}>{t("general.hotkey_cancel")}</button>
            {:else}
              <span class="ok">{t("models.found")}</span>
              <button class="ghost" title={t("remove")} onclick={() => (confirmDelete = m.id)}>✕</button>
            {/if}
          {:else}
            <button class:primary={inUse} onclick={() => models.download([m.id])}>
              {m.done > 0 ? t("models.resume") : t("models.download")}
            </button>
          {/if}
        </div>
      </div>
    {/each}
  </div>
  <p class="note">{t("models.hint")}</p>

  <SectionTitle theme="models-folder">{t("models.folder")}</SectionTitle>
  <div class="group">
    <div class="row">
      <input
        type="text"
        class="mono path"
        value={s.models_dir ?? ""}
        placeholder={data.default_dir}
        oninput={(e) => setDir(e.currentTarget.value)}
      />
    </div>
    <div class="row">
      <span class="muted small">{s.models_dir ? "" : t("models.folder_default")}</span>
      <div class="control">
        {#if s.models_dir}
          <button onclick={() => setDir("")}>{t("models.folder_reset")}</button>
        {/if}
        <button onclick={() => api.openFolder(data.dir)}>{t("open_folder")}</button>
      </div>
    </div>
  </div>
{/if}

<style>
  .path {
    width: 100%;
  }

  .model .label {
    flex: 1;
    gap: 2px;
  }

  .bar {
    height: 4px;
    margin: 4px 0 2px;
    border-radius: 2px;
    background: var(--line);
    overflow: hidden;
    max-width: 320px;
  }

  .bar div {
    height: 100%;
    background: var(--accent);
    transition: width 150ms linear;
  }

  .ok {
    color: var(--ok);
    font-weight: 600;
    font-size: 12px;
  }

  .error {
    color: var(--accent);
  }

  .more {
    margin-left: 6px;
  }

  .details {
    margin: 4px 0;
    padding: 6px 8px;
    border-radius: 6px;
    background: var(--line);
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    user-select: text;
    max-width: 440px;
  }
</style>
