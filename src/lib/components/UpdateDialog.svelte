<script lang="ts">
  import Dialog from "./Dialog.svelte";
  import { updater } from "$lib/updater.svelte";
</script>

{#if updater.available}
  <Dialog bind:open={updater.dialogOpen} title="Update available" width={480}>
    <p class="body-large ver">Kizuna {updater.available.version}</p>
    {#if updater.available.body}
      <div class="notes body-medium">{updater.available.body}</div>
    {/if}
    {#if updater.installing}
      <div class="bar"><div class="fill" style:width="{Math.round(updater.progress * 100)}%"></div></div>
      <p class="body-small">Downloading… the app restarts when it's done.</p>
    {/if}
    {#if updater.lastError}<p class="body-small err">{updater.lastError}</p>{/if}
    {#snippet actions()}
      <button class="btn text" disabled={updater.installing} onclick={() => (updater.dialogOpen = false)}>Later</button>
      <button class="btn" disabled={updater.installing} onclick={() => updater.install()}>Update</button>
    {/snippet}
  </Dialog>
{/if}

<style>
  .ver { margin: 0 0 8px; color: var(--on-surface); }
  .notes { white-space: pre-line; max-height: 240px; overflow-y: auto; padding: 12px; border-radius: var(--radius-m); background: var(--surface-container); }
  .bar { height: 6px; margin-top: 16px; border-radius: 3px; background: var(--secondary-container); overflow: hidden; }
  .fill { height: 100%; background: var(--primary); transition: width 150ms; }
  .err { color: var(--error); }
</style>
