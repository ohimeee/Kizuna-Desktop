<script lang="ts">
  // A chapter's download state as one circle, like Mihon: outline + arrow (download), dashed
  // (queued), a ring that fills with progress (downloading), filled check (downloaded), error.
  type State = "none" | "queued" | "downloading" | "done" | "failed";

  let {
    state,
    progress,
    title,
    onclick,
  }: { state: State; progress?: number; title: string; onclick?: () => void } = $props();

  const R = 9;
  const C = 2 * Math.PI * R;
  const known = $derived(progress !== undefined && progress > 0);
  const dash = $derived(known ? `${Math.max(0.04, Math.min(1, progress!)) * C} ${C}` : `${C * 0.25} ${C}`);
</script>

<button class="icon-btn dlc {state}" {title} aria-label={title} disabled={state === "downloading"} {onclick}>
  <svg viewBox="0 0 24 24" width="22" height="22" aria-hidden="true">
    {#if state === "done"}
      <circle cx="12" cy="12" r="10" class="fill" />
      <path d="M7.5 12.3l3 3 6-6.3" class="mark" />
    {:else if state === "failed"}
      <circle cx="12" cy="12" r="10" class="fill" />
      <path d="M12 7v6.2M12 16.6v.4" class="mark" />
    {:else if state === "downloading"}
      <circle cx="12" cy="12" r={R} class="track" />
      <circle cx="12" cy="12" r={R} class="arc" class:indeterminate={!known} stroke-dasharray={dash} transform="rotate(-90 12 12)" />
      <path d="M12 8.5v6M9.5 12.5l2.5 2.5 2.5-2.5" class="arrow small" />
    {:else}
      <circle cx="12" cy="12" r={R} class="ring" class:dashed={state === "queued"} />
      <path d="M12 7.5v8M8.8 12.6l3.2 3.2 3.2-3.2" class="arrow" />
    {/if}
  </svg>
</button>

<style>
  .dlc { flex: none; color: var(--on-surface-variant); }
  .dlc:disabled { opacity: 1; cursor: default; }
  svg { display: block; overflow: visible; }
  .ring, .track, .arc { fill: none; stroke-width: 2; }
  .ring { stroke: currentColor; }
  .ring.dashed { stroke-dasharray: 3.2 2.6; transform-origin: 12px 12px; animation: turn 4s linear infinite; }
  .track { stroke: color-mix(in srgb, var(--primary) 22%, transparent); }
  .arc { stroke: var(--primary); stroke-linecap: round; transition: stroke-dasharray 200ms; }
  .arc.indeterminate { transform-origin: 12px 12px; animation: spin 0.9s linear infinite; }
  .arrow { fill: none; stroke: currentColor; stroke-width: 2; stroke-linecap: round; stroke-linejoin: round; }
  .downloading .arrow { stroke: var(--primary); }
  .arrow.small { stroke-width: 1.8; }
  .done { color: var(--tertiary); }
  .done .fill { fill: var(--tertiary); }
  .done .mark, .failed .mark { fill: none; stroke: var(--on-tertiary); stroke-width: 2.2; stroke-linecap: round; stroke-linejoin: round; }
  .failed .fill { fill: var(--error); }
  .failed .mark { stroke: var(--on-error); }
  .queued { color: var(--primary); }
  @keyframes spin { from { transform: rotate(-90deg); } to { transform: rotate(270deg); } }
  @keyframes turn { to { transform: rotate(360deg); } }
</style>
