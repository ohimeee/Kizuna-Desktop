<script lang="ts">
  import type { Snippet } from "svelte";

  let {
    open = $bindable(false),
    title,
    children,
    actions,
    width = 400,
  }: { open: boolean; title: string; children: Snippet; actions?: Snippet; width?: number } = $props();

  function onKey(e: KeyboardEvent) {
    if (open && e.key === "Escape") open = false;
  }
</script>

<svelte:window onkeydown={onKey} />

{#if open}
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div class="scrim" onclick={() => (open = false)}>
    <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
    <div class="dialog" style:width="{width}px" onclick={(e) => e.stopPropagation()} role="dialog" tabindex="-1" aria-modal="true" aria-label={title}>
      <h2 class="title-large">{title}</h2>
      <div class="content body-medium">{@render children()}</div>
      {#if actions}<div class="actions">{@render actions()}</div>{/if}
    </div>
  </div>
{/if}

<style>
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 500;
    display: grid;
    place-items: center;
    background: rgb(0 0 0 / 0.4);
    animation: fade 150ms;
  }
  .dialog {
    max-width: calc(100vw - 48px);
    max-height: calc(100vh - 48px);
    display: flex;
    flex-direction: column;
    padding: 24px;
    border-radius: var(--radius-xl);
    background: var(--surface-container-high);
    color: var(--on-surface);
    box-shadow: 0 8px 24px rgb(0 0 0 / 0.3);
    animation: pop 200ms var(--ease);
  }
  h2 {
    margin: 0 0 16px;
  }
  .content {
    min-height: 0; /* lets the content shrink and scroll inside the dialog's max-height */
    overflow-y: auto;
    color: var(--on-surface-variant);
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 24px;
  }
  @keyframes fade { from { opacity: 0; } }
  @keyframes pop { from { opacity: 0; transform: scale(0.95); } }
</style>
