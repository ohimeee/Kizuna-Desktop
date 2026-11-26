<script lang="ts">
  import { onMount } from "svelte";
  import { api, errorText, type UpdateItem } from "$lib/api";
  import Icon from "$lib/components/Icon.svelte";
  import { readerHref } from "$lib/api";
  import { coverSrc } from "$lib/manga";
  import { toast } from "$lib/components/Toasts.svelte";
  import { groupByDay } from "$lib/format";

  let items = $state<UpdateItem[]>([]);
  let loaded = $state(false);
  let updating = $state(false);

  onMount(load);
  async function load() {
    items = await api.updatesList();
    loaded = true;
  }

  async function update() {
    updating = true;
    try {
      const r = await api.libraryUpdate();
      await load();
      toast(r.newChapters ? `${r.newChapters} new chapter${r.newChapters === 1 ? "" : "s"}` : "No new chapters");
      if (r.errors.length) toast(`${r.errors.length} failed: ${r.errors[0]}`, undefined, 8000);
    } catch (e) {
      toast(errorText(e));
    } finally {
      updating = false;
    }
  }

  async function markRead(u: UpdateItem) {
    await api.chaptersSetRead([u.chapter.id], !u.chapter.read);
    u.chapter.read = !u.chapter.read;
  }

  const groups = $derived(groupByDay(items, (i) => i.chapter.dateFetch));
</script>

<div class="screen">
  <header class="top-bar">
    <h1>Updates</h1>
    <button class="icon-btn" title="Update library" disabled={updating} onclick={update}><Icon name="refresh" class={updating ? "spin" : ""} /></button>
  </header>
  <div class="scroll">
    {#if loaded && items.length === 0}
      <div class="empty">
        <div class="face">(・o・;)</div>
        <div class="body-large">No recent updates</div>
        <div class="body-small">New chapters for novels in your library show up here after an update.</div>
      </div>
    {/if}
    {#each groups as g}
      <h2 class="day label-large">{g.day}</h2>
      <ul>
        {#each g.items as u (u.chapter.id)}
          <li class:read={u.chapter.read}>
            <a class="row" href={readerHref(u.kind, u.chapter.id)}>
              <div class="thumb">{#if u.novelCover}<img src={coverSrc(u.kind, u.sourceId, u.novelCover)} alt="" referrerpolicy="no-referrer" />{/if}</div>
              <div class="text">
                <div class="body-medium ellipsis">{u.novelTitle}</div>
                <div class="body-small muted ellipsis">
                  {#if u.chapter.bookmark}<Icon name="bookmark" fill size={14} />{/if}
                  {u.chapter.name}{#if !u.chapter.read && u.chapter.progress > 0 && u.chapter.progress < 100} • Progress {u.chapter.progress}%{/if}
                </div>
              </div>
            </a>
            <button class="icon-btn" title={u.chapter.read ? "Mark as unread" : "Mark as read"} onclick={() => markRead(u)}><Icon name={u.chapter.read ? "remove_done" : "done_all"} size={20} /></button>
          </li>
        {/each}
      </ul>
    {/each}
  </div>
</div>

<style>
  .day { margin: 16px 24px 4px; color: var(--on-surface-variant); }
  ul { list-style: none; margin: 0; padding: 0 12px; }
  li { display: flex; align-items: center; border-radius: var(--radius-m); padding-right: 8px; }
  li:hover { background: color-mix(in srgb, var(--on-surface) 6%, transparent); }
  li.read .text { opacity: 0.45; }
  .row { flex: 1; min-width: 0; display: flex; align-items: center; gap: 16px; padding: 8px 12px; }
  .thumb { width: 40px; height: 56px; flex: none; border-radius: 6px; overflow: hidden; background: var(--surface-container-high); }
  .thumb img { width: 100%; height: 100%; object-fit: cover; }
  .text { min-width: 0; }
  .ellipsis { overflow: hidden; white-space: nowrap; text-overflow: ellipsis; }
  .text :global(.icon) { vertical-align: -2px; color: var(--primary); }
</style>
