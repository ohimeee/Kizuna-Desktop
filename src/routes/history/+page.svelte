<script lang="ts">
  import { onMount } from "svelte";
  import { api, type HistoryItem } from "$lib/api";
  import Dialog from "$lib/components/Dialog.svelte";
  import Icon from "$lib/components/Icon.svelte";
  import { readerHref } from "$lib/api";
  import { coverSrc } from "$lib/manga";
  import { formatTime, groupByDay } from "$lib/format";

  let items = $state<HistoryItem[]>([]);
  let loaded = $state(false);
  let query = $state("");
  let confirmClear = $state(false);

  onMount(load);
  async function load() {
    items = await api.historyList();
    loaded = true;
  }

  async function remove(h: HistoryItem) {
    await api.historyRemove(h.chapter.novelId);
    items = items.filter((i) => i.chapter.novelId !== h.chapter.novelId);
  }

  async function clearAll() {
    await api.historyClear();
    items = [];
    confirmClear = false;
  }

  const shown = $derived(query.trim() ? items.filter((i) => i.novelTitle.toLowerCase().includes(query.trim().toLowerCase())) : items);
  const groups = $derived(groupByDay(shown, (i) => i.lastRead));
</script>

<div class="screen">
  <header class="top-bar">
    <h1>History</h1>
    <input class="text-field search" placeholder="Search history" bind:value={query} />
    <button class="icon-btn" title="Clear history" disabled={!items.length} onclick={() => (confirmClear = true)}><Icon name="delete" /></button>
  </header>
  <div class="scroll">
    {#if loaded && items.length === 0}
      <div class="empty"><div class="face">(・o・;)</div><div class="body-large">Nothing read recently</div></div>
    {/if}
    {#each groups as g}
      <h2 class="day label-large">{g.day}</h2>
      <ul>
        {#each g.items as h (h.chapter.id)}
          <li>
            <a class="row" href={readerHref(h.kind, h.chapter.id)}>
              <div class="thumb">{#if h.novelCover}<img src={coverSrc(h.kind, h.sourceId, h.novelCover)} alt="" referrerpolicy="no-referrer" />{/if}</div>
              <div class="text">
                <div class="body-medium ellipsis">{h.novelTitle}</div>
                <!-- "Ch. 53 - 2:09 PM • 34%": percent only for 1..99, like Android. -->
                <div class="body-small muted ellipsis">
                  {h.chapter.name} - {formatTime(h.lastRead)}{#if h.chapter.progress >= 1 && h.chapter.progress <= 99} • {h.chapter.progress}%{/if}
                </div>
              </div>
            </a>
            <a class="icon-btn" href="/novel/{h.chapter.novelId}" title="Open novel"><Icon name="info" size={20} /></a>
            <button class="icon-btn" title="Remove from history" onclick={() => remove(h)}><Icon name="delete" size={20} /></button>
          </li>
        {/each}
      </ul>
    {/each}
  </div>
</div>

<Dialog bind:open={confirmClear} title="Clear history?">
  This removes all reading history. Reading progress in chapters is kept.
  {#snippet actions()}
    <button class="btn text" onclick={() => (confirmClear = false)}>Cancel</button>
    <button class="btn" onclick={clearAll}>Clear</button>
  {/snippet}
</Dialog>

<style>
  .search { width: min(320px, 40vw); }
  .day { margin: 16px 24px 4px; color: var(--on-surface-variant); }
  ul { list-style: none; margin: 0; padding: 0 12px; max-width: 900px; }
  li { display: flex; align-items: center; border-radius: var(--radius-m); padding-right: 8px; }
  li:hover { background: color-mix(in srgb, var(--on-surface) 6%, transparent); }
  .row { flex: 1; min-width: 0; display: flex; align-items: center; gap: 16px; padding: 8px 12px; }
  .thumb { width: 40px; height: 56px; flex: none; border-radius: 6px; overflow: hidden; background: var(--surface-container-high); }
  .thumb img { width: 100%; height: 100%; object-fit: cover; }
  .text { min-width: 0; }
  .ellipsis { overflow: hidden; white-space: nowrap; text-overflow: ellipsis; }
</style>
