<script lang="ts">
  import { onMount } from "svelte";
  import { api, megabytes, needed, type Permissions } from "../lib/api";
  import { models } from "../lib/models.svelte";
  import { t } from "../lib/i18n.svelte";
  import { store } from "../lib/store.svelte";
  import { pretty } from "../lib/hotkey";

  let { done }: { done: () => void } = $props();

  let permissions = $state<Permissions | null>(null);
  const speech = $derived(store.settings!.speech);
  const ids = $derived(needed(speech));
  const downloading = $derived(ids.some((id) => models.downloading(id)));
  const share = $derived.by(() => {
    const items = ids.map((id) => models.item(id)).filter((m) => m !== undefined);
    const total = items.reduce((sum, m) => sum + m.size, 0);
    return total ? items.reduce((sum, m) => sum + models.share(m.id) * m.size, 0) / total : 0;
  });

  onMount(() => {
    models.start();
    const check = async () => {
      permissions = await api.permissions();
    };
    check();
    // The user flips the switches in System Settings; pick that up without a restart.
    const timer = setInterval(check, 1500);
    return () => clearInterval(timer);
  });

  // Only the first welcome is marked as seen; opened for a missing permission, it leads on
  // into the settings.
  async function finish() {
    if (store.info?.first_run) await api.closeOnboarding();
    done();
  }
</script>

<div class="onboarding">
  <img src="/icon.png" alt="" width="64" height="64" />
  <h1>{t("onboarding.title")}</h1>
  <p>
    {t("onboarding.how", { hotkey: pretty(store.settings!.hotkey) })}
    {t("onboarding.translate", { hotkey: pretty(store.settings!.translate_hotkey) })}
  </p>

  {#if permissions?.needed}
    <p class="muted">{t("onboarding.permissions")}</p>
    <div class="group">
      <div class="row">
        <span>{t("onboarding.microphone")}</span>
        <div class="control">
          {#if permissions.microphone === "granted"}
            <span class="ok">{t("onboarding.granted")}</span>
          {:else if permissions.microphone === "unknown"}
            <button class="primary" onclick={() => api.requestMicrophone()}>{t("onboarding.grant")}</button>
          {:else}
            <button onclick={() => api.openPermissionSettings("microphone")}>{t("onboarding.open_settings")}</button>
          {/if}
        </div>
      </div>
      <div class="row">
        <span>{t("onboarding.accessibility")}</span>
        <div class="control">
          {#if permissions.accessibility === "granted"}
            <span class="ok">{t("onboarding.granted")}</span>
          {:else}
            <!-- The request puts Boltay on the list; once there, only the list turns it on. -->
            <button class="primary" onclick={() => api.requestAccessibility()}>{t("onboarding.grant")}</button>
            <button onclick={() => api.openPermissionSettings("accessibility")}>{t("onboarding.open_settings")}</button>
          {/if}
        </div>
      </div>
      {#if permissions.accessibility !== "granted"}
        <p class="muted small">{t("onboarding.accessibility_stale")}</p>
      {/if}
    </div>
  {/if}

  {#if models.data && !models.ready(speech)}
    <div class="download">
      <p class="muted">{t("onboarding.model_missing")}</p>
      {#if downloading}
        <div class="bar"><div style="width: {share * 100}%"></div></div>
        <button onclick={() => ids.forEach((id) => models.cancel(id))}>{t("models.cancel")}</button>
      {:else}
        <button class="primary" onclick={() => models.download(ids)}>
          {t("onboarding.download", { size: megabytes(models.missingBytes(speech)) })}
        </button>
      {/if}
      {#each ids as id}
        {#if models.errors[id]}
          <p class="note warn">{t("models.failed")}: {models.errors[id]}</p>
        {/if}
      {/each}
    </div>
  {/if}

  <button class="start" class:primary={models.ready(speech)} onclick={finish}>{t("onboarding.done")}</button>
</div>

<style>
  .onboarding {
    max-width: 520px;
    margin: 0 auto;
    padding: 40px 24px;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 12px;
    text-align: center;
  }

  h1 {
    font-size: 20px;
    margin: 4px 0 0;
  }

  .group {
    width: 100%;
    text-align: left;
  }

  .ok {
    color: var(--ok);
    font-weight: 600;
  }

  .download {
    width: 100%;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
  }

  .bar {
    width: 100%;
    height: 4px;
    border-radius: 2px;
    background: var(--line);
    overflow: hidden;
  }

  .bar div {
    height: 100%;
    background: var(--accent);
    transition: width 150ms linear;
  }

  .start {
    margin-top: 12px;
    padding: 6px 28px;
  }
</style>
