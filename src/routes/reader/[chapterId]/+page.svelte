<script lang="ts">
  // The novel reader. Behaviour follows the Android reader's regression log
  // (Kizuna/docs/novel-reader-ui-notes.md); desktop-specific choices are noted inline.
  import { page } from "$app/state";
  import { afterNavigate, goto } from "$app/navigation";
  import { api, errorText, type Chapter } from "$lib/api";
  import Icon from "$lib/components/Icon.svelte";
  import ChapterListPanel from "$lib/reader/ChapterListPanel.svelte";
  import ReaderSettingsPanel from "$lib/reader/ReaderSettingsPanel.svelte";
  import { progressPercent, splitNovelParagraphs, withChapterTitle } from "$lib/reader/paragraphs";
  import { paragraphHtml, paragraphText } from "$lib/reader/richtext";
  import { READER_THEMES, settings } from "$lib/settings.svelte";
  import { formatTime } from "$lib/format";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { onMount, tick, untrack } from "svelte";

  const CHAPTER_TITLE_SCALE = 1.7;
  const CHAPTER_TITLE_LINE_SPACING = 1.25;
  const CHAPTER_TITLE_MAX_LENGTH = 120;
  const CHAPTER_TOP_PADDING = 64;
  /** Android scrolls `speed` px per 32 ms. */
  const AUTO_SCROLL_SPEED_SCALE = 1000 / 32;
  const MAX_AUTO_SCROLL_FRAME_SECONDS = 0.1;
  /** Wheel distance past the top that switches to the previous chapter (desktop's pull-down). */
  const PULL_PREVIOUS_THRESHOLD = 360;

  const chapterId = $derived(Number(page.params.chapterId));
  const s = $derived(settings.values);

  let chapter = $state<Chapter | null>(null);
  let chapters = $state<Chapter[]>([]);
  let novelTitle = $state("");
  let paragraphs = $state<string[]>([]);
  let loading = $state(true);
  let error = $state<string | null>(null);

  // Chrome starts hidden: opening a chapter drops straight into reading.
  let showControls = $state(false);
  let showChapterList = $state(false);
  let showSettings = $state(false);

  let scroller: HTMLElement | undefined = $state();
  let firstVisible = $state(0);
  let atEnd = $state(false);
  let autoScrollPaused = $state(false);
  let pull = $state(0);

  const index = $derived(chapters.findIndex((c) => c.id === chapterId));
  const prev = $derived(index > 0 ? chapters[index - 1] : null);
  const next = $derived(index >= 0 && index < chapters.length - 1 ? chapters[index + 1] : null);
  const percent = $derived(progressPercent(firstVisible, paragraphs.length, atEnd));

  // ---- theme ------------------------------------------------------------------------------
  const pageColors = $derived(
    s.readerTheme === "follow" ? { bg: "var(--background)", fg: "var(--on-background)" } : READER_THEMES[s.readerTheme],
  );
  const fontCss = $derived(
    ({
      original: "var(--font-ui)",
      serif: "Georgia, 'Times New Roman', serif",
      sans: "Arial, 'Segoe UI', sans-serif",
      mono: "Consolas, 'Cascadia Mono', monospace",
      lora: "Lora, Georgia, serif",
      nunito: "Nunito, 'Segoe UI', sans-serif",
    })[s.fontFamily],
  );
  const paragraphSpacing = $derived(s.removeExtraSpacing ? 4 : 16);

  // ---- loading ----------------------------------------------------------------------------
  let loadGen = 0;
  $effect(() => {
    const id = chapterId;
    untrack(() => load(id));
  });

  async function load(id: number) {
    const gen = ++loadGen;
    loading = true;
    error = null;
    showChapterList = false;
    try {
      const ch = await api.chapterGet(id);
      if (gen !== loadGen) return;
      chapter = ch;
      if (!chapters.length || chapters[0].novelId !== ch.novelId) {
        const view = await api.novelGet(ch.novelId);
        if (gen !== loadGen) return;
        chapters = view.chapters;
        novelTitle = view.novel.title;
      }
      const text = await api.chapterContent(id);
      if (gen !== loadGen) return;
      paragraphs = withChapterTitle(ch.name, splitNovelParagraphs(text));
      loading = false;
      await tick();
      restorePosition(ch);
    } catch (e) {
      if (gen === loadGen) {
        error = errorText(e);
        loading = false;
      }
    }
  }

  function restorePosition(ch: Chapter) {
    if (!scroller) return;
    // A finished chapter reopens at the top, like Android.
    const target = ch.read ? 0 : ch.lastParagraph;
    if (target > 0) scroller.querySelector<HTMLElement>(`[data-i="${target}"]`)?.scrollIntoView({ block: "start" });
    else scroller.scrollTop = 0;
    measure();
  }

  // ---- progress ---------------------------------------------------------------------------
  function measure() {
    if (!scroller) return;
    const top = scroller.scrollTop;
    atEnd = top + scroller.clientHeight >= scroller.scrollHeight - 4;
    const nodes = scroller.querySelectorAll<HTMLElement>("[data-i]");
    // Binary search for the first paragraph whose bottom is below the viewport top.
    let lo = 0, hi = nodes.length - 1, found = nodes.length - 1;
    while (lo <= hi) {
      const mid = (lo + hi) >> 1;
      const el = nodes[mid];
      if (el.offsetTop + el.offsetHeight > top + 1) { found = mid; hi = mid - 1; } else lo = mid + 1;
    }
    firstVisible = Math.max(0, found);
    scheduleSave();
  }

  let saveTimer: ReturnType<typeof setTimeout> | undefined;
  function scheduleSave() {
    clearTimeout(saveTimer);
    saveTimer = setTimeout(saveNow, 400);
  }

  function saveNow() {
    clearTimeout(saveTimer);
    if (!chapter || loading || error) return;
    const markRead = atEnd || chapter.read; // reaching the end marks read; never unmarks
    const p = percent;
    if (chapter.lastParagraph === firstVisible && chapter.read === markRead && chapter.progress === p) return;
    chapter.lastParagraph = firstVisible;
    chapter.read = markRead;
    chapter.progress = p;
    const inList = chapters.find((c) => c.id === chapter!.id);
    if (inList) Object.assign(inList, { lastParagraph: firstVisible, read: markRead, progress: p });
    api.chapterSaveProgress(chapter.id, firstVisible, p, markRead).catch((e) => console.error(e));
  }

  let ticking = false;
  function onScroll() {
    if (ticking) return;
    ticking = true;
    requestAnimationFrame(() => {
      ticking = false;
      measure();
    });
  }

  // ---- navigation -------------------------------------------------------------------------
  function goChapter(c: Chapter | null) {
    if (!c) return;
    saveNow();
    goto(`/reader/${c.id}`, { replaceState: true });
  }

  /** End-of-chapter pill: the only control that explicitly finishes the chapter. */
  async function finishAndNext() {
    if (chapter && !chapter.read) {
      chapter.read = true;
      await api.chapterSaveProgress(chapter.id, paragraphs.length - 1, 100, true);
    }
    goChapter(next);
  }

  // Page that opened the reader (chapter switches replace the entry, so only the first counts).
  let openedFrom: string | null = null;
  afterNavigate(({ from }) => {
    if (openedFrom === null) openedFrom = from?.url.pathname ?? "";
  });

  function back() {
    saveNow();
    const details = chapter ? `/novel/${chapter.novelId}` : null;
    // Step back to the details page instead of stacking a second copy of it, so its own back
    // button still reaches the source/library. From History/Updates, swap the reader for details.
    if (!details || openedFrom === details) history.back();
    else goto(details, { replaceState: true });
  }

  async function toggleBookmark() {
    if (!chapter) return;
    const b = !chapter.bookmark;
    await api.chaptersSetBookmark([chapter.id], b);
    chapter.bookmark = b;
    const inList = chapters.find((c) => c.id === chapter!.id);
    if (inList) inList.bookmark = b;
  }

  // Desktop stand-in for the pull-down-past-the-top gesture: keep scrolling up at the top.
  let pullReset: ReturnType<typeof setTimeout> | undefined;
  function onWheel(e: WheelEvent) {
    if (!scroller || !prev || e.deltaY >= 0 || scroller.scrollTop > 0) {
      if (pull) pull = 0;
      return;
    }
    pull = Math.min(PULL_PREVIOUS_THRESHOLD, pull - e.deltaY);
    clearTimeout(pullReset);
    if (pull >= PULL_PREVIOUS_THRESHOLD) {
      pull = 0;
      goChapter(prev);
      return;
    }
    pullReset = setTimeout(() => (pull = 0), 700);
  }

  // ---- seekbar ----------------------------------------------------------------------------
  function seek(e: Event) {
    showControls = true; // seeking never hides the chrome
    if (!scroller) return;
    const v = Number((e.target as HTMLInputElement).value);
    scroller.scrollTop = ((scroller.scrollHeight - scroller.clientHeight) * v) / 100;
  }

  // ---- auto-scroll (per frame, clamped delta; paused by user input) ----------------------
  $effect(() => {
    if (!s.autoScroll || loading || autoScrollPaused || showChapterList || showSettings) return;
    let last = performance.now();
    let carry = 0;
    let raf = requestAnimationFrame(function frame(now) {
      const dt = Math.min(MAX_AUTO_SCROLL_FRAME_SECONDS, (now - last) / 1000);
      last = now;
      if (scroller) {
        carry += s.autoScrollSpeed * AUTO_SCROLL_SPEED_SCALE * dt;
        const px = Math.floor(carry);
        if (px >= 1) { scroller.scrollTop += px; carry -= px; }
      }
      raf = requestAnimationFrame(frame);
    });
    return () => cancelAnimationFrame(raf);
  });

  // ---- battery & clock --------------------------------------------------------------------
  let now = $state(Date.now());
  let battery = $state<number | null>(null);
  onMount(() => {
    const t = setInterval(() => (now = Date.now()), 15_000);
    type Battery = { level: number; addEventListener: (e: string, f: () => void) => void };
    const nav = navigator as Navigator & { getBattery?: () => Promise<Battery> };
    nav.getBattery?.().then((b) => {
      const upd = () => (battery = Math.round(b.level * 100));
      upd();
      b.addEventListener("levelchange", upd);
    }).catch(() => {});
    return () => {
      clearInterval(t);
      saveNow();
    };
  });

  // ---- keep screen on ---------------------------------------------------------------------
  onMount(() => {
    let lock: { release: () => Promise<void> } | null = null;
    const nav = navigator as Navigator & { wakeLock?: { request: (t: "screen") => Promise<{ release: () => Promise<void> }> } };
    nav.wakeLock?.request("screen").then((l) => (lock = l)).catch(() => {});
    return () => { lock?.release(); };
  });

  // ---- fullscreen -------------------------------------------------------------------------
  $effect(() => {
    const full = s.fullscreen;
    getCurrentWindow().setFullscreen(full).catch(() => {});
  });
  onMount(() => () => { if (settings.values.fullscreen) getCurrentWindow().setFullscreen(false).catch(() => {}); });

  // ---- keyboard ---------------------------------------------------------------------------
  function onKey(e: KeyboardEvent) {
    if ((e.target as HTMLElement).tagName === "INPUT" && (e.target as HTMLInputElement).type !== "range") return;
    if (!scroller) return;
    const pageStep = scroller.clientHeight * 0.9;
    switch (e.key) {
      case "ArrowLeft": goChapter(prev); break;
      case "ArrowRight": goChapter(next); break;
      case "PageDown": scroller.scrollBy({ top: pageStep, behavior: "smooth" }); break;
      case "PageUp": scroller.scrollBy({ top: -pageStep, behavior: "smooth" }); break;
      case " ":
        if (s.autoScroll) autoScrollPaused = !autoScrollPaused;
        else scroller.scrollBy({ top: e.shiftKey ? -pageStep : pageStep, behavior: "smooth" });
        break;
      case "Home": scroller.scrollTo({ top: 0 }); break;
      case "End": scroller.scrollTo({ top: scroller.scrollHeight }); break;
      case "c": case "C": showChapterList = !showChapterList; break;
      case "s": case "S": showSettings = !showSettings; break;
      case "b": case "B": toggleBookmark(); break;
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

  function onPageClick(e: MouseEvent) {
    // Selecting text shouldn't toggle the chrome.
    if (window.getSelection()?.toString()) return;
    if ((e.target as HTMLElement).closest("button, a")) return;
    showControls = !showControls;
  }

  const titleIsHeading = $derived(paragraphs.length > 0 && paragraphText(paragraphs[0]).length <= CHAPTER_TITLE_MAX_LENGTH);
  const rendered = $derived(paragraphs.map((p) => paragraphHtml(p, s.bionicReading)));
  const chromeVisible = $derived(showControls && !showChapterList);
</script>

<svelte:window onkeydown={onKey} />

<div
  class="reader"
  style:--page-bg={pageColors.bg}
  style:--page-fg={pageColors.fg}
  style:--reader-font={fontCss}
>
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div class="page" bind:this={scroller} onscroll={onScroll} onwheel={onWheel} onclick={onPageClick}>
    {#if pull > 0}
      <div class="pull-hint" style:opacity={Math.min(1, pull / 120)}>
        {pull > PULL_PREVIOUS_THRESHOLD * 0.6 ? "Keep scrolling for previous chapter" : "Scroll up for previous chapter"}
      </div>
    {/if}

    {#if loading}
      <div class="center"><div class="spinner"></div></div>
    {:else if error}
      <div class="center err">
        <div class="face">(╥﹏╥)</div>
        <div>{error}</div>
        <button class="btn tonal" onclick={() => load(chapterId)}><Icon name="refresh" size={18} />Retry</button>
      </div>
    {:else}
      <article
        style:max-width="{s.columnWidth + s.contentPadding * 2}px"
        style:padding="{CHAPTER_TOP_PADDING}px {s.contentPadding}px {s.showBatteryAndTime ? 48 : 24}px"
        style:font-size="{s.fontSize}px"
        style:line-height={s.lineSpacing}
        style:text-align={s.textAlign}
      >
        {#each rendered as html, i}
          {#if i === 0}
            <p
              data-i="0"
              class="title"
              class:heading={titleIsHeading}
              style:margin-bottom="{paragraphSpacing + s.fontSize * s.lineSpacing}px"
            >{@html html}</p>
          {:else}
            <p data-i={i} style:margin-bottom="{paragraphSpacing}px">{@html html}</p>
          {/if}
        {/each}

        <div class="end">
          <div class="body-medium finished">Finished: {chapter?.name}</div>
          {#if next}
            <button class="btn next-pill" onclick={finishAndNext}>Next: {next.name}<Icon name="chevron_right" size={18} /></button>
          {:else}
            <div class="body-small finished">No more chapters</div>
          {/if}
        </div>
      </article>
    {/if}
  </div>

  <!-- Battery — progress — time strip: always on (gated only by its preference), solid page color, reader font. -->
  {#if s.showBatteryAndTime && !loading && !error}
    <div class="strip">
      <span>{battery !== null ? `${battery}%` : ""}</span>
      <span>{percent}%</span>
      <span>{formatTime(now)}</span>
    </div>
  {/if}

  <!-- Tap-hideable chrome, an overlay over full-bleed content. -->
  {#if chromeVisible}
    <header class="chrome top">
      <button class="icon-btn" title="Back (Esc)" onclick={back}><Icon name="arrow_back" /></button>
      <div class="titles">
        <div class="title-medium ellipsis">{chapter?.name ?? ""}</div>
        <div class="body-small muted ellipsis">{novelTitle}</div>
      </div>
      {#if chapter}
        <button class="icon-btn" title="Open in browser" onclick={() => chapter && openUrl(chapter.url)}><Icon name="public" /></button>
      {/if}
      <button class="icon-btn" title="Fullscreen (F11)" onclick={() => settings.set("fullscreen", !s.fullscreen)}><Icon name={s.fullscreen ? "fullscreen_exit" : "fullscreen"} /></button>
      <button class="icon-btn" title="Settings (S)" onclick={() => (showSettings = true)}><Icon name="tune" /></button>
    </header>

    <!-- Drawn over the strip, not stacked above it. Prev/next dim instead of hiding. -->
    <nav class="chrome bottom">
      <button class="icon-btn" title="Previous chapter (←)" disabled={!prev} onclick={() => goChapter(prev)}><Icon name="chevron_left" /></button>
      <button class="icon-btn" title="Scroll to top (Home)" onclick={() => scroller?.scrollTo({ top: 0, behavior: "smooth" })}><Icon name="vertical_align_top" /></button>
      <button class="icon-btn" title="Chapters (C)" onclick={() => (showChapterList = true)}><Icon name="format_list_bulleted" /></button>
      <button class="icon-btn" class:active={chapter?.bookmark} title="Bookmark (B)" onclick={toggleBookmark}><Icon name="bookmark" fill={chapter?.bookmark} /></button>
      <button class="icon-btn" title="Next chapter (→)" disabled={!next} onclick={() => goChapter(next)}><Icon name="chevron_right" /></button>
    </nav>

    {#if s.showVerticalSeekbar && !loading && !error}
      <div class="chrome seek">
        <span class="label-medium">{percent}</span>
        <input type="range" min="0" max="100" value={percent} oninput={seek} aria-label="Reading progress" />
      </div>
    {/if}
  {/if}

  {#if s.autoScroll && autoScrollPaused}
    <div class="paused label-medium">Auto-scroll paused · Space to resume</div>
  {/if}

  {#if showChapterList && chapter}
    <ChapterListPanel {chapters} currentId={chapter.id} onPick={(c) => goChapter(c)} onClose={() => (showChapterList = false)} />
  {/if}
  {#if showSettings}
    <ReaderSettingsPanel onClose={() => (showSettings = false)} />
  {/if}
</div>

<style>
  .reader {
    position: relative;
    height: 100vh;
    overflow: hidden;
    background: var(--page-bg);
    color: var(--page-fg);
  }
  .page {
    height: 100%;
    overflow-y: auto;
    font-family: var(--reader-font);
  }
  article {
    margin: 0 auto;
    word-wrap: break-word;
  }
  article p {
    margin: 0;
    white-space: pre-line; /* plain-text chapters keep single newlines */
    user-select: text;
  }
  .title {
    font-weight: 700;
  }
  .title.heading {
    font-size: 1.7em; /* CHAPTER_TITLE_SCALE: a multiple of the reader's own size */
    line-height: 1.25;
  }
  article :global(b.bionic) {
    font-weight: 700;
  }
  .end {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 16px;
    padding: 48px 0 32px;
    text-align: center;
    font-family: var(--font-ui);
  }
  .finished { opacity: 0.6; }
  .next-pill { max-width: 100%; overflow: hidden; white-space: nowrap; text-overflow: ellipsis; }
  .center {
    display: grid;
    place-items: center;
    align-content: center;
    gap: 12px;
    height: 100%;
    text-align: center;
    padding: 32px;
  }
  .err .face { font-size: 40px; }
  .pull-hint {
    position: sticky;
    top: 12px;
    z-index: 5;
    width: max-content;
    margin: 0 auto -32px;
    padding: 6px 14px;
    border-radius: 16px;
    background: color-mix(in srgb, var(--page-fg) 12%, var(--page-bg));
    font-family: var(--font-ui);
    font-size: 13px;
  }

  .strip {
    position: absolute;
    left: 0;
    right: 0;
    bottom: 0;
    z-index: 10;
    display: flex;
    justify-content: space-between;
    padding: 6px 24px 8px;
    background: var(--page-bg); /* solid page color, never the translucent chrome */
    color: var(--page-fg);
    font-family: var(--reader-font);
    font-size: 14px; /* bodyMedium */
    font-variant-numeric: tabular-nums;
    opacity: 0.85;
  }

  .chrome {
    position: absolute;
    z-index: 20;
    color: var(--on-surface);
    background: color-mix(in srgb, var(--surface-container) 92%, transparent);
    backdrop-filter: blur(6px);
  }
  .chrome.top {
    top: 0;
    left: 0;
    right: 0;
    display: flex;
    align-items: center;
    gap: 4px;
    height: 64px;
    padding: 0 8px;
    animation: from-top 200ms var(--ease);
  }
  .titles { flex: 1; min-width: 0; margin-left: 4px; }
  .ellipsis { overflow: hidden; white-space: nowrap; text-overflow: ellipsis; }
  .chrome.bottom {
    bottom: 0;
    left: 0;
    right: 0;
    display: flex;
    justify-content: space-evenly;
    height: 64px;
    align-items: center;
    animation: from-bottom 200ms var(--ease);
  }
  .chrome.seek {
    top: 50%;
    right: 16px;
    transform: translateY(-50%);
    width: 40px;
    height: 60vh; /* ~60% of the height, not full length */
    border-radius: 20px;
    display: flex;
    flex-direction: column;
    align-items: center;
    padding: 12px 0;
    gap: 8px;
    animation: from-right 200ms var(--ease);
  }
  .seek input {
    flex: 1;
    writing-mode: vertical-lr;
    width: 24px;
    accent-color: var(--primary);
    cursor: pointer;
  }
  .paused {
    position: absolute;
    top: 16px;
    left: 50%;
    transform: translateX(-50%);
    z-index: 15;
    padding: 6px 14px;
    border-radius: 16px;
    background: var(--inverse-surface);
    color: var(--inverse-on-surface);
  }
  @keyframes from-top { from { transform: translateY(-100%); opacity: 0; } }
  @keyframes from-bottom { from { transform: translateY(100%); opacity: 0; } }
  @keyframes from-right { from { transform: translate(100%, -50%); opacity: 0; } }
</style>
