<script lang="ts">
  import type { OverlayPosition } from "../lib/api";
  import { t } from "../lib/i18n.svelte";

  let { value, onchange }: { value: OverlayPosition; onchange: (v: OverlayPosition) => void } = $props();
  const options: OverlayPosition[] = ["bottom", "top", "cursor"];
</script>

<div class="picker" role="radiogroup" aria-label={t("general.overlay")}>
  {#each options as option}
    <button
      type="button"
      role="radio"
      aria-checked={value === option}
      class="option"
      class:on={value === option}
      onclick={() => onchange(option)}
    >
      <span class="screen {option}">
        <span class="lines"><i></i><i></i><i></i></span>
        {#if option === "cursor"}<span class="caret"></span>{/if}
        <span class="pill"></span>
      </span>
      <span class="name">{t(`general.overlay_${option}`)}</span>
    </button>
  {/each}
</div>

<style>
  .picker {
    display: flex;
    gap: 8px;
  }

  .option {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
    padding: 8px 8px 6px;
  }

  .screen {
    position: relative;
    display: block;
    width: 74px;
    height: 46px;
    overflow: hidden;
    border-radius: 6px;
    background: var(--field);
    box-shadow: inset 0 0 0 1px var(--line);
  }

  .lines {
    position: absolute;
    left: 9px;
    top: 12px;
    display: flex;
    flex-direction: column;
    gap: 5px;
  }

  .lines i {
    display: block;
    width: 40px;
    height: 3px;
    border-radius: 2px;
    background: var(--line);
  }

  .lines i:nth-child(2) {
    width: 30px;
  }

  .lines i:nth-child(3) {
    width: 22px;
  }

  .pill {
    position: absolute;
    left: 50%;
    width: 28px;
    height: 8px;
    margin-left: -14px;
    border-radius: 4px;
    background: var(--accent);
    box-shadow: 0 0 8px var(--accent-glow);
  }

  .bottom .pill {
    bottom: 5px;
  }

  .top .pill {
    top: 5px;
  }

  .cursor .pill {
    left: 44px;
    top: 28px;
    width: 22px;
    margin-left: 0;
  }

  .caret {
    position: absolute;
    left: 40px;
    top: 21px;
    width: 1.5px;
    height: 9px;
    background: var(--text);
  }

  .name {
    font-size: 12px;
    color: var(--muted);
  }

  .on .name {
    color: var(--text);
  }

  .on .screen {
    box-shadow:
      inset 0 0 0 1px var(--accent),
      0 0 12px -4px var(--accent-glow);
  }
</style>
