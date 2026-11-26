<script lang="ts">
  // Novel details. Two panes on a wide window (info | chapters), stacked when narrow.
  // Chapters default to chapter-1-first, like the Android app does for novels.
  import { page } from "$app/state";
  import { goto } from "$app/navigation";
  import { api, errorText, readerHref, type Category, type Chapter, type NovelView } from "$lib/api";
  import { coverSrc } from "$lib/manga";
  import Dialog from "$lib/components/Dialog.svelte";
  import Icon from "$lib/components/Icon.svelte";
  import { toast } from "$lib/components/Toasts.svelte";
  import { formatDate } from "$lib/format";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { untrack } from "svelte";
  import { downloads } from "$lib/downloads.svelte";

  const id = $derived(Number(page.params.id));

  let view = $state<NovelView | null>(null);
  let error = $state<string | null>(null);
  let refreshing = $state(false);
  let descExpanded = $state(false);
  let coverFailed = $state(false);

  let sortDesc = $state(false);
  let filter = $state<"all" | "unread" | "bookmarked">("all");
  let selected = $state<Set<number>>(new Set());
  let lastClicked: number | null = null;

  let categories = $state<Category[]>([]);
  let catDialog = $state(false);
  let catChoice = $state<Set<number>>(new Set());
  let showDlMenu = $state(false);

  /** Chapters with an offline copy; kept current by download events. */
  let downloaded = $state<Set<number>>(new Set());
  $effect(() => {
    downloaded = new Set(view?.downloaded ?? []);
  });
  $effect(() => {
    const d = downloads.lastDone;
    if (d && view && d.novelId === view.novel.id) untrack(() => (downloaded = new Set(downloaded).add(d.chapterId)));
  });

  async function download(ids: number[]) {
    const todo = ids.filter((i) => !downloaded.has(i));
    if (!todo.length) return toast("Already downloaded");
    await api.downloadsEnqueue(todo);
    selected = new Set();
    showDlMenu = false;
    toast(`Downloading ${todo.length} chapter${todo.length === 1 ? "" : "s"}`);
  }

  async function deleteDownloads(ids: number[]) {
    if (!view) return;
    const have = ids.filter((i) => downloaded.has(i));
    await api.downloadsDelete(view.novel.id, have);
    const next = new Set(downloaded);
    for (const i of have) next.delete(i);
    downloaded = next;
    selected = new Set();
  }

  function downloadNext(n: number | "unread" | "all") {
    if (!view) return;
    const list = n === "all" ? view.chapters : view.chapters.filter((c) => !c.read);
    const pick = typeof n === "number" ? list.filter((c) => !downloaded.has(c.id)).slice(0, n) : list;
    download(pick.map((c) => c.id));
  }

  $effect(() => {
    const novelId = id;
    untrack(() => load(novelId));
  });

  async function load(novelId: number) {
    error = null;
    view = null;
    selected = new Set();
    try {
      view = await api.novelGet(novelId);
      if (!view.novel.initialized || view.chapters.length === 0) await refresh();
    } catch (e) {
      error = errorText(e);
    }
  }

  async function refresh() {
    if (!view) return;
    refreshing = true;
    try {
      view = await api.novelRefresh(view.novel.id);
    } catch (e) {
      toast(`Refresh failed: ${errorText(e)}`, undefined, 8000);
    } finally {
      refreshing = false;
    }
  }

  async function toggleLibrary() {
    if (!view) return;
    const n = view.novel;
    if (n.favorite) {
      await api.novelSetFavorite(n.id, false);
      n.favorite = false;
      toast("Removed from library");
      return;
    }
    categories = await api.categoriesList();
    if (categories.length) {
      catChoice = new Set(view.categoryIds);
      catDialog = true;
    } else {
      await addToLibrary([]);
    }
  }

  async function addToLibrary(categoryIds: number[]) {
    if (!view) return;
    await api.novelSetFavorite(view.novel.id, true, categoryIds);
    view.novel.favorite = true;
    view.categoryIds = categoryIds;
    catDialog = false;
    toast("Added to library");
  }

  const shown = $derived.by(() => {
    if (!view) return [] as Chapter[];
    let list = view.chapters;
    if (filter === "unread") list = list.filter((c) => !c.read);
    if (filter === "bookmarked") list = list.filter((c) => c.bookmark);
    return sortDesc ? [...list].reverse() : list;
  });

  /** Next chapter to read: the first unread in reading order (resumes a half-read one). */
  const nextChapter = $derived(view?.chapters.find((c) => !c.read) ?? null);
  const anyRead = $derived(view?.chapters.some((c) => c.read || c.lastParagraph > 0) ?? false);

  function onRowClick(e: MouseEvent, c: Chapter) {
    if (e.ctrlKey || e.metaKey || selected.size > 0) {
      e.preventDefault();
      const next = new Set(selected);
      if (e.shiftKey && lastClicked !== null) {
        const ids = shown.map((x) => x.id);
        const [a, b] = [ids.indexOf(lastClicked), ids.indexOf(c.id)].sort((x, y) => x - y);
        for (const cid of ids.slice(a, b + 1)) next.add(cid);
      } else if (next.has(c.id)) next.delete(c.id);
      else next.add(c.id);
      selected = next;
      lastClicked = c.id;
    }
  }

  async function setRead(ids: number[], read: boolean) {
    if (!view) return;
    await api.chaptersSetRead(ids, read);
    for (const c of view.chapters) if (ids.includes(c.id)) { c.read = read; c.progress = read ? 100 : 0; if (!read) c.lastParagraph = 0; }
    selected = new Set();
  }

  async function setBookmark(ids: number[], bookmark: boolean) {
    if (!view) return;
    await api.chaptersSetBookmark(ids, bookmark);
    for (const c of view.chapters) if (ids.includes(c.id)) c.bookmark = bookmark;
    selected = new Set();
  }

  async function markPreviousRead(c: Chapter) {
    if (!view) return;
    const ids = view.chapters.filter((x) => x.sourceOrder < c.sourceOrder && !x.read).map((x) => x.id);
    if (ids.length) await setRead(ids, true);
  }

  const selectedList = $derived(view?.chapters.filter((c) => selected.has(c.id)) ?? []);

  function onKey(e: KeyboardEvent) {
    if (e.key === "Escape" && selected.size) selected = new Set();
    if ((e.ctrlKey || e.metaKey) && e.key === "a" && (e.target as HTMLElement).tagName !== "INPUT") {
      e.preventDefault();
      selected = new Set(shown.map((c) => c.id));
    }
  }
</script>

<svelte:window onkeydown={onKey} />

<div class="screen">
  <header class="top-bar">
    <button class="icon-btn" title="Back" onclick={() => history.back()}><Icon name="arrow_back" /></button>
    {#if selected.size}
      <h1>{selected.size} selected</h1>
      {#if view?.novel.kind !== "manga"}<button class="icon-btn" title="Download" onclick={() => download([...selected])}><Icon name="download" /></button>{/if}
      {#if selectedList.some((c) => downloaded.has(c.id))}
        <button class="icon-btn" title="Delete downloads" onclick={() => deleteDownloads([...selected])}><Icon name="delete" /></button>
      {/if}
      <button class="icon-btn" title="Bookmark" onclick={() => setBookmark([...selected], !selectedList.every((c) => c.bookmark))}><Icon name="bookmark" /></button>
      <button class="icon-btn" title="Mark as read" onclick={() => setRead([...selected], true)}><Icon name="done_all" /></button>
      <button class="icon-btn" title="Mark as unread" onclick={() => setRead([...selected], false)}><Icon name="remove_done" /></button>
      <button class="icon-btn" title="Clear selection" onclick={() => (selected = new Set())}><Icon name="close" /></button>
    {:else}
      <span style="flex: 1"></span>
      {#if view && view.novel.kind !== "manga"}
        <div class="menu-wrap">
          <button class="icon-btn" title="Download" onclick={() => (showDlMenu = !showDlMenu)}><Icon name="download" /></button>
          {#if showDlMenu}
            <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
            <div class="menu-scrim" onclick={() => (showDlMenu = false)}></div>
            <div class="menu">
              <button onclick={() => downloadNext(1)}>Next chapter</button>
              <button onclick={() => downloadNext(5)}>Next 5 chapters</button>
              <button onclick={() => downloadNext(10)}>Next 10 chapters</button>
              <button onclick={() => downloadNext(25)}>Next 25 chapters</button>
              <button onclick={() => downloadNext("unread")}>Unread</button>
              <button onclick={() => downloadNext("all")}>All</button>
            </div>
          {/if}
        </div>
      {/if}
      <button class="icon-btn" title="Refresh" disabled={refreshing || !view} onclick={refresh}><Icon name="refresh" class={refreshing ? "spin" : ""} /></button>
      {#if view}
        <button class="icon-btn" title="Open in browser" onclick={() => view && openUrl(view.novel.url)}><Icon name="open_in_new" /></button>
      {/if}
    {/if}
  </header>

  {#if error}
    <div class="empty"><div class="face">(╥﹏╥)</div><div>{error}</div><button class="btn tonal" onclick={() => load(id)}>Retry</button></div>
  {:else if !view}
    <div class="empty"><div class="spinner"></div></div>
  {:else}
    {@const n = view.novel}
    <div class="panes">
      <section class="info scroll">
        <div class="head">
          <div class="cover">
            {#if n.cover && !coverFailed}
              <img src={coverSrc(n.kind, n.sourceId, n.cover)} alt="" referrerpolicy="no-referrer" onerror={() => (coverFailed = true)} />
            {:else}<div class="ph"><Icon name="menu_book" size={48} /></div>{/if}
          </div>
          <div class="meta">
            <h2 class="title-large">{n.title}</h2>
            {#if n.author}<div class="body-medium"><Icon name="edit" size={16} /> {n.author}</div>{/if}
            <div class="body-medium muted">
              {#if n.status}<Icon name="schedule" size={16} /> {n.status} · {/if}{view.sourceName ?? "Source not installed"}
            </div>
          </div>
        </div>

        <div class="actions">
          <button class="action" class:on={n.favorite} onclick={toggleLibrary}>
            <Icon name="favorite" fill={n.favorite} />
            <span class="label-medium">{n.favorite ? "In library" : "Add to library"}</span>
          </button>
          <button class="action" onclick={() => openUrl(n.url)}>
            <Icon name="public" /><span class="label-medium">WebView</span>
          </button>
        </div>

        {#if n.description}
          <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
          <div class="desc body-medium" class:expanded={descExpanded} onclick={() => (descExpanded = !descExpanded)}>
            {n.description}
          </div>
          <button class="more-btn" onclick={() => (descExpanded = !descExpanded)} title={descExpanded ? "Show less" : "Show more"}>
            <Icon name={descExpanded ? "keyboard_arrow_up" : "keyboard_arrow_down"} />
          </button>
        {/if}
        {#if n.genres.length}
          <div class="genres">{#each n.genres as g}<span class="chip">{g}</span>{/each}</div>
        {/if}
      </section>

      <section class="chapters">
        <div class="ch-head">
          <span class="title-medium">{view.chapters.length} chapters</span>
          <span class="spacer"></span>
          <button class="chip" class:selected={filter !== "all"} onclick={() => (filter = filter === "all" ? "unread" : filter === "unread" ? "bookmarked" : "all")}>
            <Icon name="filter_list" size={18} />{filter === "all" ? "All" : filter === "unread" ? "Unread" : "Bookmarked"}
          </button>
          <button class="icon-btn" title={sortDesc ? "Newest first" : "Oldest first"} onclick={() => (sortDesc = !sortDesc)}>
            <Icon name={sortDesc ? "arrow_downward" : "arrow_upward"} />
          </button>
        </div>
        {#if nextChapter}
          <a class="btn resume" href={readerHref(n.kind, nextChapter.id)}>
            <Icon name="play_arrow" fill />{anyRead ? "Continue" : "Start"} · {nextChapter.name}
          </a>
        {/if}
        <div class="scroll">
          {#if refreshing && view.chapters.length === 0}
            <div class="empty"><div class="spinner"></div></div>
          {:else if shown.length === 0}
            <div class="empty"><div>No chapters</div></div>
          {/if}
          <ul>
            {#each shown as c (c.id)}
              {@const dl = downloads.status[c.id]}
              <li class:read={c.read} class:sel={selected.has(c.id)}>
                <a class="row" href={readerHref(view.novel.kind, c.id)} onclick={(e) => onRowClick(e, c)}>
                  <div class="text">
                    <div class="body-medium name">
                      {#if c.bookmark}<Icon name="bookmark" fill size={16} class="bm" />{/if}
                      {#if c.locked}<Icon name="lock" size={16} />{/if}
                      {c.name}
                    </div>
                    <div class="body-small muted">
                      {formatDate(c.dateUpload)}
                      {#if !c.read && c.progress > 0 && c.progress < 100}{c.dateUpload ? " • " : ""}Progress {c.progress}%{/if}
                    </div>
                  </div>
                </a>
                {#if downloaded.has(c.id)}
                  <button class="icon-btn dl done" title="Downloaded (click to delete)" onclick={() => deleteDownloads([c.id])}><Icon name="check_circle" fill size={20} /></button>
                {:else if dl === "downloading"}
                  <span class="icon-btn dl"><span class="mini-spin"></span></span>
                {:else if dl === "queued"}
                  <button class="icon-btn dl" title="Queued (click to cancel)" onclick={() => api.downloadsCancel([c.id])}><Icon name="schedule" size={20} /></button>
                {:else if dl === "failed"}
                  <button class="icon-btn dl failed" title={`Failed: ${downloads.errors[c.id] ?? ""} (click to retry)`} onclick={() => download([c.id])}><Icon name="error" size={20} /></button>
                {/if}
                <div class="row-actions">
                  <button class="icon-btn" title={c.bookmark ? "Remove bookmark" : "Bookmark"} onclick={() => setBookmark([c.id], !c.bookmark)}><Icon name="bookmark" fill={c.bookmark} size={20} /></button>
                  <button class="icon-btn" title={c.read ? "Mark as unread" : "Mark as read"} onclick={() => setRead([c.id], !c.read)}><Icon name={c.read ? "remove_done" : "done_all"} size={20} /></button>
                  <button class="icon-btn" title="Mark previous as read" onclick={() => markPreviousRead(c)}><Icon name="vertical_align_top" size={20} /></button>
                  {#if !downloaded.has(c.id) && !dl && view.novel.kind !== "manga"}
                    <button class="icon-btn" title="Download" onclick={() => download([c.id])}><Icon name="download" size={20} /></button>
                  {/if}
                </div>
              </li>
            {/each}
          </ul>
        </div>
        <p class="hint body-small muted">Ctrl+click to select · Shift+click for a range · Ctrl+A selects all</p>
      </section>
    </div>
  {/if}
</div>

<Dialog bind:open={catDialog} title="Set categories">
  {#each categories as c (c.id)}
    <label class="cat">
      <input type="checkbox" checked={catChoice.has(c.id)} onchange={(e) => {
        const next = new Set(catChoice);
        if ((e.target as HTMLInputElement).checked) next.add(c.id); else next.delete(c.id);
        catChoice = next;
      }} />
      {c.name}
    </label>
  {/each}
  {#snippet actions()}
    <button class="btn text" onclick={() => goto("/more/categories")}>Edit</button>
    <button class="btn text" onclick={() => (catDialog = false)}>Cancel</button>
    <button class="btn" onclick={() => addToLibrary([...catChoice])}>OK</button>
  {/snippet}
</Dialog>

<style>
  .panes {
    flex: 1;
    min-height: 0;
    display: grid;
    grid-template-columns: minmax(320px, 420px) 1fr;
    gap: 8px;
    padding: 0 16px 0 24px;
  }
  @media (max-width: 900px) {
    .panes { grid-template-columns: 1fr; overflow-y: auto; }
    .panes .scroll { overflow: visible; }
  }
  .info {
    padding-bottom: 24px;
  }
  .head {
    display: flex;
    gap: 16px;
  }
  .cover {
    width: 120px;
    aspect-ratio: 2 / 3;
    flex: none;
    border-radius: var(--radius-s);
    overflow: hidden;
    background: var(--surface-container-high);
  }
  .cover img { width: 100%; height: 100%; object-fit: cover; }
  .ph { display: grid; place-items: center; height: 100%; color: var(--on-surface-variant); }
  .meta { display: flex; flex-direction: column; gap: 6px; min-width: 0; }
  .meta h2 { margin: 0; word-break: break-word; }
  .meta :global(.icon) { vertical-align: -3px; }
  .actions {
    display: flex;
    margin: 16px 0 8px;
  }
  .action {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
    padding: 8px;
    border: none;
    border-radius: var(--radius-m);
    background: none;
    color: var(--on-surface-variant);
  }
  .action:hover { background: color-mix(in srgb, var(--on-surface) 6%, transparent); }
  .action.on { color: var(--primary); }
  .desc {
    white-space: pre-line;
    display: -webkit-box;
    -webkit-line-clamp: 4;
    line-clamp: 4;
    -webkit-box-orient: vertical;
    overflow: hidden;
    cursor: pointer;
    color: var(--on-surface-variant);
  }
  .desc.expanded { display: block; }
  .more-btn {
    display: flex;
    justify-content: center;
    width: 100%;
    border: none;
    background: none;
    color: var(--on-surface-variant);
  }
  .genres { display: flex; flex-wrap: wrap; gap: 8px; margin-top: 8px; }

  .chapters {
    display: flex;
    flex-direction: column;
    min-height: 0;
    border-radius: var(--radius-l) var(--radius-l) 0 0;
    background: var(--surface-container-low);
  }
  .ch-head {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 12px 12px 8px 20px;
  }
  .spacer { flex: 1; }
  .resume {
    margin: 0 16px 8px;
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
    justify-content: flex-start;
  }
  ul { list-style: none; margin: 0; padding: 0 8px 8px; }
  li {
    position: relative;
    display: flex;
    align-items: center;
    border-radius: var(--radius-m);
  }
  li:hover { background: color-mix(in srgb, var(--on-surface) 6%, transparent); }
  li.sel { background: var(--secondary-container); }
  li.read .text { opacity: 0.45; }
  .row {
    flex: 1;
    min-width: 0;
    display: block;
    padding: 12px 12px;
  }
  .name {
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }
  .name :global(.icon) { vertical-align: -3px; margin-right: 2px; }
  .name :global(.bm) { color: var(--primary); }
  .row-actions {
    display: none;
    padding-right: 4px;
  }
  li:hover .row-actions { display: flex; }
  .hint { margin: 0; padding: 6px 20px; }
  .cat { display: flex; align-items: center; gap: 12px; padding: 8px 0; color: var(--on-surface); }
  :global(.spin) { animation: spin 0.9s linear infinite; }
  .dl { flex: none; }
  .dl.done { color: var(--tertiary); }
  .dl.failed { color: var(--error); }
  .mini-spin {
    width: 18px;
    height: 18px;
    border: 2px solid var(--secondary-container);
    border-top-color: var(--primary);
    border-radius: 50%;
    animation: spin 0.9s linear infinite;
  }
  .menu-wrap { position: relative; }
  .menu-scrim { position: fixed; inset: 0; z-index: 30; }
  .menu {
    position: absolute;
    right: 0;
    top: 44px;
    z-index: 31;
    min-width: 200px;
    padding: 8px 0;
    border-radius: var(--radius-s);
    background: var(--surface-container);
    box-shadow: 0 4px 16px rgb(0 0 0 / 0.3);
  }
  .menu button {
    display: block;
    width: 100%;
    padding: 12px 16px;
    border: none;
    background: none;
    text-align: left;
  }
  .menu button:hover { background: color-mix(in srgb, var(--on-surface) 8%, transparent); }
</style>
