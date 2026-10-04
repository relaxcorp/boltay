<script lang="ts">
  import { onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import SectionTitle from "../components/SectionTitle.svelte";
  import { api, type UpdateStatus } from "../lib/api";
  import { t } from "../lib/i18n.svelte";
  import { store } from "../lib/store.svelte";

  let update = $state<UpdateStatus>({ checking: false, checked: false, available: null });

  onMount(() => {
    const load = () => api.updateStatus().then((u) => (update = u));
    load();
    const unlisten = listen("update-changed", load);
    return () => unlisten.then((f) => f());
  });

  async function check() {
    update = { ...update, checking: true };
    update = await api.checkUpdates();
  }

  const REPO = "https://github.com/relaxcorp/boltay";
  const SITE = "https://relaxlab.net";

  const credits = [
    { name: "GigaAM v3", by: "SaluteDevices", license: "MIT", url: "https://github.com/salute-developers/GigaAM" },
    { name: "Parakeet TDT 0.6B v3", by: "NVIDIA", license: "CC-BY-4.0", url: "https://huggingface.co/nvidia/parakeet-tdt-0.6b-v3" },
    { name: "Silero VAD", by: "Silero", license: "MIT", url: "https://github.com/snakers4/silero-vad" },
    { name: "Opus-MT ru-en", by: "Helsinki-NLP", license: "CC-BY-4.0", url: "https://huggingface.co/Helsinki-NLP/opus-mt-ru-en" },
    { name: "Opus-MT tc-big zle-en", by: "Helsinki-NLP", license: "CC-BY-4.0", url: "https://huggingface.co/Helsinki-NLP/opus-mt-tc-big-zle-en" },
    { name: "Opus-MT en-ru", by: "Helsinki-NLP", license: "Apache-2.0", url: "https://huggingface.co/Helsinki-NLP/opus-mt-en-ru" },
    { name: "Opus-MT tc-big en-zle", by: "Helsinki-NLP", license: "CC-BY-4.0", url: "https://huggingface.co/Helsinki-NLP/opus-mt-tc-big-en-zle" },
    { name: "ONNX exports", by: "istupakov, Xenova, TigreGotico", license: "MIT / CC-BY-4.0 / Apache-2.0", url: "https://github.com/istupakov/onnx-asr" },
    { name: "ONNX Runtime", by: "Microsoft", license: "MIT", url: "https://github.com/microsoft/onnxruntime" },
    { name: "Tauri", by: "Tauri Programme", license: "MIT / Apache-2.0", url: "https://tauri.app" },
  ];
</script>

<div class="head">
  <img src="/icon.png" alt="" width="56" height="56" />
  <div>
    <h1>Boltay</h1>
    <p class="muted">{t("about.version", { v: store.info!.version })}</p>
    <button class="link byline" onclick={() => api.openLink(SITE)}>{t("about.by")}</button>
  </div>
</div>
<p class="tagline">{t("about.tagline")}</p>

<div class="group update" class:fresh={update.available}>
  <div class="row">
    <div class="label">
      {#if update.available}
        <span>{t("about.update", { v: update.available.version })}</span>
        {#if update.available.notes}
          <span class="muted small notes">{update.available.notes}</span>
        {/if}
      {:else if update.checking}
        <span>{t("about.checking")}</span>
      {:else}
        <span>{t("about.up_to_date")}</span>
        <span class="muted small">{t("about.version", { v: store.info!.version })}</span>
      {/if}
    </div>
    <div class="control">
      {#if update.available}
        <button class="primary" onclick={() => api.openLink(update.available!.url)}>{t("about.download")}</button>
      {:else}
        <button onclick={check} disabled={update.checking}>{t("about.check_now")}</button>
      {/if}
    </div>
  </div>
</div>

<SectionTitle theme="licenses">{t("about.licenses")}</SectionTitle>
<div class="group">
  {#each credits as c}
    <div class="row">
      <div class="label">
        <button class="link" onclick={() => api.openLink(c.url)}>{c.name}</button>
        <span class="muted small">{c.by}</span>
      </div>
      <span class="muted">{c.license}</span>
    </div>
  {/each}
</div>

<p class="note">
  Boltay — MIT.
  <button class="link" onclick={() => api.openLink(REPO)}>{t("about.source")}</button>
  ·
  <button class="link" onclick={() => api.openNotices()}>{t("about.notices")}</button>
  ·
  <button class="link" onclick={() => api.openLogs()}>{t("open_logs")}</button>
</p>

<style>
  .head {
    display: flex;
    align-items: center;
    gap: 16px;
  }

  h1 {
    font-size: 20px;
    margin: 0;
  }

  .byline {
    color: var(--muted);
    text-decoration: underline;
    text-decoration-color: color-mix(in srgb, currentColor 35%, transparent);
    text-underline-offset: 3px;
  }

  .byline:hover {
    color: var(--accent);
    text-decoration: underline;
  }

  .tagline {
    margin: 16px 0 0;
  }

  .update {
    margin: 14px 0 4px;
  }

  .update.fresh {
    box-shadow:
      inset 0 1px 0 var(--plate-hi),
      0 0 0 1px rgba(255, 92, 72, 0.45),
      0 3px 0 var(--plate-edge),
      0 0 28px -10px var(--accent-glow);
  }

  .notes {
    display: -webkit-box;
    -webkit-line-clamp: 3;
    line-clamp: 3;
    -webkit-box-orient: vertical;
    overflow: hidden;
    white-space: pre-line;
  }

  .label .link {
    text-align: left;
  }
</style>
