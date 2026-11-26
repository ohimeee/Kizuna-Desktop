<script lang="ts">
  import { onMount } from "svelte";
  import { api } from "$lib/api";
  import Icon from "$lib/components/Icon.svelte";
  import { downloads } from "$lib/downloads.svelte";
  import { downloadLocation } from "$lib/downloadLocation.svelte";

  let size = $state<number | null>(null);
  onMount(async () => {
    await downloads.refresh();
    size = await api.downloadsSize();
  });
  $effect(() => {
    downloads.lastDone;
    api.downloadsSize().then((s) => (size = s));
  });

  function fmtSize(b: number) {
    if (b < 1024) return `${b} B`;
    if (b < 1024 ** 2) return `${(b / 1024).toFixed(0)} KB`;
    return `${(b / 1024 ** 2).toFixed(1)} MB`;
  }

  async function togglePause() {
    await api.downloadsSetPaused(!downloads.paused);
    await downloads.refresh();
  }
</script>

<div class="screen">
  <header class="top-bar">
    <a class="icon-btn" href="/more" title="Back"><Icon name="arrow_back" /></a>
    <h1>Download queue</h1>
    {#if size !== null}<span class="body-small muted">{fmtSize(size)} on disk</span>{/if}
    <button class="icon-btn" title="Open downloads folder" onclick={() => downloadLocation.openFolder()}><Icon name="folder" /></button>
    <button class="icon-btn" title={downloads.paused ? "Resume" : "Pause"} disabled={!downloads.count} onclick={togglePause}>
      <Icon name={downloads.paused ? "play_arrow" : "schedule"} />
    </button>
    <button class="icon-btn" title="Cancel all" disabled={!downloads.queue.length} onclick={() => api.downloadsClear()}><Icon name="delete" /></button>
  </header>
  <div class="scroll">
    {#if !downloads.count}
      <div class="empty"><div class="face">(・ω・)</div><div>No downloads</div></div>
    {:else}
      <ul>
        {#if downloads.active}
          <li class="active">
            <span class="mini-spin"></span>
            <div class="text">
              <div class="body-medium ellipsis">{downloads.active.chapterName}</div>
              <div class="body-small muted ellipsis">{downloads.active.novelTitle}</div>
            </div>
          </li>
        {/if}
        {#each downloads.queue as q (q.chapterId)}
          <li>
            <Icon name="schedule" size={20} />
            <div class="text">
              <div class="body-medium ellipsis">{q.chapterName}</div>
              <div class="body-small muted ellipsis">{q.novelTitle}</div>
            </div>
            <button class="icon-btn" title="Cancel" onclick={() => api.downloadsCancel([q.chapterId])}><Icon name="close" size={20} /></button>
          </li>
        {/each}
      </ul>
      {#if downloads.paused}<p class="body-small muted note">Paused</p>{/if}
    {/if}
  </div>
</div>

<style>
  ul { list-style: none; margin: 0; padding: 8px 16px; }
  li { display: flex; align-items: center; gap: 16px; padding: 8px 12px; border-radius: var(--radius-m); color: var(--on-surface-variant); }
  li.active { background: var(--secondary-container); color: var(--on-secondary-container); }
  .text { flex: 1; min-width: 0; color: var(--on-surface); }
  .ellipsis { overflow: hidden; white-space: nowrap; text-overflow: ellipsis; }
  .note { padding: 0 28px; }
  .mini-spin { width: 20px; height: 20px; border: 2px solid var(--outline-variant); border-top-color: var(--primary); border-radius: 50%; animation: spin 0.9s linear infinite; }
</style>
