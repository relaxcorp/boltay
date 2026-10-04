<script lang="ts">
  import { onMount } from "svelte";
  import Dropdown from "../components/Dropdown.svelte";
  import MicMeter from "../components/MicMeter.svelte";
  import OverlayPicker from "../components/OverlayPicker.svelte";
  import PlayButton from "../components/PlayButton.svelte";
  import SectionTitle from "../components/SectionTitle.svelte";
  import { api, SOUND_SETS, type Delta, type ImportPreview } from "../lib/api";
  import { locale, t } from "../lib/i18n.svelte";
  import { store } from "../lib/store.svelte";
  import HotkeyInput from "../lib/HotkeyInput.svelte";
  import { isSolo } from "../lib/hotkey";
  import { models } from "../lib/models.svelte";

  const s = $derived(store.settings!);
  let microphones = $state<string[]>([]);
  let volume = $state<number | null>(null);
  let saved = $state("");
  let preview = $state<ImportPreview | null>(null);
  let resetting = $state(false);
  const HOTKEYS = ["hotkey", "translate_hotkey", "translator_hotkey"] as const;
  type HotkeyField = (typeof HOTKEYS)[number];
  const noErrors = (): Record<HotkeyField, string> => ({ hotkey: "", translate_hotkey: "", translator_hotkey: "" });
  let hotkeyErrors = $state(noErrors());

  // Any successful save registered the hotkeys anew: an error from launch is history.
  $effect(() => {
    if (store.savedAt) hotkeyErrors = noErrors();
  });

  onMount(async () => {
    models.start();
    // The saved hotkeys failed at launch: say so where they are set. A taken combination
    // is named at the start of the message.
    const error = await api.hotkeyError();
    if (error) {
      const field = HOTKEYS.find((f) => error.startsWith(`${s[f]} `)) ?? HOTKEYS.find((f) => isSolo(s[f]));
      hotkeyErrors[field ?? "hotkey"] = error;
    }
    microphones = await api.microphones();
  });

  // A microphone turned down in the Windows sound settings is the usual reason for "nothing heard".
  $effect(() => {
    s.microphone;
    api.micVolume().then((v) => (volume = v));
  });

  const save = () => store.save(0);

  async function setHotkey(field: HotkeyField, value: string) {
    hotkeyErrors = noErrors();
    if (HOTKEYS.some((f) => f !== field && s[f] === value)) {
      hotkeyErrors[field] = t("general.hotkeys_same");
      return;
    }
    s[field] = value;
    hotkeyErrors[field] = await store.saveNow();
  }

  function setLimit(value: number) {
    if (Number.isFinite(value) && value > 0) {
      s.max_recording_secs = Math.round(value * 60);
      store.save();
    }
  }

  function setSound(set: (typeof SOUND_SETS)[number]) {
    s.sound_set = set;
    save();
    api.previewSound(set);
  }

  async function raiseVolume() {
    try {
      await api.raiseMicVolume();
      volume = await api.micVolume();
    } catch (e) {
      store.error = String(e);
    }
  }

  async function exportSettings() {
    try {
      saved = (await api.exportSettings()) ?? "";
    } catch (e) {
      store.error = String(e);
    }
  }

  async function importSettings() {
    try {
      preview = await api.importSettings();
    } catch (e) {
      store.error = String(e);
    }
  }

  async function applyImport() {
    try {
      await api.applyImport();
    } catch (e) {
      store.error = String(e);
    }
    preview = null;
  }

  function cancelImport() {
    preview = null;
    api.cancelImport();
  }

  async function reset() {
    resetting = false;
    try {
      await api.resetSettings();
    } catch (e) {
      store.error = String(e);
    }
  }

  const delta = (d: Delta) => [d.added && `+${d.added}`, d.removed && `−${d.removed}`].filter(Boolean).join(" ");

  function describe(p: ImportPreview): string {
    const what = [
      p.changes && t("general.backup_changes", { n: p.changes }),
      delta(p.dictionary) && t("general.backup_dictionary", { d: delta(p.dictionary) }),
      delta(p.snippets) && t("general.backup_snippets", { d: delta(p.snippets) }),
      delta(p.brands) && t("general.backup_brands", { d: delta(p.brands) }),
    ].filter(Boolean);
    const date = p.modified ? new Date(p.modified).toLocaleDateString(locale.lang, { day: "numeric", month: "short" }) : "";
    return t("general.backup_preview", {
      file: p.file,
      date,
      what: what.length ? what.join(", ") : t("general.backup_same"),
    });
  }

  const microphoneOptions = $derived([
    { value: null, label: t("general.microphone_default") },
    ...microphones.map((name) => ({ value: name, label: name })),
    // A saved microphone that is unplugged right now stays selected.
    ...(s.microphone && !microphones.includes(s.microphone) ? [{ value: s.microphone, label: s.microphone }] : []),
  ]);

  const pauseOptions = $derived.by(() => {
    const steps = [1.5, 2, 3];
    if (s.paragraph_pause_secs && !steps.includes(s.paragraph_pause_secs)) steps.push(s.paragraph_pause_secs);
    return [
      { value: null, label: t("general.paragraph_pause_off") },
      ...steps.map((n) => ({ value: n, label: t("general.seconds", { n: n.toLocaleString(locale.lang) }) })),
    ];
  });

  const gainOptions = $derived([
    { value: 0, label: t("general.gain_off") },
    ...[6, 12, 18].map((n) => ({ value: n, label: t("general.gain_db", { n }) })),
  ]);

  const thresholdOptions = $derived([
    { value: 0.3, label: t("general.threshold_low") },
    { value: 0.5, label: t("general.threshold_normal") },
    { value: 0.7, label: t("general.threshold_high") },
  ]);
</script>

<SectionTitle theme="recording">{t("general.section_recording")}</SectionTitle>
<div class="group">
  <div class="row">
    <div class="label">
      <span>{t("general.hotkey")}</span>
      <span class="muted small">{t("general.hotkey_hint")} {t(store.info?.os === "windows" ? "general.hotkey_flex" : "general.hotkey_flex_combo")}</span>
      {#if isSolo(s.hotkey)}<span class="muted small">{t("general.hotkey_admin")}</span>{/if}
      {#if hotkeyErrors.hotkey}<span class="error small">{hotkeyErrors.hotkey}</span>{/if}
    </div>
    <div class="control">
      <HotkeyInput value={s.hotkey} onchange={(h) => setHotkey("hotkey", h)} />
    </div>
  </div>
  <div class="row">
    <div class="label">
      <span>{t("general.translate_hotkey")}</span>
      <span class="muted small">{t("general.translate_hotkey_hint")}</span>
      {#if isSolo(s.translate_hotkey)}<span class="muted small">{t("general.hotkey_admin")}</span>{/if}
      {#if hotkeyErrors.translate_hotkey}<span class="error small">{hotkeyErrors.translate_hotkey}</span>{/if}
    </div>
    <div class="control">
      <HotkeyInput value={s.translate_hotkey} onchange={(h) => setHotkey("translate_hotkey", h)} />
    </div>
  </div>
  <div class="row">
    <div class="label">
      <span>{t("general.translator_hotkey")}</span>
      <span class="muted small">{t("general.translator_hotkey_hint")}</span>
      {#if isSolo(s.translator_hotkey)}<span class="muted small">{t("general.hotkey_admin")}</span>{/if}
      {#if hotkeyErrors.translator_hotkey}<span class="error small">{hotkeyErrors.translator_hotkey}</span>{/if}
    </div>
    <div class="control">
      <HotkeyInput value={s.translator_hotkey} onchange={(h) => setHotkey("translator_hotkey", h)} />
    </div>
  </div>
  <div class="row">
    <span>{t("general.mode")}</span>
    <div class="control">
      <Dropdown
        value={s.mode}
        options={[
          { value: "hold" as const, label: t("general.mode_hold") },
          { value: "toggle" as const, label: t("general.mode_toggle") },
        ]}
        onchange={(v) => { s.mode = v; save(); }}
      />
    </div>
  </div>
  {#if s.mode === "hold"}
    <label class="row sub">
      <div class="label">
        <span>{t("general.latch")}</span>
        <span class="muted small">{t("general.latch_hint")}</span>
      </div>
      <input type="checkbox" bind:checked={s.latch} onchange={save} />
    </label>
  {/if}
  <div class="row">
    <span>{t("general.microphone")}</span>
    <div class="control">
      <MicMeter device={s.microphone} gain={s.mic_gain_db} />
      <Dropdown value={s.microphone} options={microphoneOptions} onchange={(v) => { s.microphone = v; save(); }} />
    </div>
  </div>
  <div class="row sub">
    <div class="label">
      <span>{t("general.gain")}</span>
      <span class="muted small">{t("general.gain_hint")}</span>
    </div>
    <div class="control">
      <Dropdown value={s.mic_gain_db} options={gainOptions} onchange={(v) => { s.mic_gain_db = v; save(); }} />
    </div>
  </div>
  <div class="row sub">
    <div class="label">
      <span>{t("general.threshold")}</span>
      <span class="muted small">{t("general.threshold_hint")}</span>
    </div>
    <div class="control">
      <Dropdown value={s.vad_threshold} options={thresholdOptions} onchange={(v) => { s.vad_threshold = v; save(); }} />
    </div>
  </div>
  {#if volume !== null && volume < 60}
    <div class="row sub">
      <span class="warn small">{t("general.mic_volume_low", { n: volume })}</span>
      <div class="control">
        <button onclick={raiseVolume}>{t("general.mic_volume_raise")}</button>
      </div>
    </div>
  {/if}
  <div class="row">
    <span>{t("general.overlay")}</span>
    <div class="control">
      <OverlayPicker value={s.overlay_position} onchange={(v) => { s.overlay_position = v; save(); }} />
    </div>
  </div>
  <label class="row">
    <span>{t("general.sounds")}</span>
    <input type="checkbox" bind:checked={s.sounds} onchange={save} />
  </label>
  {#if s.sounds}
    <div class="row sub">
      <div class="label">
        <span>{t("general.sound_set")}</span>
        <span class="muted small">{t("general.sound_set_hint")}</span>
      </div>
      <div class="control">
        <Dropdown
          value={s.sound_set}
          options={SOUND_SETS.map((id) => ({ value: id, label: t(`sound.${id}`) }))}
          onchange={setSound}
        >
          {#snippet item(option)}
            <PlayButton small label={option.label} onplay={() => api.previewSound(option.value)} />
          {/snippet}
        </Dropdown>
        <PlayButton label={t("general.sound_play")} onplay={() => api.previewSound(s.sound_set)} />
      </div>
    </div>
  {/if}
</div>
{#if store.info?.display === "wayland"}
  <p class="note warn">{t("general.wayland")}</p>
{/if}

<SectionTitle theme="recognition">{t("general.section_recognition")}</SectionTitle>
<div class="group">
  <div class="row">
    <span>{t("general.language")}</span>
    <div class="control">
      <Dropdown
        value={s.speech}
        options={[
          { value: "russian" as const, label: `${t("general.language_ru")} — GigaAM` },
          { value: "other" as const, label: `${t("general.language_other")} — Parakeet` },
        ]}
        onchange={(v) => { s.speech = v; save(); }}
      />
    </div>
  </div>
  <div class="row">
    <div class="label">
      <span>{t("general.paste")}</span>
      <span class="muted small">{t("general.paste_hint")}</span>
    </div>
    <div class="control">
      <Dropdown
        value={s.paste}
        options={[
          { value: "clipboard" as const, label: t(store.info?.os === "macos" ? "general.paste_clipboard_mac" : "general.paste_clipboard") },
          { value: "type" as const, label: t("general.paste_type") },
        ]}
        onchange={(v) => { s.paste = v; save(); }}
      />
    </div>
  </div>
  <label class="row sub">
    <div class="label">
      <span>{t("general.paragraph_messages")}</span>
      <span class="muted small">{t("general.paragraph_messages_hint")}</span>
    </div>
    <input type="checkbox" bind:checked={s.paragraph_messages} onchange={save} />
  </label>
  {#if s.paste === "clipboard"}
    <label class="row sub">
      <span>{t("general.restore_clipboard")}</span>
      <input type="checkbox" bind:checked={s.restore_clipboard} onchange={save} />
    </label>
  {/if}
  <div class="row">
    <div class="label">
      <span>{t("general.paragraph_pause")}</span>
      <span class="muted small">{t("general.paragraph_pause_hint")}</span>
    </div>
    <div class="control">
      <Dropdown value={s.paragraph_pause_secs} options={pauseOptions} onchange={(v) => { s.paragraph_pause_secs = v; save(); }} />
    </div>
  </div>
</div>

{#if models.data && !models.ready(s.speech)}
  <p class="note warn">
    {t("models.language_missing")}
    <button class="link" onclick={() => (store.section = "models")}>{t("models.go")}</button>
  </p>
{/if}

<SectionTitle theme="system">{t("general.section_system")}</SectionTitle>
<div class="group">
  <label class="row">
    <span>{t("general.autostart")}</span>
    <input type="checkbox" bind:checked={s.autostart} onchange={save} />
  </label>
  <div class="row">
    <span>{t("general.ui_language")}</span>
    <div class="control">
      <Dropdown
        value={s.ui_language}
        options={[
          { value: null, label: t("general.ui_language_system") },
          { value: "en" as const, label: "English" },
          { value: "ru" as const, label: "Русский" },
        ]}
        onchange={(v) => { s.ui_language = v; save(); }}
      />
    </div>
  </div>
  <label class="row">
    <div class="label">
      <span>{t("general.updates")}</span>
      <span class="muted small">{t("general.updates_hint")}</span>
    </div>
    <input type="checkbox" bind:checked={s.check_updates} onchange={save} />
  </label>
</div>

<SectionTitle theme="backup">{t("general.section_backup")}</SectionTitle>
<div class="group">
  <div class="row">
    <div class="label">
      <span>{t("general.backup_export")}</span>
      <span class="muted small">{saved ? t("general.backup_saved", { file: saved }) : t("general.backup_export_hint")}</span>
    </div>
    <div class="control"><button onclick={exportSettings}>{t("general.backup_export_btn")}</button></div>
  </div>
  <div class="row">
    <div class="label">
      <span>{t("general.backup_import")}</span>
      <span class="muted small">{preview ? describe(preview) : t("general.backup_import_hint")}</span>
    </div>
    <div class="control">
      {#if preview}
        <button class="primary" onclick={applyImport}>{t("general.backup_apply")}</button>
        <button onclick={cancelImport}>{t("general.hotkey_cancel")}</button>
      {:else}
        <button onclick={importSettings}>{t("general.backup_import_btn")}</button>
      {/if}
    </div>
  </div>
  <div class="row">
    <div class="label">
      <span>{t("general.backup_reset")}</span>
      <span class="muted small">{resetting ? t("general.backup_reset_confirm") : t("general.backup_reset_hint")}</span>
    </div>
    <div class="control">
      {#if resetting}
        <button class="primary" onclick={reset}>{t("general.backup_reset_btn")}</button>
        <button onclick={() => (resetting = false)}>{t("general.hotkey_cancel")}</button>
      {:else}
        <button onclick={() => (resetting = true)}>{t("general.backup_reset_btn")}</button>
      {/if}
    </div>
  </div>
</div>

<SectionTitle theme="advanced">{t("general.section_advanced")}</SectionTitle>
<div class="group">
  <label class="row">
    <div class="label">
      <span>{t("general.quality_translation")}</span>
      <span class="muted small">{t("general.quality_translation_hint")}</span>
    </div>
    <input type="checkbox" bind:checked={s.quality_translation} onchange={save} />
  </label>
  <div class="row">
    <span>{t("general.limit")}</span>
    <div class="control">
      <input
        type="number"
        min="1"
        max="60"
        style="width: 64px"
        value={Math.round(s.max_recording_secs / 60)}
        oninput={(e) => setLimit(e.currentTarget.valueAsNumber)}
      />
      <span class="muted">{t("general.minutes")}</span>
    </div>
  </div>
  <div class="row">
    <span>{t("general.unload")}</span>
    <div class="control">
      <Dropdown
        value={s.unload_after_minutes}
        options={[
          { value: null, label: t("general.unload_never") },
          ...[5, 15, 60].map((n) => ({ value: n, label: t("general.unload_after", { n }) })),
        ]}
        onchange={(v) => { s.unload_after_minutes = v; save(); }}
      />
    </div>
  </div>
  <label class="row">
    <div class="label">
      <span>{t("general.graph_cache")}</span>
      <span class="muted small">{t("general.graph_cache_hint")}</span>
    </div>
    <input type="checkbox" bind:checked={s.graph_cache} onchange={save} />
  </label>
</div>

<style>
  .warn {
    color: var(--warn);
  }

  .error {
    color: var(--accent);
  }
</style>
