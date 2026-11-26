<script lang="ts">
  // One source: Popular / Latest / Search, with the plugin's filters. Infinite scroll.
  import { page } from "$app/state";
  import { api, errorText, type BrowseItem, type BrowseQuery } from "$lib/api";
  import CoverCard from "$lib/components/CoverCard.svelte";
  import Icon from "$lib/components/Icon.svelte";
  import { sources } from "$lib/sources.svelte";
  import { onMount, untrack } from "svelte";

  const sourceId = $derived(page.params.sourceId ?? "");
  const source = $derived(sources.byId(sourceId));

  type Tab = "popular" | "latest" | "search";
  let tab = $state<Tab>((page.url.searchParams.get("tab") as Tab) || "popular");
  let query = $state("");
  let filterValues = $state<Record<string, string>>({});
  let showFilters = $state(false);

  let items = $state<BrowseItem[]>([]);
  let pageNo = $state(0);
  let hasNext = $state(true);
  let loading = $state(false);
  let error = $state<string | null>(null);
  let generation = 0;

  onMount(() => {
    if (!sources.loaded) sources.refresh();
  });

  function currentQuery(): BrowseQuery {
    if (tab === "latest") return { kind: "latest" };
    if (tab === "search") {
      const filters: Record<string, string> = {};
      for (const [k, v] of Object.entries(filterValues)) if (v) filters[k] = v;
      return { kind: "search", query: query.trim(), filters };
    }
    return { kind: "popular" };
  }

  function reset() {
    generation++;
    items = [];
    pageNo = 0;
    hasNext = true;
    error = null;
    loading = false;
    loadMore();
  }

  async function loadMore() {
    if (loading || !hasNext || !sourceId) return;
    loading = true;
    const gen = generation;
    try {
      const res = await api.sourceBrowse(sourceId, currentQuery(), pageNo + 1);
      if (gen !== generation) return;
      const seen = new Set(items.map((i) => i.url));
      items.push(...res.novels.filter((n) => !seen.has(n.url)));
      pageNo += 1;
      hasNext = res.hasNextPage && res.novels.length > 0;
    } catch (e) {
      if (gen === generation) error = errorText(e);
    } finally {
      if (gen === generation) loading = false;
    }
  }

  // (Re)load when the source or tab changes; search reloads on submit. untrack: reset() reads
  // and writes the paging state, which would otherwise re-trigger this effect forever.
  $effect(() => {
    const id = sourceId;
    const t = tab;
    if (id && t !== "search") untrack(reset);
  });

  function submitSearch(e: Event) {
    e.preventDefault();
    tab = "search";
    reset();
  }

  let sentinel: HTMLElement | undefined = $state();
  $effect(() => {
    if (!sentinel) return;
    const io = new IntersectionObserver((entries) => {
      if (entries[0].isIntersecting) loadMore();
    }, { rootMargin: "600px" });
    io.observe(sentinel);
    return () => io.disconnect();
  });
</script>

<div class="screen">
  <header class="top-bar">
    <a class="icon-btn" href="/browse" title="Back"><Icon name="arrow_back" /></a>
    <h1>{source?.name ?? "Source"}</h1>
    <form class="search" onsubmit={submitSearch}>
      <input class="text-field" placeholder="Search {source?.name ?? ''}" bind:value={query} />
    </form>
    {#if source?.baseUrl}
      <a class="icon-btn" href={source.baseUrl} target="_blank" rel="noreferrer" title="Open site in browser"><Icon name="open_in_new" /></a>
    {/if}
  </header>

  <div class="chips">
    <button class="chip" class:selected={tab === "popular"} onclick={() => (tab = "popular")}><Icon name="favorite" size={18} />Popular</button>
    {#if source?.supportsLatest !== false}
      <button class="chip" class:selected={tab === "latest"} onclick={() => (tab = "latest")}><Icon name="new_label" size={18} />Latest</button>
    {/if}
    {#if source?.filters.length}
      <button class="chip" class:selected={showFilters} onclick={() => (showFilters = !showFilters)}><Icon name="filter_list" size={18} />Filter</button>
    {/if}
    {#if tab === "search"}<span class="chip selected"><Icon name="search" size={18} />{query || "Filtered"}</span>{/if}
  </div>

  {#if showFilters && source}
    <div class="filters">
      {#each source.filters as f (f.id)}
        <label class="filter">
          <span class="label-medium muted">{f.name}</span>
          <select bind:value={filterValues[f.id]}>
            <option value="">Any</option>
            {#each f.options as o}<option value={o.value}>{o.label}</option>{/each}
          </select>
        </label>
      {/each}
      <button class="btn" onclick={() => { tab = "search"; reset(); }}>Apply</button>
      <button class="btn text" onclick={() => (filterValues = {})}>Reset</button>
    </div>
  {/if}

  <div class="scroll">
    {#if items.length}
      <div class="grid">
        {#each items as n (n.url)}
          <CoverCard href="/novel/{n.id}" title={n.title} cover={n.cover} inLibrary={n.favorite} />
        {/each}
      </div>
    {/if}

    {#if error}
      <div class="empty">
        <div class="face">(╥﹏╥)</div>
        <div class="error-text">{error}</div>
        <button class="btn tonal" onclick={() => { error = null; loadMore(); }}><Icon name="refresh" size={18} />Retry</button>
      </div>
    {:else if loading}
      <div class="loading"><div class="spinner"></div></div>
    {:else if !items.length && !hasNext}
      <div class="empty"><div class="face">(・_・;)</div><div>No results found</div></div>
    {/if}
    <div bind:this={sentinel} class="sentinel"></div>
  </div>
</div>

<style>
  .search {
    display: flex;
    margin-left: auto;
  }
  .search .text-field {
    width: min(360px, 40vw);
  }
  .chips {
    display: flex;
    gap: 8px;
    padding: 0 24px 12px;
    flex-wrap: wrap;
    flex: none;
  }
  .filters {
    display: flex;
    flex-wrap: wrap;
    align-items: end;
    gap: 16px;
    margin: 0 24px 12px;
    padding: 16px;
    border-radius: var(--radius-l);
    background: var(--surface-container-low);
    flex: none;
  }
  .filter {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  select {
    height: 40px;
    padding: 0 12px;
    border-radius: var(--radius-s);
    border: 1px solid var(--outline-variant);
    background: var(--surface-container-high);
    min-width: 180px;
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(150px, 1fr));
    gap: 4px;
    padding: 0 20px 20px;
  }
  .loading {
    display: grid;
    place-items: center;
    padding: 32px;
  }
  .error-text {
    max-width: 640px;
    word-break: break-word;
  }
  .sentinel {
    height: 1px;
  }
</style>
