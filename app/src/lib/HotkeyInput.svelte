<script lang="ts">
  import { onDestroy } from "svelte";
  import { api } from "./api";
  import { t } from "./i18n.svelte";
  import { pretty } from "./hotkey";
  import { store } from "./store.svelte";

  let { value, onchange }: { value: string; onchange: (hotkey: string) => void } = $props();

  let recording = $state(false);
  let hint = $state("");
  // The modifier or CapsLock pressed first and alone so far: on release it may be the hotkey.
  let alone: string | null = null;

  const MODS = ["ControlLeft", "ControlRight", "ShiftLeft", "ShiftRight", "AltLeft", "AltRight", "MetaLeft", "MetaRight"];
  // A left modifier on its own is half of every Ctrl+C, it cannot start a recording.
  const SOLO = ["ControlRight", "AltRight", "ShiftRight", "CapsLock"];
  // A key on its own is watched by a Windows input hook; elsewhere only combinations work.
  const solo = $derived(store.info?.os === "windows");
  const mac = $derived(store.info?.os === "macos");
  // Every app relies on these: as a hotkey they would stop working everywhere.
  const RESERVED = $derived(
    mac
      ? ["KeyQ", "KeyW", "KeyC", "KeyV", "KeyX", "KeyZ", "KeyA", "KeyH", "KeyM", "Tab", "Space", "Comma"].map((k) => `Super+${k}`)
      : ["KeyC", "KeyV", "KeyX", "KeyZ", "KeyA"].map((k) => `Ctrl+${k}`),
  );

  async function start() {
    recording = true;
    hint = "";
    alone = null;
    await api.suspendHotkey(true);
  }

  async function stop() {
    recording = false;
    await api.suspendHotkey(false);
  }

  const done = (hotkey: string) => stop().then(() => onchange(hotkey));

  // Leaving the window or closing it mid-recording must not leave the hotkeys let go.
  onDestroy(() => {
    if (recording) api.suspendHotkey(false);
  });

  function onkeydown(e: KeyboardEvent) {
    if (!recording) return;
    e.preventDefault();
    if (e.code === "Escape") return void stop();
    if (MODS.includes(e.code) || e.code === "CapsLock") {
      alone = alone === null ? e.code : "";
      return;
    }
    alone = "";
    const mods = [e.ctrlKey && "Ctrl", e.altKey && "Alt", e.shiftKey && "Shift", e.metaKey && "Super"].filter(Boolean);
    const fkey = /^F\d{1,2}$/.test(e.code);
    // A bare letter, or one with Shift, as a global hotkey would swallow it in every app.
    if (!fkey && !e.ctrlKey && !e.altKey && !e.metaKey) {
      hint = t(solo ? "general.hotkey_need_modifier" : "general.hotkey_need_modifier_combo");
      return;
    }
    const hotkey = [...mods, e.code].join("+");
    if (RESERVED.includes(hotkey)) {
      hint = t("general.hotkey_reserved");
      return;
    }
    done(hotkey);
  }

  function onkeyup(e: KeyboardEvent) {
    if (!recording || alone !== e.code) return;
    if (solo && SOLO.includes(e.code)) return void done(e.code);
    hint = t(solo ? "general.hotkey_take_right" : "general.hotkey_need_modifier_combo");
    alone = null;
  }
</script>

<svelte:window {onkeydown} {onkeyup} onblur={() => recording && stop()} />

<div class="hotkey">
  {#if recording}
    <span class="keys recording">{hint || t("general.hotkey_press")}</span>
    <button onclick={stop}>{t("general.hotkey_cancel")}</button>
  {:else}
    <span class="keys">{pretty(value)}</span>
    <button onclick={start}>{t("general.hotkey_change")}</button>
  {/if}
</div>

<style>
  .hotkey {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .keys {
    min-width: 140px;
    text-align: center;
  }
</style>
