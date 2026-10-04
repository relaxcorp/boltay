<script lang="ts">
  import { onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import Calendar from "../components/Calendar.svelte";
  import Dropdown from "../components/Dropdown.svelte";
  import { api, type Entry } from "../lib/api";
  import { locale, t } from "../lib/i18n.svelte";
  import { store } from "../lib/store.svelte";

  type Range = "all" | "today" | "yesterday" | "week" | "month" | "day";

  const s = $derived(store.settings!);
  let entries = $state<Entry[]>([]);
  let copied = $state<number | null>(null);
  let query = $state("");
  let range = $state<Range>("all");
  let picked = $state<number | null>(null);
  // window.confirm is not available in every webview, so the question is asked inline.
  let confirming = $state(false);

  const refresh = async () => (entries = await api.history());

  onMount(() => {
    refresh();
    const unlisten = listen("history-changed", refresh);
    return () => unlisten.then((f) => f());
  });

  const DAY = 86_400_000;
  const startOf = (at: number) => {
    const d = new Date(at);
    return new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime();
  };
  const today = startOf(Date.now());

  function inRange(day: number): boolean {
    switch (range) {
      case "all":
        return true;
      case "today":
        return day === today;
      case "yesterday":
        return day === today - DAY;
      case "week":
        return day > today - 7 * DAY;
      case "month":
        return day > today - 30 * DAY;
      case "day":
        return day === picked;
    }
  }

  const shown = $derived.by(() => {
    const q = query.trim().toLowerCase();
    return entries.filter((e) => inRange(startOf(e.at)) && (!q || e.text.toLowerCase().includes(q)));
  });

  // Newest first, so a day's dictations sit next to each other.
  const groups = $derived.by(() => {
    const out: { day: number; items: Entry[] }[] = [];
    for (const entry of [...shown].sort((a, b) => b.at - a.at)) {
      const day = startOf(entry.at);
      const last = out[out.length - 1];
      if (last?.day === day) last.items.push(entry);
      else out.push({ day, items: [entry] });
    }
    return out;
  });

  const days = $derived(new Set(entries.map((e) => startOf(e.at))));

  const words = (text: string) => text.split(/\s+/).filter(Boolean).length;
  const wordsSince = (since: number) =>
    entries.filter((e) => e.at >= since).reduce((sum, e) => sum + words(e.text), 0);
  const week = $derived(wordsSince(today - 6 * DAY));
  const month = $derived(wordsSince(today - 29 * DAY));

  /// Typing speed for the "saved" figure: 40 words a minute is an average typist.
  function saved(words: number): string {
    const minutes = Math.round(words / 40);
    const h = Math.floor(minutes / 60);
    const m = minutes % 60;
    return h ? t("history.hours", { h, m }) : t("history.minutes", { m });
  }

  function dayTitle(day: number): string {
    if (day === today) return t("history.today");
    if (day === today - DAY) return t("history.yesterday");
    return new Date(day).toLocaleDateString(locale.lang, { day: "numeric", month: "long", weekday: "short" });
  }

  const time = (at: number) => new Date(at).toLocaleTimeString(locale.lang, { hour: "2-digit", minute: "2-digit" });
  const number = (n: number) => n.toLocaleString(locale.lang);

  function choose(next: Range) {
    range = next;
    picked = null;
  }

  async function copy(entry: Entry) {
    await api.copy(entry.text);
    copied = entry.at;
    setTimeout(() => (copied = null), 1500);
  }

  async function remove(entry: Entry) {
    await api.deleteHistory(entry.at);
    await refresh();
  }

  async function clear() {
    confirming = false;
    await api.clearHistory();
    await refresh();
  }

  function setRetention(value: number | null) {
    s.history.retention_days = value;
    store.save(0);
  }

  const chips = ["all", "today", "yesterday", "week", "month"] as const;
</script>

{#if entries.length}
  <div class="group stats">
    <div class="stat">
      <b>{number(week)}</b>
      <span class="muted small">{t("history.stats_week")}</span>
    </div>
    <div class="stat">
      <b>{number(month)}</b>
      <span class="muted small">{t("history.stats_month")}</span>
    </div>
    <div class="stat">
      <b>≈ {saved(month)}</b>
      <span class="muted small">{t("history.stats_saved")}</span>
    </div>
  </div>
{/if}

<div class="group">
  <label class="row">
    <span>{t("history.enabled")}</span>
    <input type="checkbox" bind:checked={s.history.enabled} onchange={() => store.save(0)} />
  </label>
  <div class="row">
    <span>{t("history.retention")}</span>
    <div class="control">
      {#if confirming}
        <span>{t("history.clear_confirm")}</span>
        <button class="primary" onclick={clear}>{t("remove")}</button>
        <button onclick={() => (confirming = false)}>{t("general.hotkey_cancel")}</button>
      {:else}
        <!-- Hidden while the question is asked, so the answer buttons fit in the row. -->
        <Dropdown
          value={s.history.retention_days}
          options={[
            ...[7, 30, 90, 365].map((n) => ({ value: n as number | null, label: t("history.days", { n }) })),
            { value: null, label: t("history.forever") },
          ]}
          onchange={setRetention}
        />
        <button onclick={() => (confirming = true)} disabled={!entries.length}>{t("history.clear")}</button>
      {/if}
    </div>
  </div>
</div>

{#if !s.history.enabled}
  <p class="note">{t(store.info?.os === "macos" ? "history.off_mac" : "history.off")}</p>
{/if}

<div class="filters">
  <input class="search" type="text" bind:value={query} placeholder={t("history.search")} />
  <div class="chips">
    {#each chips as chip}
      <button class="chip" class:on={range === chip} onclick={() => choose(chip)}>{t(`history.${chip}`)}</button>
    {/each}
    <Calendar
      {days}
      picked={range === "day" ? picked : null}
      onpick={(day) => {
        picked = day;
        range = "day";
      }}
    />
  </div>
</div>

{#each groups as group (group.day)}
  <div class="day-title">
    <span>{dayTitle(group.day)}</span>
    <span class="count">{group.items.length}</span>
  </div>
  <div class="list">
    {#each group.items as entry (entry.at)}
      <div class="entry">
        <div class="meta">
          <span class="muted small">{time(entry.at)}</span>
          <div class="actions">
            <button class="ghost" onclick={() => copy(entry)}>{copied === entry.at ? t("copied") : t("copy")}</button>
            <button class="ghost" title={t("remove")} onclick={() => remove(entry)}>✕</button>
          </div>
        </div>
        <p class="text">{entry.text}</p>
      </div>
    {/each}
  </div>
{:else}
  <p class="muted empty">{entries.length ? t("history.nothing") : t("history.empty")}</p>
{/each}

<style>
  .stats {
    display: grid;
    grid-template-columns: 1fr 1fr 1.4fr;
    margin-bottom: 16px;
  }

  .stat {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 12px 18px;
  }

  .stat + .stat {
    border-left: 1px solid var(--line);
  }

  .stat b {
    font-size: 18px;
    font-weight: 650;
    letter-spacing: -0.01em;
  }

  .filters {
    position: relative;
    z-index: 20;
    display: flex;
    flex-direction: column;
    gap: 10px;
    margin: 20px 0 6px;
  }

  .search {
    width: 100%;
  }

  .chips {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
  }

  .day-title {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 18px 2px 8px;
    font-size: 12px;
    font-weight: 600;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--muted);
  }

  .count {
    padding: 1px 7px;
    border-radius: 999px;
    font-size: 11px;
    letter-spacing: 0;
    background: var(--hover);
  }

  .list {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .entry {
    border-radius: 14px;
    padding: 10px 14px 12px;
    background: var(--plate);
    box-shadow:
      inset 0 1px 0 var(--plate-hi),
      0 0 0 1px var(--line),
      0 2px 0 var(--plate-edge);
  }

  .meta {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .actions {
    display: flex;
    gap: 2px;
  }

  .actions button {
    padding: 2px 8px;
    font-size: 12px;
  }

  .text {
    white-space: pre-wrap;
    user-select: text;
    cursor: text;
  }

  .empty {
    padding: 24px 0;
    text-align: center;
  }
</style>
