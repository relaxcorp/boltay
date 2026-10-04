<script lang="ts">
  import { locale, t } from "../lib/i18n.svelte";

  /// Days are local midnights in milliseconds; only days with dictations can be picked.
  let {
    days,
    picked,
    onpick,
  }: { days: Set<number>; picked: number | null; onpick: (day: number) => void } = $props();

  const startOf = (d: Date) => new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime();
  let open = $state(false);
  let root = $state<HTMLDivElement>();
  let month = $state(new Date(new Date().getFullYear(), new Date().getMonth(), 1));
  const today = startOf(new Date());

  // Weeks start on Monday.
  const cells = $derived.by(() => {
    const lead = (month.getDay() + 6) % 7;
    const count = new Date(month.getFullYear(), month.getMonth() + 1, 0).getDate();
    return [
      ...Array<null>(lead).fill(null),
      ...Array.from({ length: count }, (_, i) => new Date(month.getFullYear(), month.getMonth(), i + 1).getTime()),
    ];
  });
  const weekdays = $derived(
    Array.from({ length: 7 }, (_, i) => new Date(2024, 0, 1 + i).toLocaleDateString(locale.lang, { weekday: "short" })),
  );

  const shift = (by: number) => (month = new Date(month.getFullYear(), month.getMonth() + by, 1));

  function pick(day: number) {
    open = false;
    onpick(day);
  }

  $effect(() => {
    if (!open) return;
    const outside = (e: PointerEvent) => {
      if (!root?.contains(e.target as Node)) open = false;
    };
    const escape = (e: KeyboardEvent) => e.key === "Escape" && (open = false);
    document.addEventListener("pointerdown", outside, true);
    document.addEventListener("keydown", escape);
    return () => {
      document.removeEventListener("pointerdown", outside, true);
      document.removeEventListener("keydown", escape);
    };
  });
</script>

<div class="wrap" bind:this={root}>
  <button class="chip" class:on={picked !== null} aria-expanded={open} onclick={() => (open = !open)}>
    <svg viewBox="0 0 24 24" aria-hidden="true"><rect x="3.5" y="5" width="17" height="15" rx="3" /><path d="M3.5 10h17M8 3v4M16 3v4" /></svg>
    {picked !== null ? new Date(picked).toLocaleDateString(locale.lang, { day: "numeric", month: "short" }) : t("history.pick")}
  </button>
  {#if open}
    <div class="calendar" role="dialog">
      <div class="head">
        <button class="ghost" aria-label="‹" onclick={() => shift(-1)}>‹</button>
        <span>{month.toLocaleDateString(locale.lang, { month: "long", year: "numeric" })}</span>
        <button class="ghost" aria-label="›" onclick={() => shift(1)}>›</button>
      </div>
      <div class="grid">
        {#each weekdays as day}<span class="weekday">{day}</span>{/each}
        {#each cells as day}
          {#if day === null}
            <span></span>
          {:else}
            <button
              class="day"
              class:has={days.has(day)}
              class:today={day === today}
              class:picked={day === picked}
              disabled={!days.has(day)}
              onclick={() => pick(day)}
            >
              {new Date(day).getDate()}
            </button>
          {/if}
        {/each}
      </div>
    </div>
  {/if}
</div>

<style>
  .wrap {
    position: relative;
  }

  .chip {
    display: inline-flex;
    align-items: center;
    gap: 7px;
  }

  svg {
    width: 13px;
    height: 13px;
    fill: none;
    stroke: currentColor;
    stroke-width: 2;
    stroke-linecap: round;
  }

  .calendar {
    position: absolute;
    right: 0;
    top: calc(100% + 8px);
    z-index: 40;
    width: 250px;
    padding: 12px;
    border-radius: 16px;
    background: var(--plate);
    box-shadow:
      inset 0 1px 0 var(--plate-hi),
      0 0 0 1px var(--line),
      0 3px 0 var(--plate-edge),
      0 24px 40px -12px rgba(0, 0, 0, 0.55);
  }

  .head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 8px;
    font-weight: 600;
    text-transform: capitalize;
  }

  .head button {
    padding: 2px 10px;
    font-size: 16px;
  }

  .grid {
    display: grid;
    grid-template-columns: repeat(7, 1fr);
    gap: 3px;
  }

  .weekday {
    padding-bottom: 4px;
    font-size: 10.5px;
    color: var(--muted);
    text-align: center;
    text-transform: uppercase;
  }

  /* Days are plain cells, not keycaps. */
  .day {
    position: relative;
    display: grid;
    place-items: center;
    height: 30px;
    padding: 0;
    border-radius: 9px;
    font-size: 12.5px;
    color: var(--muted);
    background: none;
    box-shadow: none;
    transform: none;
  }

  .day:disabled {
    opacity: 0.4;
  }

  .day.has {
    color: var(--text);
    cursor: pointer;
  }

  .day.has:hover {
    background: var(--hover);
    filter: none;
  }

  .day.has::after {
    content: "";
    position: absolute;
    left: 50%;
    bottom: 4px;
    width: 4px;
    height: 4px;
    margin-left: -2px;
    border-radius: 50%;
    background: var(--accent);
    box-shadow: 0 0 6px var(--accent-glow);
  }

  .day.today {
    box-shadow: inset 0 0 0 1px var(--line);
  }

  .day.picked {
    color: #fff;
    background: var(--yolk);
  }

  .day.picked::after {
    background: #fff;
    box-shadow: none;
  }

  .day:active:not(:disabled) {
    transform: none;
    box-shadow: none;
  }

  @media (prefers-reduced-motion: no-preference) {
    .calendar {
      animation: drop 0.16s cubic-bezier(0.3, 0.8, 0.3, 1);
    }
  }

  @keyframes drop {
    from {
      opacity: 0;
      transform: translateY(-4px) scale(0.98);
    }
  }
</style>
