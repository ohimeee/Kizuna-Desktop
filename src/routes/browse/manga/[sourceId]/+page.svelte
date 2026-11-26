<script lang="ts">
  // One manga source: Popular / Latest / Search. Infinite scroll.
  import { page } from "$app/state";
  import { errorText, type BrowseItem } from "$lib/api";
  import CoverCard from "$lib/components/CoverCard.svelte";
  import Icon from "$lib/components/Icon.svelte";
  import { mangaApi, mangaImage, type CachedSource } from "$lib/manga";
  import { onMount, untrack } from "svelte";

  const sourceId = $derived(page.params.sourceId ?? "");
  let source = $state<CachedSource | null>(null);

  type Tab = "popular" | "latest" | "search";
  let tab = $state<Tab>("popular");
  let query = $state("");
  let items = $state<BrowseItem[]>([]);
  let pageNo = $state(0);
  let hasNext = $state(true);
  let loading = $state(false);
  let error = $state<string | null>(null);
  let generation = 0;

  onMount(async () => {
    source = (await mangaApi.sourcesCached())[sourceId] ?? null;
  });

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
      const res = await mangaApi.browse(sourceId, tab, pageNo + 1, tab === "search" ? query.trim() : undefined);
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

  $effect(() => {
    const id = sourceId;
    const t = tab;
    if (id && t !== "search") untrack(reset);
  });

  function submitSearch(e: Event) {
    e.preventDefault();
    if (!query.trim()) return;
    tab = "search";
    reset();
  }

  let sentinel: HTMLElement | undefined = $state();
  $effect(() => {
    if (!sentinel) return;
    const io = new IntersectionObserver((entries) => {
      if (entries[0].isIntersecting) loadMore();
    }, { rootMargin: "800px" });
    io.observe(sentinel);
    return () => io.disconnect();
  });
</script>

<div class="screen">
  <header class="top-bar">
    <a class="icon-btn" href="/browse?kind=manga" title="Back"><Icon name="arrow_back" /></a>
    <h1>{source?.name ?? "Source"} {#if source}<span class="muted body-medium">{source.lang.toUpperCase()}</span>{/if}</h1>
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
    {#if tab === "search"}<span class="chip selected"><Icon name="search" size={18} />{query}</span>{/if}
  </div>

  <div class="scroll">
    {#if items.length}
      <div class="grid">
        {#each items as n (n.url)}
          <CoverCard href="/novel/{n.id}" title={n.title} cover={mangaImage(sourceId, n.cover)} inLibrary={n.favorite} />
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
      <div class="loading"><div class="spinner"></div>{#if !items.length}<div class="body-small muted">Starting the manga engine…</div>{/if}</div>
    {:else if !items.length && !hasNext}
      <div class="empty"><div class="face">(・_・;)</div><div>No results found</div></div>
    {/if}
    <div bind:this={sentinel} class="sentinel"></div>
  </div>
</div>

<style>
  .search { display: flex; margin-left: auto; }
  .search .text-field { width: min(360px, 40vw); }
  .chips { display: flex; gap: 8px; padding: 0 24px 12px; flex-wrap: wrap; flex: none; }
  .grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(150px, 1fr)); gap: 4px; padding: 0 20px 20px; }
  .loading { display: grid; place-items: center; gap: 8px; padding: 32px; }
  .error-text { max-width: 640px; word-break: break-word; white-space: pre-line; }
  .sentinel { height: 1px; }
</style>
