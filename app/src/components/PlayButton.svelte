<script lang="ts">
  /// `small` is the one inside a dropdown option.
  let { label, onplay, small = false }: { label: string; onplay: () => void; small?: boolean } = $props();

  // Restarted on every press, so a second press rings again.
  let ring = $state(0);

  function play(e: MouseEvent) {
    // Inside a dropdown option the click must not choose the option too.
    e.stopPropagation();
    onplay();
    ring += 1;
  }
</script>

<button type="button" class="play" class:small aria-label={label} title={label} onclick={play}>
  {#key ring}
    {#if ring}<span class="ring"></span>{/if}
  {/key}
</button>

<style>
  .play {
    position: relative;
    display: grid;
    place-items: center;
    flex: none;
    width: 32px;
    height: 32px;
    padding: 0;
    border-radius: 50%;
    background: var(--yolk);
    box-shadow:
      inset 0 1px 0 rgba(255, 255, 255, 0.3),
      0 3px 0 var(--yolk-edge),
      0 6px 14px -6px var(--accent-glow);
  }

  .play:active:not(:disabled) {
    box-shadow:
      inset 0 1px 0 rgba(255, 255, 255, 0.3),
      0 0 0 var(--yolk-edge);
  }

  .play::before {
    content: "";
    width: 10px;
    height: 11px;
    margin-left: 2px;
    background: #fff;
    clip-path: polygon(0 0, 100% 50%, 0 100%);
  }

  .play.small {
    width: 24px;
    height: 24px;
    background: var(--btn);
    box-shadow:
      inset 0 1px 0 var(--btn-hi),
      0 0 0 1px var(--line);
    transform: none;
  }

  .play.small::before {
    width: 8px;
    height: 9px;
    background: var(--text);
  }

  .play.small:hover {
    background: var(--yolk);
  }

  .play.small:hover::before {
    background: #fff;
  }

  .ring {
    position: absolute;
    inset: -2px;
    border-radius: 50%;
    border: 2px solid var(--accent);
    opacity: 0;
    pointer-events: none;
  }

  @media (prefers-reduced-motion: no-preference) {
    .ring {
      animation: ring 1s ease-out;
    }
  }

  @keyframes ring {
    from {
      opacity: 1;
      transform: scale(1);
    }
    to {
      opacity: 0;
      transform: scale(1.8);
    }
  }
</style>
