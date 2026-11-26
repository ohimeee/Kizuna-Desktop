<script lang="ts" module>
  // Snackbar-style messages. `toast("text")` from anywhere.
  interface Toast { id: number; text: string; action?: { label: string; run: () => void } }
  let list = $state<Toast[]>([]);
  let next = 0;

  export function toast(text: string, action?: Toast["action"], ms = 4000) {
    const id = next++;
    list.push({ id, text, action });
    setTimeout(() => (list = list.filter((t) => t.id !== id)), ms);
  }
</script>

<div class="toasts">
  {#each list as t (t.id)}
    <div class="toast body-medium">
      <span>{t.text}</span>
      {#if t.action}
        <button class="btn text" onclick={() => { t.action?.run(); list = list.filter((x) => x.id !== t.id); }}>{t.action.label}</button>
      {/if}
    </div>
  {/each}
</div>

<style>
  .toasts {
    position: fixed;
    left: 50%;
    bottom: 24px;
    transform: translateX(-50%);
    display: flex;
    flex-direction: column;
    gap: 8px;
    z-index: 1000;
    pointer-events: none;
  }
  .toast {
    display: flex;
    align-items: center;
    gap: 12px;
    min-width: 280px;
    max-width: 640px;
    padding: 12px 16px;
    border-radius: 4px;
    background: var(--inverse-surface);
    color: var(--inverse-on-surface);
    box-shadow: 0 3px 8px rgb(0 0 0 / 0.3);
    pointer-events: auto;
    animation: in 200ms var(--ease);
  }
  .toast :global(.btn.text) { color: var(--inverse-primary); height: 32px; }
  @keyframes in { from { opacity: 0; transform: translateY(8px); } }
</style>
