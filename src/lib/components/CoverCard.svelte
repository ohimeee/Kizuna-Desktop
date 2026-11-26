<script lang="ts">
  // A cover in a grid, Mihon's "compact grid" style: title over a gradient at the bottom,
  // badges in the top-left corner.
  import Icon from "./Icon.svelte";

  let {
    href,
    title,
    cover,
    unread = 0,
    inLibrary = false,
    dimmed = false,
  }: { href: string; title: string; cover: string | null; unread?: number; inLibrary?: boolean; dimmed?: boolean } = $props();

  let failed = $state(false);
</script>

<a {href} class="card" class:dimmed title={title}>
  <div class="cover">
    {#if cover && !failed}
      <img src={cover} alt="" loading="lazy" referrerpolicy="no-referrer" onerror={() => (failed = true)} />
    {:else}
      <div class="placeholder"><Icon name="menu_book" size={40} /></div>
    {/if}
    <div class="badges">
      {#if inLibrary}<span class="badge in-lib label-small">In library</span>{/if}
      {#if unread > 0}<span class="badge">{unread}</span>{/if}
    </div>
    <div class="shade"></div>
    <div class="title title-small">{title}</div>
  </div>
</a>

<style>
  .card {
    display: block;
    border-radius: var(--radius-s);
    padding: 4px;
    transition: background 150ms;
  }
  .card:hover {
    background: color-mix(in srgb, var(--on-surface) 8%, transparent);
  }
  .card.dimmed .cover {
    opacity: 0.38;
  }
  .cover {
    position: relative;
    aspect-ratio: 2 / 3;
    border-radius: var(--radius-s);
    overflow: hidden;
    background: var(--surface-container-high);
  }
  img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .placeholder {
    display: grid;
    place-items: center;
    height: 100%;
    color: var(--on-surface-variant);
    opacity: 0.6;
  }
  .badges {
    position: absolute;
    top: 6px;
    left: 6px;
    display: flex;
    gap: 4px;
  }
  .badge.in-lib {
    background: var(--primary);
    color: var(--on-primary);
  }
  .shade {
    position: absolute;
    inset: 55% 0 0 0;
    background: linear-gradient(transparent, rgb(0 0 0 / 0.75));
  }
  .title {
    position: absolute;
    left: 8px;
    right: 8px;
    bottom: 8px;
    color: #fff;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
    text-shadow: 0 1px 2px rgb(0 0 0 / 0.6);
  }
</style>
