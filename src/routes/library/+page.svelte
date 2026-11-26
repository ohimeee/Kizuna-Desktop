<script lang="ts">
  import { onMount } from "svelte";
  import { api, errorText, type Category, type LibraryItem } from "$lib/api";
  import CoverCard from "$lib/components/CoverCard.svelte";
  import Icon from "$lib/components/Icon.svelte";
  import { toast } from "$lib/components/Toasts.svelte";
  import { settings, type LibrarySort } from "$lib/settings.svelte";

  let items = $state<LibraryItem[]>([]);
  let categories = $state<Category[]>([]);
  let loaded = $state(false);
  let tab = $state<number>(-1); // -1 = all / uncategorized default view
  let query = $state("");
  let showSort = $state(false);
  let updating = $state(false);

  onMount(load);

  async function load() {
    [items, categories] = await Promise.all([api.libraryList(), api.categoriesList()]);
    loaded = true;
  }

  async function updateLibrary() {
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

  const sorts: { id: LibrarySort; label: string }[] = [
    { id: "title", label: "Alphabetically" },
    { id: "lastRead", label: "Last read" },
    { id: "unread", label: "Unread count" },
    { id: "totalChapters", label: "Total chapters" },
    { id: "dateAdded", label: "Date added" },
  ];

  function pickSort(id: LibrarySort) {
    if (settings.values.librarySort === id) settings.set("librarySortDesc", !settings.values.librarySortDesc);
    else settings.set("librarySort", id);
  }

  // Tabs: "Default" (no category) first when used, then each category, like Mihon.
  const hasUncategorized = $derived(items.some((i) => i.categoryIds.length === 0));
  const tabs = $derived([
    ...(hasUncategorized || categories.length === 0 ? [{ id: -1, name: categories.length ? "Default" : "All" }] : []),
    ...categories.map((c) => ({ id: c.id, name: c.name })),
  ]);
  $effect(() => {
    if (tabs.length && !tabs.some((t) => t.id === tab)) tab = tabs[0].id;
  });

  const shown = $derived.by(() => {
    const q = query.trim().toLowerCase();
    let list = items.filter((i) =>
      categories.length === 0 ? true : tab === -1 ? i.categoryIds.length === 0 : i.categoryIds.includes(tab),
    );
    if (q) list = list.filter((i) => i.title.toLowerCase().includes(q) || i.author?.toLowerCase().includes(q));
    if (settings.values.libraryUnreadOnly) list = list.filter((i) => i.unread > 0);
    const key = settings.values.librarySort;
    const cmp: Record<LibrarySort, (a: LibraryItem, b: LibraryItem) => number> = {
      title: (a, b) => a.title.localeCompare(b.title),
      lastRead: (a, b) => b.lastRead - a.lastRead,
      unread: (a, b) => b.unread - a.unread,
      totalChapters: (a, b) => b.total - a.total,
      dateAdded: (a, b) => b.dateAdded - a.dateAdded,
    };
    const sorted = [...list].sort(cmp[key]);
    return settings.values.librarySortDesc ? sorted.reverse() : sorted;
  });

  const countFor = (id: number) =>
    items.filter((i) => (id === -1 ? (categories.length ? i.categoryIds.length === 0 : true) : i.categoryIds.includes(id))).length;
</script>

<div class="screen">
  <header class="top-bar">
    <h1>Library</h1>
    <input class="text-field search" placeholder="Search library" bind:value={query} />
    <button class="icon-btn" class:active={showSort} title="Sort & filter" onclick={() => (showSort = !showSort)}><Icon name="filter_list" /></button>
    <button class="icon-btn" title="Update library" disabled={updating} onclick={updateLibrary}><Icon name="refresh" class={updating ? "spin" : ""} /></button>
  </header>

  {#if tabs.length > 1}
    <div class="tabs">
      {#each tabs as t (t.id)}
        <button class:on={tab === t.id} onclick={() => (tab = t.id)}>{t.name} <span class="count">{countFor(t.id)}</span></button>
      {/each}
    </div>
  {/if}

  {#if showSort}
    <div class="sortbar">
      {#each sorts as so}
        <button class="chip" class:selected={settings.values.librarySort === so.id} onclick={() => pickSort(so.id)}>
          {#if settings.values.librarySort === so.id}<Icon name={settings.values.librarySortDesc ? "arrow_downward" : "arrow_upward"} size={16} />{/if}
          {so.label}
        </button>
      {/each}
      <span class="sep"></span>
      <button class="chip" class:selected={settings.values.libraryUnreadOnly} onclick={() => settings.set("libraryUnreadOnly", !settings.values.libraryUnreadOnly)}>Unread only</button>
      <button class="chip" class:selected={settings.values.showUnreadBadge} onclick={() => settings.set("showUnreadBadge", !settings.values.showUnreadBadge)}>Unread badge</button>
    </div>
  {/if}

  <div class="scroll">
    {#if loaded && items.length === 0}
      <div class="empty">
        <div class="face">(´･ω･`)</div>
        <div class="body-large">Your library is empty</div>
        <a class="btn tonal" href="/browse"><Icon name="explore" size={18} />Browse sources</a>
      </div>
    {:else if loaded && shown.length === 0}
      <div class="empty"><div class="face">(・_・;)</div><div>Nothing here</div></div>
    {:else}
      <div class="grid">
        {#each shown as n (n.id)}
          <CoverCard href="/novel/{n.id}" title={n.title} cover={n.cover} unread={settings.values.showUnreadBadge ? n.unread : 0} />
        {/each}
      </div>
    {/if}
  </div>
</div>

<style>
  .search { width: min(320px, 40vw); }
  .tabs {
    display: flex;
    gap: 4px;
    padding: 0 16px;
    border-bottom: 1px solid var(--outline-variant);
    overflow-x: auto;
    flex: none;
  }
  .tabs button {
    height: 48px;
    padding: 0 16px;
    border: none;
    background: none;
    color: var(--on-surface-variant);
    font-weight: 500;
    border-bottom: 3px solid transparent;
    white-space: nowrap;
  }
  .tabs button.on { color: var(--primary); border-bottom-color: var(--primary); }
  .count { opacity: 0.7; font-size: 12px; }
  .sortbar {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    padding: 12px 24px;
    flex: none;
  }
  .sep { width: 1px; background: var(--outline-variant); margin: 0 4px; }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(150px, 1fr));
    gap: 4px;
    padding: 8px 20px 20px;
  }
</style>
