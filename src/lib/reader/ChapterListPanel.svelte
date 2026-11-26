<script lang="ts">
  // Slide-in chapter picker (novel-reader-ui-notes.md, "Bottom navigation bar"): opaque panel
  // from the start edge painted in the CHROME palette (not the reading theme), uniform dim over
  // everything else, starts scrolled to the current chapter. Its footer buttons move the list,
  // not the reader.
  import type { Chapter } from "$lib/api";
  import Icon from "$lib/components/Icon.svelte";
  import { formatDate } from "$lib/format";
  import { onMount, tick } from "svelte";

  let { chapters, currentId, onPick, onClose }: {
    chapters: Chapter[];
    currentId: number;
    onPick: (c: Chapter) => void;
    onClose: () => void;
  } = $props();

  let list: HTMLElement | undefined = $state();

  function scrollToCurrent(smooth = false) {
    list?.querySelector(".current")?.scrollIntoView({ block: "center", behavior: smooth ? "smooth" : "instant" });
  }

  onMount(async () => {
    await tick();
    scrollToCurrent();
  });
</script>

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="scrim" onclick={onClose}></div>
<aside class="panel">
  <header>
    <span class="title-medium">Chapters</span>
    <span class="body-small muted">{chapters.length}</span>
    <span class="spacer"></span>
    <button class="icon-btn" title="Close" onclick={onClose}><Icon name="close" /></button>
  </header>
  <ul bind:this={list}>
    {#each chapters as c (c.id)}
      <li>
        <button class="row" class:current={c.id === currentId} class:read={c.read && c.id !== currentId} onclick={() => onPick(c)}>
          <span class="name body-medium">
            {#if c.locked}<Icon name="lock" size={16} />{/if}
            {c.name}
          </span>
          {#if c.bookmark}<Icon name="bookmark" fill size={18} class="bm" />{/if}
          {#if c.dateUpload}<span class="body-small muted date">{formatDate(c.dateUpload)}</span>{/if}
        </button>
      </li>
    {/each}
  </ul>
  <footer>
    <button class="btn text" onclick={() => list?.scrollTo({ top: 0, behavior: "smooth" })}><Icon name="vertical_align_top" size={18} />Scroll to top</button>
    <button class="btn text" onclick={() => scrollToCurrent(true)}><Icon name="my_location" size={18} />Current chapter</button>
  </footer>
</aside>

<style>
  .scrim {
    position: absolute;
    inset: 0;
    z-index: 40;
    background: rgb(0 0 0 / 0.6); /* CHAPTER_PANEL_SCRIM_ALPHA */
    animation: fade 200ms;
  }
  .panel {
    position: absolute;
    top: 0;
    bottom: 0;
    left: 0;
    z-index: 41;
    width: min(440px, 86vw);
    display: flex;
    flex-direction: column;
    background: var(--surface-container);
    color: var(--on-surface);
    box-shadow: 4px 0 16px rgb(0 0 0 / 0.3);
    animation: slide 220ms var(--ease);
  }
  header,
  footer {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 8px 8px 20px;
    flex: none;
  }
  footer {
    justify-content: space-between;
    padding: 8px;
    border-top: 1px solid var(--outline-variant);
  }
  .spacer { flex: 1; }
  ul {
    flex: 1;
    overflow-y: auto;
    list-style: none;
    margin: 0;
    padding: 0 8px;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 18px 12px; /* roomier than Mihon's 12dp rows on purpose */
    border: none;
    border-radius: var(--radius-m);
    background: none;
    text-align: left;
  }
  .row:hover { background: color-mix(in srgb, var(--on-surface) 6%, transparent); }
  .row.current { background: var(--secondary-container); color: var(--primary); font-weight: 600; }
  .row.read { opacity: 0.45; }
  .name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }
  .name :global(.icon) { vertical-align: -3px; }
  .row :global(.bm) { color: var(--primary); }
  .date { flex: none; }
  @keyframes fade { from { opacity: 0; } }
  @keyframes slide { from { transform: translateX(-100%); } }
</style>
