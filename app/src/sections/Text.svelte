<script lang="ts">
  import Dropdown from "../components/Dropdown.svelte";
  import SectionTitle from "../components/SectionTitle.svelte";
  import { onDestroy } from "svelte";
  import { api, type Heard, type Profanity } from "../lib/api";
  import { t } from "../lib/i18n.svelte";
  import { store } from "../lib/store.svelte";

  const text = $derived(store.settings!.text);
  const stock = $derived(store.info!.soft_fillers);
  const custom = $derived(text.fillers.soft.filter((w) => !stock.includes(w)));
  const modes: Profanity[] = ["keep", "mask", "remove", "soften"];

  let newFiller = $state("");
  let brandsOpen = $state(false);
  let confirmingReset = $state(false);
  // What a variants field shows while it has focus: parsing it back on every keystroke
  // would eat the comma being typed.
  let drafts = $state<Record<number, string>>({});
  let sample = $state("");
  let cheatOpen = $state(false);
  let voice = $state<"idle" | "listening" | "working">("idle");
  let heard = $state<Heard | null>(null);

  // What is said, and what it turns into. The commands are speech, so they stay as spoken.
  const COMMANDS: [string, string][] = $derived([
    ["«абзац», «новый абзац»", t("text.cmd_empty_line")],
    ["«новая строка», «с новой строки»", t("text.cmd_line_break")],
    ["«удали последнее»", t("text.cmd_delete")],
    ["«вопросительный знак»", "?"],
    ["«восклицательный знак»", "!"],
    ["«двоеточие»", ":"],
    ["«точка с запятой»", ";"],
    ["«троеточие», «многоточие»", "…"],
    ["«тире»", "—"],
    ["«дефис»", "-"],
    ["«открыть / закрыть кавычки»", "« »"],
    ["«открыть / закрыть скобку»", "( )"],
    ["«запятая», «точка»", t("text.cmd_if_on")],
    ["«new line», «new paragraph», «scratch that»", t("text.cmd_english")],
  ]);

  async function voiceCheck() {
    try {
      if (voice === "idle") {
        heard = null;
        await api.voiceCheckStart();
        voice = "listening";
      } else if (voice === "listening") {
        voice = "working";
        heard = await api.voiceCheckStop();
        voice = "idle";
      }
    } catch (e) {
      voice = "idle";
      store.error = String(e);
    }
  }

  onDestroy(() => {
    if (voice === "listening") api.voiceCheckCancel();
  });
  let result = $state("");

  const save = () => store.save(0);
  const saveLater = () => store.save(600);

  function toggleFiller(word: string, on: boolean) {
    text.fillers.soft = on
      ? [...text.fillers.soft, word]
      : text.fillers.soft.filter((w) => w !== word);
    save();
  }

  function addFiller() {
    const word = newFiller.trim().toLowerCase();
    if (word && !text.fillers.soft.includes(word)) {
      text.fillers.soft = [...text.fillers.soft, word];
      save();
    }
    newFiller = "";
  }

  function setVariants(i: number, value: string) {
    drafts[i] = value;
    text.brands.entries[i].variants = value
      .split(",")
      .map((v) => v.trim())
      .filter(Boolean);
    saveLater();
  }

  function removeBrand(i: number) {
    text.brands.entries.splice(i, 1);
    drafts = {};
    save();
  }

  function resetBrands() {
    confirmingReset = false;
    text.brands.entries = $state.snapshot(store.info!.default_brands);
    drafts = {};
    save();
  }

  $effect(() => {
    const config = $state.snapshot(text);
    const input = sample;
    if (!input.trim()) {
      result = "";
      return;
    }
    api.preview(config, input).then((out) => {
      if (input === sample) result = out;
    });
  });
</script>

<SectionTitle theme="fillers">{t("text.fillers")}</SectionTitle>
<div class="group">
  <label class="row">
    <span>{t("text.fillers_hard")}</span>
    <input type="checkbox" bind:checked={text.fillers.hard} onchange={save} />
  </label>
  <div class="row column">
    <span>{t("text.fillers_soft")}</span>
    <div class="words">
      {#each [...stock, ...custom] as word}
        <label class="check word">
          <input
            type="checkbox"
            checked={text.fillers.soft.includes(word)}
            onchange={(e) => toggleFiller(word, e.currentTarget.checked)}
          />
          {word}
        </label>
      {/each}
    </div>
    <form class="add" onsubmit={(e) => { e.preventDefault(); addFiller(); }}>
      <input type="text" bind:value={newFiller} placeholder={t("text.fillers_custom")} />
      <button type="submit" disabled={!newFiller.trim()}>{t("add")}</button>
    </form>
  </div>
</div>
<p class="note">{t("text.fillers_soft_hint")}</p>

<SectionTitle theme="commands">{t("text.commands")}</SectionTitle>
<div class="group">
  <label class="row">
    <div class="label">
      <span>{t("text.commands")}</span>
      <span class="muted small">{t("text.commands_hint")}</span>
    </div>
    <input type="checkbox" bind:checked={text.commands.enabled} onchange={save} />
  </label>
  <label class="row">
    <span>{t("text.spoken_punctuation")}</span>
    <input type="checkbox" bind:checked={text.commands.spoken_punctuation} onchange={save} />
  </label>
  <div class="row">
    <button class="link" onclick={() => (cheatOpen = !cheatOpen)}>
      {cheatOpen ? t("text.cheatsheet_hide") : t("text.cheatsheet", { n: COMMANDS.length })}
    </button>
  </div>
  {#if cheatOpen}
    {#each COMMANDS as [say, get]}
      <div class="row cmd"><span>{say}</span><span class="get">{get}</span></div>
    {/each}
  {/if}
</div>

<SectionTitle theme="profanity">{t("text.profanity")}</SectionTitle>
<div class="group">
  <div class="row">
    <span>{t("text.profanity")}</span>
    <div class="control">
      <Dropdown
        value={text.profanity}
        options={modes.map((mode) => ({ value: mode, label: t(`text.profanity_${mode}`) }))}
        onchange={(v) => { text.profanity = v; save(); }}
      />
    </div>
  </div>
</div>

<SectionTitle theme="dictionary">{t("text.dictionary")}</SectionTitle>
<p class="note first">{t("text.dictionary_hint")}</p>
<div class="group">
  {#each text.dictionary.entries as entry, i}
    <div class="row pair">
      <input type="text" bind:value={entry.from} placeholder={t("text.dictionary_from")} oninput={saveLater} />
      <span class="muted">→</span>
      <input type="text" bind:value={entry.to} placeholder={t("text.dictionary_to")} oninput={saveLater} />
      <button class="ghost" title={t("remove")} onclick={() => { text.dictionary.entries.splice(i, 1); save(); }}>✕</button>
    </div>
  {/each}
  <div class="row">
    <button onclick={() => text.dictionary.entries.push({ from: "", to: "" })}>{t("add")}</button>
  </div>
</div>

<SectionTitle theme="brands">{t("text.brands")}</SectionTitle>
<div class="group">
  <label class="row">
    <div class="label">
      <span>{t("text.brands")}</span>
      <span class="muted small">{t("text.brands_hint")}</span>
    </div>
    <input type="checkbox" bind:checked={text.brands.enabled} onchange={save} />
  </label>
  {#if text.brands.enabled}
    <div class="row">
      <button class="link" onclick={() => (brandsOpen = !brandsOpen)}>
        {brandsOpen ? t("text.brands_hide") : t("text.brands_show", { n: text.brands.entries.length })}
      </button>
    </div>
    {#if brandsOpen}
      {#each text.brands.entries as term, i}
        <div class="row pair">
          <input class="name" type="text" bind:value={term.name} placeholder={t("text.brands_name")} oninput={saveLater} />
          <input
            type="text"
            bind:value={() => drafts[i] ?? term.variants.join(", "), (v) => setVariants(i, v)}
            placeholder={t("text.brands_variants")}
            onblur={() => delete drafts[i]}
          />
          <button class="ghost" title={t("remove")} onclick={() => removeBrand(i)}>✕</button>
        </div>
      {/each}
      <div class="row">
        {#if confirmingReset}
          <span>{t("text.brands_reset_confirm")}</span>
          <button class="primary" onclick={resetBrands}>{t("text.brands_reset")}</button>
          <button onclick={() => (confirmingReset = false)}>{t("general.hotkey_cancel")}</button>
        {:else}
          <button onclick={() => text.brands.entries.push({ name: "", variants: [] })}>{t("add")}</button>
          <button class="ghost" onclick={() => (confirmingReset = true)}>{t("text.brands_reset")}</button>
        {/if}
      </div>
    {/if}
  {/if}
  <label class="row">
    <span>{t("text.ambiguous")}</span>
    <input type="checkbox" bind:checked={text.ambiguous.enabled} onchange={save} />
  </label>
</div>
<p class="note warn">{t("text.ambiguous_warn")}</p>

<SectionTitle theme="snippets">{t("text.snippets")}</SectionTitle>
<p class="note first">{t("text.snippets_hint")}</p>
<div class="group">
  {#each text.snippets.entries as snippet, i}
    <div class="row snippet">
      <div class="fields">
        <input type="text" bind:value={snippet.trigger} placeholder={t("text.snippets_trigger")} oninput={saveLater} />
        <textarea rows="3" bind:value={snippet.text} placeholder={t("text.snippets_text")} oninput={saveLater}></textarea>
      </div>
      <button class="ghost" title={t("remove")} onclick={() => { text.snippets.entries.splice(i, 1); save(); }}>✕</button>
    </div>
  {/each}
  <div class="row">
    <button onclick={() => text.snippets.entries.push({ trigger: "", text: "" })}>{t("add")}</button>
  </div>
</div>

<SectionTitle theme="formatting">{t("text.formatting")}</SectionTitle>
<div class="group">
  <label class="row">
    <span>{t("text.trailing_space")}</span>
    <input type="checkbox" bind:checked={text.cleanup.trailing_space} onchange={save} />
  </label>
</div>

<SectionTitle theme="check">{t("text.preview")}</SectionTitle>
<div class="group">
  <div class="row column">
    <textarea rows="2" bind:value={sample} placeholder={t("text.preview_placeholder")}></textarea>
    {#if result}
      <p class="result">{result}</p>
    {/if}
  </div>
  <div class="row voice">
    <button class="mic" class:listening={voice === "listening"} disabled={voice === "working"} onclick={voiceCheck}>
      <span class="dot"></span>
      {voice === "listening" ? t("text.voice_listening") : voice === "working" ? t("text.voice_working") : t("text.voice_check")}
    </button>
  </div>
  {#if heard}
    <div class="row column heard">
      {#if heard.raw.trim()}
        <span class="muted small">{t("text.voice_raw")}</span>
        <p class="raw">{heard.raw}</p>
        <span class="muted small">{t("text.voice_result")}</span>
        <p class="result">{heard.text}</p>
      {:else}
        <span class="muted">{t("text.voice_empty")}</span>
      {/if}
    </div>
  {/if}
</div>

<style>
  .column {
    flex-direction: column;
    align-items: stretch;
  }

  .words {
    display: flex;
    flex-wrap: wrap;
    gap: 6px 16px;
  }

  .word {
    min-width: 96px;
  }

  /* Many switches in a row: smaller, so the words stay the main thing. */
  .word input {
    transform: scale(0.82);
    transform-origin: left center;
  }

  .add {
    display: flex;
    gap: 8px;
  }

  .add input {
    flex: 1;
  }

  .pair input {
    flex: 1;
    min-width: 0;
  }

  .pair input.name {
    flex: 0 0 150px;
  }

  .snippet {
    align-items: flex-start;
  }

  .fields {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .note.first {
    margin: -4px 0 8px;
  }

  .cmd {
    min-height: 40px;
    padding-top: 8px;
    padding-bottom: 8px;
  }

  .get {
    font-family: var(--mono);
    font-size: 12px;
    color: var(--accent);
    text-align: right;
  }

  .voice {
    justify-content: flex-start;
  }

  .mic {
    display: inline-flex;
    align-items: center;
    gap: 10px;
  }

  .dot {
    width: 10px;
    height: 10px;
    border-radius: 50%;
    background: var(--accent);
    box-shadow: 0 0 8px var(--accent-glow);
  }

  @media (prefers-reduced-motion: no-preference) {
    .mic.listening .dot {
      animation: pulse 1s ease-in-out infinite;
    }
  }

  @keyframes pulse {
    50% {
      transform: scale(1.6);
      box-shadow: 0 0 16px var(--accent-glow);
    }
  }

  .raw {
    margin: 0 0 6px;
    font-family: var(--mono);
    font-size: 12px;
    color: var(--muted);
    user-select: text;
  }

  .result {
    white-space: pre-wrap;
    user-select: text;
    padding: 6px 8px;
    border-left: 2px solid var(--accent);
  }
</style>
