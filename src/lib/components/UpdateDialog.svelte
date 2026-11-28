<script lang="ts">
  import Dialog from "./Dialog.svelte";
  import Icon from "./Icon.svelte";
  import { updater } from "$lib/updater.svelte";
  import { renderReleaseNotes } from "$lib/releaseNotes";

  const notes = $derived(updater.available?.body ? renderReleaseNotes(updater.available.body) : "");
  const released = $derived.by(() => {
    const d = updater.available?.date ? new Date(updater.available.date.replace(" ", "T").replace(/ .*$/, "")) : null;
    return d && !isNaN(d.getTime()) ? d.toLocaleDateString(undefined, { year: "numeric", month: "long", day: "numeric" }) : "";
  });
</script>

{#if updater.available}
  <Dialog bind:open={updater.dialogOpen} title="Update available" width={560}>
    <div class="hero">
      <div class="badge"><Icon name="download" size={22} /></div>
      <div>
        <p class="title-medium ver">Kizuna {updater.available.version}</p>
        <p class="body-small muted">
          You have {updater.available.currentVersion}{released ? ` · released ${released}` : ""}
        </p>
      </div>
    </div>

    {#if notes}
      <p class="label-large section">What's new</p>
      <!-- renderReleaseNotes escapes all text before adding its own tags -->
      <div class="notes">{@html notes}</div>
    {/if}

    {#if updater.installing}
      <div class="bar"><div class="fill" style:width="{Math.round(updater.progress * 100)}%"></div></div>
      <p class="body-small muted">Downloading {Math.round(updater.progress * 100)}%. The app restarts when it's done.</p>
    {/if}
    {#if updater.lastError}<p class="body-small err">{updater.lastError}</p>{/if}
    {#snippet actions()}
      <button class="btn text" disabled={updater.installing} onclick={() => (updater.dialogOpen = false)}>Later</button>
      <button class="btn" disabled={updater.installing} onclick={() => updater.install()}>Update now</button>
    {/snippet}
  </Dialog>
{/if}

<style>
  .hero { display: flex; align-items: center; gap: 14px; margin-bottom: 18px; }
  .badge {
    flex: none; display: grid; place-items: center; width: 44px; height: 44px; border-radius: 50%;
    background: var(--primary-container); color: var(--on-primary-container);
  }
  .ver { margin: 0; color: var(--on-surface); }
  .muted { margin: 2px 0 0; color: var(--on-surface-variant); }
  .section { margin: 0 0 8px; color: var(--on-surface); }
  .notes {
    max-height: min(52vh, 440px);
    overflow-y: auto;
    overscroll-behavior: contain;
    padding: 4px 18px 14px 16px;
    border-radius: var(--radius-m);
    background: var(--surface-container);
    color: var(--on-surface-variant);
    line-height: 1.55;
  }
  .notes :global(h3) {
    margin: 14px 0 6px;
    font-size: 0.75rem;
    font-weight: 700;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--primary);
  }
  .notes :global(p) { margin: 12px 0 0; }
  .notes :global(ul) { margin: 0; padding-left: 20px; }
  .notes :global(li) { margin: 6px 0; }
  .notes :global(li::marker) { color: var(--primary); }
  .notes :global(strong) { color: var(--on-surface); font-weight: 600; }
  .notes :global(code) {
    padding: 1px 5px; border-radius: 4px; font-size: 0.9em;
    background: var(--surface-container-highest); color: var(--on-surface);
  }
  .bar { height: 6px; margin-top: 16px; border-radius: 3px; background: var(--secondary-container); overflow: hidden; }
  .fill { height: 100%; background: var(--primary); transition: width 150ms; }
  .err { color: var(--error); }
</style>
