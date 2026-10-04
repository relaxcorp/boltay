<script lang="ts" module>
  export interface Option<T> {
    value: T;
    label: string;
  }

  let opened = 0;
</script>

<script lang="ts" generics="T">
  import { tick, type Snippet } from "svelte";

  let {
    options,
    value,
    onchange,
    label,
    disabled = false,
    item,
  }: {
    options: Option<T>[];
    value: T;
    onchange: (value: T) => void;
    /** For screen readers when no visible label sits next to it. */
    label?: string;
    disabled?: boolean;
    /** Extra content at the end of each option, e.g. a play button. */
    item?: Snippet<[Option<T>]>;
  } = $props();

  const id = `dd-${++opened}`;
  let open = $state(false);
  let active = $state(0);
  let button = $state<HTMLButtonElement>();
  let list = $state<HTMLDivElement>();
  let place = $state({ left: 0, top: 0, width: 0 });

  const selected = $derived(options.findIndex((o) => o.value === value));
  const current = $derived(options[selected]?.label ?? "");

  /** The list lives in <body>: plates clip what overflows them and their entry animation
   * would make a fixed child scroll with the plate. */
  function portal(node: HTMLElement) {
    document.body.appendChild(node);
    return { destroy: () => node.remove() };
  }

  async function show() {
    if (disabled || open) return;
    active = Math.max(0, selected);
    open = true;
    await tick();
    position();
  }

  function hide(refocus = false) {
    open = false;
    if (refocus) button?.focus();
  }

  // Below the button if it fits, above it otherwise.
  function position() {
    if (!button || !list) return;
    const r = button.getBoundingClientRect();
    const h = list.offsetHeight;
    const w = Math.max(r.width, list.offsetWidth);
    const below = window.innerHeight - r.bottom;
    place = {
      left: Math.max(8, Math.min(r.left, window.innerWidth - w - 8)),
      top: below > h + 12 || r.top < h + 12 ? r.bottom + 6 : r.top - h - 6,
      width: r.width,
    };
  }

  function choose(index: number) {
    const option = options[index];
    hide(true);
    if (option && option.value !== value) onchange(option.value);
  }

  function onkeydown(e: KeyboardEvent) {
    const last = options.length - 1;
    if (!open) {
      if (["ArrowDown", "ArrowUp", "Enter", " "].includes(e.key)) {
        e.preventDefault();
        show();
      }
      return;
    }
    switch (e.key) {
      case "ArrowDown":
        active = active >= last ? 0 : active + 1;
        break;
      case "ArrowUp":
        active = active <= 0 ? last : active - 1;
        break;
      case "Home":
        active = 0;
        break;
      case "End":
        active = last;
        break;
      case "Enter":
      case " ":
        choose(active);
        break;
      case "Escape":
        hide(true);
        break;
      case "Tab":
        hide();
        return;
      default:
        return;
    }
    e.preventDefault();
    e.stopPropagation();
  }

  // A click anywhere else, scrolling the page or resizing the window closes the list.
  $effect(() => {
    if (!open) return;
    const outside = (e: PointerEvent) => {
      const target = e.target as Node;
      if (!button?.contains(target) && !list?.contains(target)) hide();
    };
    const scrolled = (e: Event) => {
      if (!list?.contains(e.target as Node)) hide();
    };
    const resized = () => hide();
    document.addEventListener("pointerdown", outside, true);
    document.addEventListener("scroll", scrolled, true);
    window.addEventListener("resize", resized);
    window.addEventListener("blur", resized);
    return () => {
      document.removeEventListener("pointerdown", outside, true);
      document.removeEventListener("scroll", scrolled, true);
      window.removeEventListener("resize", resized);
      window.removeEventListener("blur", resized);
    };
  });
</script>

<button
  bind:this={button}
  type="button"
  class="dd"
  role="combobox"
  aria-haspopup="listbox"
  aria-expanded={open}
  aria-controls={id}
  aria-label={label}
  aria-activedescendant={open ? `${id}-${active}` : undefined}
  {disabled}
  onclick={() => (open ? hide() : show())}
  {onkeydown}
>
  <span>{current}</span>
  <svg viewBox="0 0 10 10" aria-hidden="true"><path d="M1.5 3.5L5 7l3.5-3.5" /></svg>
</button>

{#if open}
  <div
    bind:this={list}
    use:portal
    {id}
    class="list"
    role="listbox"
    style:left="{place.left}px"
    style:top="{place.top}px"
    style:min-width="{place.width}px"
  >
    {#each options as option, i}
      <div
        id="{id}-{i}"
        class="option"
        class:active={i === active}
        role="option"
        aria-selected={i === selected}
        tabindex="-1"
        onpointerenter={() => (active = i)}
        onclick={() => choose(i)}
        onkeydown={() => {}}
      >
        <span class="name">{option.label}</span>
        {#if item}{@render item(option)}{/if}
      </div>
    {/each}
  </div>
{/if}

<style>
  .dd {
    display: inline-flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    min-width: 150px;
    max-width: 280px;
    text-align: left;
  }

  .dd span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .dd svg {
    width: 9px;
    height: 9px;
    flex: none;
    fill: none;
    stroke: var(--muted);
    stroke-width: 1.6;
    stroke-linecap: round;
    stroke-linejoin: round;
  }

  .dd[aria-expanded="true"] svg {
    transform: rotate(180deg);
  }

  .list {
    position: fixed;
    z-index: 50;
    max-height: min(320px, calc(100vh - 24px));
    overflow-y: auto;
    padding: 6px;
    border-radius: 14px;
    background: var(--plate);
    box-shadow:
      inset 0 1px 0 var(--plate-hi),
      0 0 0 1px var(--line),
      0 3px 0 var(--plate-edge),
      0 24px 40px -12px rgba(0, 0, 0, 0.55);
    font: 13.5px/1.5 var(--sans);
    color: var(--text);
    user-select: none;
  }

  .option {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 7px 10px;
    border-radius: 9px;
    white-space: nowrap;
    cursor: pointer;
    outline: none;
  }

  /* The chosen option is marked with a glowing yolk dot. */
  .option::before {
    content: "";
    width: 6px;
    height: 6px;
    flex: none;
    border-radius: 50%;
  }

  .option[aria-selected="true"]::before {
    background: var(--accent);
    box-shadow: 0 0 8px var(--accent-glow);
  }

  .option.active {
    background: var(--hover);
  }

  .name {
    flex: 1;
  }

  @media (prefers-reduced-motion: no-preference) {
    .dd svg {
      transition: transform 0.2s;
    }

    .list {
      transform-origin: top;
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
