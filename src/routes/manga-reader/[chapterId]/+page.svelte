<script lang="ts">
  // Manga reader. Separate from the novel reader (as on Android). Modes: webtoon (continuous
  // vertical scroll), paged left-to-right, paged right-to-left. Progress is the page index,
  // stored in the same chapter fields the novel reader uses.
  import { page } from "$app/state";
  import { goto } from "$app/navigation";
  import { api, errorText, type Chapter } from "$lib/api";
  import Icon from "$lib/components/Icon.svelte";
  import ChapterListPanel from "$lib/reader/ChapterListPanel.svelte";
  import { mangaApi, mangaPageImage, type MangaPageRef } from "$lib/manga";
  import { progressPercent } from "$lib/reader/paragraphs";
  import { settings } from "$lib/settings.svelte";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { onMount, tick, untrack } from "svelte";

  const PRELOAD_AHEAD = 4;

  const chapterId = $derived(Number(page.params.chapterId));
  const s = $derived(settings.values);
  const paged = $derived(s.mangaReaderMode !== "webtoon");
  const rtl = $derived(s.mangaReaderMode === "paged-rtl");

  let chapter = $state<Chapter | null>(null);
  let chapters = $state<Chapter[]>([]);
  let title = $state("");
  let sourceId = $state("");
  let pages = $state<MangaPageRef[]>([]);
  let loading = $state(true);
  let error = $state<string | null>(null);
  let failed = $state<Record<number, number>>({}); // page index → retry counter

  let current = $state(0);
  let atEnd = $state(false);
  let zoom = $state(1);
  let showControls = $state(false);
  let showChapterList = $state(false);
  let showSettings = $state(false);
  let scroller: HTMLElement | undefined = $state();

  const index = $derived(chapters.findIndex((c) => c.id === chapterId));
  const prev = $derived(index > 0 ? chapters[index - 1] : null);
  const next = $derived(index >= 0 && index < chapters.length - 1 ? chapters[index + 1] : null);
  const percent = $derived(progressPercent(current, pages.length, atEnd));

  let loadGen = 0;
  $effect(() => {
    const id = chapterId;
    untrack(() => load(id));
  });

  async function load(id: number) {
    const gen = ++loadGen;
    loading = true;
    error = null;
    failed = {};
    showChapterList = false;
    try {
      const ch = await api.chapterGet(id);
      if (gen !== loadGen) return;
      chapter = ch;
      if (!chapters.length || chapters[0].novelId !== ch.novelId) {
        const view = await api.novelGet(ch.novelId);
        if (gen !== loadGen) return;
        chapters = view.chapters;
        title = view.novel.title;
        sourceId = view.novel.sourceId;
      }
      const list = await mangaApi.pages(id);
      if (gen !== loadGen) return;
      pages = list;
      loading = false;
      const start = ch.read ? 0 : Math.min(ch.lastParagraph, Math.max(0, list.length - 1));
      current = start;
      atEnd = false;
      await tick();
      if (!paged) scrollToPage(start, "instant");
    } catch (e) {
      if (gen === loadGen) {
        error = errorText(e);
        loading = false;
      }
    }
  }

  // ---- progress ---------------------------------------------------------------------------
  let saveTimer: ReturnType<typeof setTimeout> | undefined;
  function scheduleSave() {
    clearTimeout(saveTimer);
    saveTimer = setTimeout(saveNow, 500);
  }
  function saveNow() {
    clearTimeout(saveTimer);
    if (!chapter || loading || error || !pages.length) return;
    const markRead = atEnd || chapter.read;
    const p = percent;
    if (chapter.lastParagraph === current && chapter.read === markRead && chapter.progress === p) return;
    chapter.lastParagraph = current;
    chapter.read = markRead;
    chapter.progress = p;
    const inList = chapters.find((c) => c.id === chapter!.id);
    if (inList) Object.assign(inList, { lastParagraph: current, read: markRead, progress: p });
    api.chapterSaveProgress(chapter.id, current, p, markRead).catch(console.error);
  }

  // Webtoon: the current page is the one crossing the upper third of the viewport.
  let ticking = false;
  function onScroll() {
    if (ticking || !scroller || paged) return;
    ticking = true;
    requestAnimationFrame(() => {
      ticking = false;
      if (!scroller) return;
      const probe = scroller.scrollTop + scroller.clientHeight / 3;
      const nodes = scroller.querySelectorAll<HTMLElement>("[data-p]");
      let found = 0;
      for (let i = 0; i < nodes.length; i++) {
        if (nodes[i].offsetTop <= probe) found = i;
        else break;
      }
      current = found;
      atEnd = scroller.scrollTop + scroller.clientHeight >= scroller.scrollHeight - 8;
      scheduleSave();
    });
  }

  function scrollToPage(i: number, behavior: ScrollBehavior = "smooth") {
    scroller?.querySelector<HTMLElement>(`[data-p="${i}"]`)?.scrollIntoView({ block: "start", behavior });
  }

  // Paged: move by one page; past either end goes to the adjacent chapter.
  function step(delta: number) {
    if (!paged) {
      scroller?.scrollBy({ top: delta * scroller.clientHeight * 0.85, behavior: "smooth" });
      return;
    }
    const target = current + delta;
    if (target < 0) return goChapter(prev);
    if (target >= pages.length) return finishAndNext();
    current = target;
    atEnd = current >= pages.length - 1;
    scheduleSave();
  }

  function seek(e: Event) {
    showControls = true;
    const i = Number((e.target as HTMLInputElement).value);
    if (paged) {
      current = i;
      atEnd = current >= pages.length - 1;
      scheduleSave();
    } else scrollToPage(i, "instant");
  }

  // ---- chapters ---------------------------------------------------------------------------
  function goChapter(c: Chapter | null) {
    if (!c) return;
    saveNow();
    goto(`/manga-reader/${c.id}`, { replaceState: true });
  }

  async function finishAndNext() {
    if (chapter && !chapter.read) {
      chapter.read = true;
      await api.chapterSaveProgress(chapter.id, Math.max(0, pages.length - 1), 100, true);
    }
    goChapter(next);
  }

  function back() {
    saveNow();
    if (chapter) goto(`/novel/${chapter.novelId}`);
    else history.back();
  }

  async function toggleBookmark() {
    if (!chapter) return;
    const b = !chapter.bookmark;
    await api.chaptersSetBookmark([chapter.id], b);
    chapter.bookmark = b;
    const inList = chapters.find((c) => c.id === chapter!.id);
    if (inList) inList.bookmark = b;
  }

  // ---- input ------------------------------------------------------------------------------
  function onKey(e: KeyboardEvent) {
    const tag = (e.target as HTMLElement).tagName;
    if (tag === "INPUT" && (e.target as HTMLInputElement).type !== "range") return;
    const fwd = rtl ? -1 : 1;
    switch (e.key) {
      case "ArrowRight": paged ? step(fwd) : goChapter(next); break;
      case "ArrowLeft": paged ? step(-fwd) : goChapter(prev); break;
      case "ArrowDown": if (paged) step(1); else return; break;
      case "ArrowUp": if (paged) step(-1); else return; break;
      case "PageDown": case " ": step(e.shiftKey ? -1 : 1); break;
      case "PageUp": step(-1); break;
      case "Home": paged ? (current = 0) : scroller?.scrollTo({ top: 0 }); break;
      case "End": paged ? (current = pages.length - 1) : scroller?.scrollTo({ top: scroller.scrollHeight }); break;
      case "]": goChapter(next); break;
      case "[": goChapter(prev); break;
      case "c": case "C": showChapterList = !showChapterList; break;
      case "s": case "S": showSettings = !showSettings; break;
      case "b": case "B": toggleBookmark(); break;
      case "+": case "=": zoom = Math.min(3, +(zoom + 0.1).toFixed(2)); break;
      case "-": zoom = Math.max(0.3, +(zoom - 0.1).toFixed(2)); break;
      case "0": zoom = 1; break;
      case "F11": settings.set("fullscreen", !s.fullscreen); break;
      case "Escape":
        if (showChapterList) showChapterList = false;
        else if (showSettings) showSettings = false;
        else if (s.fullscreen) settings.set("fullscreen", false);
        else back();
        break;
      default: return;
    }
    e.preventDefault();
  }

  function onWheel(e: WheelEvent) {
    if (e.ctrlKey) {
      e.preventDefault();
      zoom = Math.min(3, Math.max(0.3, +(zoom - Math.sign(e.deltaY) * 0.1).toFixed(2)));
    } else if (paged) {
      e.preventDefault();
      step(e.deltaY > 0 ? 1 : -1);
    }
  }

  /** Paged: tap left/right thirds to turn pages, middle to toggle the chrome. */
  function onPageClick(e: MouseEvent) {
    if ((e.target as HTMLElement).closest("button, a, input")) return;
    if (!paged) return void (showControls = !showControls);
    const x = e.clientX / window.innerWidth;
    if (x < 0.33) step(rtl ? 1 : -1);
    else if (x > 0.67) step(rtl ? -1 : 1);
    else showControls = !showControls;
  }

  function retry(i: number) {
    failed[i] = (failed[i] ?? 0) + 1;
  }
  const src = (p: MangaPageRef, i: number) => {
    const base = mangaPageImage(sourceId, p);
    return failed[i] ? `${base}&r=${failed[i]}` : base;
  };

  // Paged preloading: warm the next few pages.
  $effect(() => {
    if (!paged || !pages.length) return;
    for (let i = current + 1; i <= Math.min(pages.length - 1, current + PRELOAD_AHEAD); i++) {
      const img = new Image();
      img.src = src(pages[i], i);
    }
  });

  $effect(() => {
    getCurrentWindow().setFullscreen(s.fullscreen).catch(() => {});
  });
  onMount(() => () => {
    saveNow();
    if (settings.values.fullscreen) getCurrentWindow().setFullscreen(false).catch(() => {});
  });

  const chromeVisible = $derived(showControls && !showChapterList);
  const fitClass = $derived(`fit-${s.mangaFit}`);
</script>

<svelte:window onkeydown={onKey} />

<div class="reader">
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div class="stage {fitClass}" class:paged bind:this={scroller} onscroll={onScroll} onwheel={onWheel} onclick={onPageClick}>
    {#if loading}
      <div class="center"><div class="spinner"></div></div>
    {:else if error}
      <div class="center err">
        <div class="face">(╥﹏╥)</div>
        <div class="msg">{error}</div>
        <button class="btn tonal" onclick={() => load(chapterId)}><Icon name="refresh" size={18} />Retry</button>
      </div>
    {:else if paged}
      {@const p = pages[current]}
      {#if p}
        <div class="page-wrap" style:transform="scale({zoom})">
          {#key `${current}-${failed[current] ?? 0}`}
            <img class="page" src={src(p, current)} alt="Page {current + 1}" draggable="false" onerror={() => retry(current)} />
          {/key}
        </div>
      {/if}
    {:else}
      <div class="strip" style:max-width="{s.mangaFit === 'width' ? s.mangaMaxWidth * zoom : 99999}px" style:gap={s.mangaPageGap ? "12px" : "0"}>
        {#each pages as p, i (i)}
          <div class="slot" data-p={i}>
            <img
              class="page"
              src={src(p, i)}
              alt="Page {i + 1}"
              loading={Math.abs(i - current) <= PRELOAD_AHEAD ? "eager" : "lazy"}
              draggable="false"
              style:zoom={s.mangaFit === "width" ? 1 : zoom}
              onerror={() => (failed[i] ?? 0) < 2 && retry(i)}
            />
          </div>
        {/each}
        <div class="end">
          <div class="body-medium">Finished: {chapter?.name}</div>
          {#if next}
            <button class="btn" onclick={finishAndNext}>Next: {next.name}<Icon name="chevron_right" size={18} /></button>
          {:else}<div class="body-small muted">No more chapters</div>{/if}
        </div>
      </div>
    {/if}
  </div>

  {#if !loading && !error && pages.length}
    <div class="counter label-medium">{current + 1} / {pages.length}</div>
  {/if}

  {#if chromeVisible}
    <header class="chrome top">
      <button class="icon-btn" title="Back (Esc)" onclick={back}><Icon name="arrow_back" /></button>
      <div class="titles">
        <div class="title-medium ellipsis">{chapter?.name ?? ""}</div>
        <div class="body-small muted ellipsis">{title}</div>
      </div>
      {#if chapter}<button class="icon-btn" title="Open in browser" onclick={() => chapter && openUrl(chapter.url)}><Icon name="public" /></button>{/if}
      <button class="icon-btn" title="Fullscreen (F11)" onclick={() => settings.set("fullscreen", !s.fullscreen)}><Icon name={s.fullscreen ? "fullscreen_exit" : "fullscreen"} /></button>
      <button class="icon-btn" title="Settings (S)" onclick={() => (showSettings = !showSettings)}><Icon name="tune" /></button>
    </header>

    <nav class="chrome bottom">
      <button class="icon-btn" title="Previous chapter ([)" disabled={!prev} onclick={() => goChapter(prev)}><Icon name="skip_previous" /></button>
      {#if pages.length > 1}
        <input class="slider" class:rtl={rtl} type="range" min="0" max={pages.length - 1} value={current} oninput={seek} aria-label="Page" />
      {/if}
      <button class="icon-btn" title="Chapters (C)" onclick={() => (showChapterList = true)}><Icon name="format_list_bulleted" /></button>
      <button class="icon-btn" class:active={chapter?.bookmark} title="Bookmark (B)" onclick={toggleBookmark}><Icon name="bookmark" fill={chapter?.bookmark} /></button>
      <button class="icon-btn" title="Next chapter (])" disabled={!next} onclick={() => goChapter(next)}><Icon name="skip_next" /></button>
    </nav>
  {/if}

  {#if showSettings}
    <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
    <div class="sheet-scrim" onclick={() => (showSettings = false)}></div>
    <aside class="sheet">
      <div class="label-large">Reading mode</div>
      <div class="seg">
        {#each [["webtoon", "Webtoon"], ["paged-ltr", "Paged (L→R)"], ["paged-rtl", "Paged (R→L)"]] as [m, label]}
          <button class:on={s.mangaReaderMode === m} onclick={() => settings.set("mangaReaderMode", m as typeof s.mangaReaderMode)}>{label}</button>
        {/each}
      </div>
      <div class="label-large">Fit</div>
      <div class="seg">
        {#each [["width", "Width"], ["height", "Height"], ["original", "Original"]] as [f, label]}
          <button class:on={s.mangaFit === f} onclick={() => settings.set("mangaFit", f as typeof s.mangaFit)}>{label}</button>
        {/each}
      </div>
      {#if !paged && s.mangaFit === "width"}
        <div class="row-s"><span>Strip width</span>
          <span class="stepper">
            <button class="icon-btn" onclick={() => settings.set("mangaMaxWidth", Math.max(400, s.mangaMaxWidth - 100))}>−</button>
            <span>{s.mangaMaxWidth}</span>
            <button class="icon-btn" onclick={() => settings.set("mangaMaxWidth", Math.min(2400, s.mangaMaxWidth + 100))}>+</button>
          </span>
        </div>
        <label class="row-s"><span>Gap between pages</span><input type="checkbox" checked={s.mangaPageGap} onchange={(e) => settings.set("mangaPageGap", (e.target as HTMLInputElement).checked)} /></label>
      {/if}
      <div class="row-s"><span>Zoom</span><span class="stepper"><button class="icon-btn" onclick={() => (zoom = Math.max(0.3, +(zoom - 0.1).toFixed(2)))}>−</button><span>{Math.round(zoom * 100)}%</span><button class="icon-btn" onclick={() => (zoom = Math.min(3, +(zoom + 0.1).toFixed(2)))}>+</button></span></div>
      <p class="body-small muted keys">←/→ page (paged) · [ ] chapter · Space next · Ctrl+wheel or +/- zoom · 0 reset · C chapters · F11 fullscreen</p>
    </aside>
  {/if}

  {#if showChapterList && chapter}
    <ChapterListPanel {chapters} currentId={chapter.id} onPick={(c) => goChapter(c)} onClose={() => (showChapterList = false)} />
  {/if}
</div>

<style>
  .reader { position: relative; height: 100vh; overflow: hidden; background: #111; color: #eee; }
  .stage { height: 100%; overflow: auto; user-select: none; }
  .stage.paged { display: grid; place-items: center; overflow: hidden; }
  .strip { margin: 0 auto; display: flex; flex-direction: column; align-items: center; }
  .slot { width: 100%; min-height: 200px; display: flex; justify-content: center; }
  .page { display: block; max-width: 100%; background: #1a1a1a; }
  .fit-width .strip .page { width: 100%; height: auto; }
  .fit-height .strip .page { height: 100vh; width: auto; max-width: none; }
  .fit-original .strip .page { max-width: none; }
  .page-wrap { transform-origin: center center; transition: transform 120ms; }
  .paged.fit-width .page { width: 100vw; max-height: none; height: auto; }
  .paged.fit-height .page { height: 100vh; width: auto; }
  .paged.fit-original .page { max-width: none; }
  .end { display: flex; flex-direction: column; align-items: center; gap: 14px; padding: 48px 0 64px; }
  .center { display: grid; place-items: center; align-content: center; gap: 12px; height: 100%; text-align: center; padding: 32px; }
  .err .face { font-size: 40px; }
  .err .msg { max-width: 640px; white-space: pre-line; word-break: break-word; }
  .counter { position: absolute; bottom: 10px; left: 50%; transform: translateX(-50%); padding: 3px 10px; border-radius: 10px; background: rgb(0 0 0 / 0.55); color: #fff; pointer-events: none; z-index: 5; }
  .chrome { position: absolute; z-index: 20; color: var(--on-surface); background: color-mix(in srgb, var(--surface-container) 92%, transparent); backdrop-filter: blur(6px); }
  .chrome.top { top: 0; left: 0; right: 0; display: flex; align-items: center; gap: 4px; height: 64px; padding: 0 8px; animation: from-top 200ms var(--ease); }
  .titles { flex: 1; min-width: 0; margin-left: 4px; }
  .ellipsis { overflow: hidden; white-space: nowrap; text-overflow: ellipsis; }
  .chrome.bottom { bottom: 0; left: 0; right: 0; display: flex; align-items: center; gap: 8px; height: 64px; padding: 0 12px; animation: from-bottom 200ms var(--ease); }
  .slider { flex: 1; accent-color: var(--primary); }
  .slider.rtl { direction: rtl; }
  .sheet-scrim { position: absolute; inset: 0; z-index: 40; }
  .sheet { position: absolute; top: 72px; right: 12px; z-index: 41; width: 340px; display: flex; flex-direction: column; gap: 10px; padding: 16px; border-radius: var(--radius-l); background: var(--surface-container-high); color: var(--on-surface); box-shadow: 0 8px 24px rgb(0 0 0 / 0.4); }
  .seg { display: flex; border: 1px solid var(--outline); border-radius: 18px; overflow: hidden; }
  .seg button { flex: 1; height: 34px; border: none; background: none; color: var(--on-surface-variant); font-size: 13px; }
  .seg button + button { border-left: 1px solid var(--outline); }
  .seg button.on { background: var(--secondary-container); color: var(--on-secondary-container); }
  .row-s { display: flex; align-items: center; justify-content: space-between; min-height: 40px; }
  .stepper { display: flex; align-items: center; gap: 4px; }
  .stepper .icon-btn { color: var(--on-surface); font-size: 18px; }
  .keys { margin: 4px 0 0; line-height: 1.6; }
  @keyframes from-top { from { transform: translateY(-100%); opacity: 0; } }
  @keyframes from-bottom { from { transform: translateY(100%); opacity: 0; } }
</style>
