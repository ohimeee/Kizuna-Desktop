<script lang="ts">
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import { api, errorText, type IndexEntry } from "$lib/api";
  import Icon from "$lib/components/Icon.svelte";
  import SourceIcon from "$lib/components/SourceIcon.svelte";
  import { toast } from "$lib/components/Toasts.svelte";
  import { sources } from "$lib/sources.svelte";
  import MangaBrowse from "$lib/components/MangaBrowse.svelte";
  import { page } from "$app/state";

  let kind = $state<"novels" | "manga">(page.url.searchParams.get("kind") === "manga" ? "manga" : "novels");

  let tab = $state<"sources" | "extensions">("sources");
  let filter = $state("");

  let index = $state<IndexEntry[] | null>(null);
  let indexError = $state<string | null>(null);
  let busy = $state<Record<string, boolean>>({});

  onMount(() => {
    sources.refresh();
  });

  $effect(() => {
    if (tab === "extensions" && index === null && indexError === null) loadIndex();
  });

  async function loadIndex() {
    indexError = null;
    try {
      index = await api.sourcesIndex();
    } catch (e) {
      indexError = errorText(e);
    }
  }

  const installedVersion = (pluginId: string) => sources.valid.find((s) => s.pluginId === pluginId)?.version;

  /** Semver-ish compare: is `a` newer than `b`? */
  function newer(a: string, b: string): boolean {
    const pa = a.split(".").map(Number), pb = b.split(".").map(Number);
    for (let i = 0; i < Math.max(pa.length, pb.length); i++) {
      const d = (pa[i] ?? 0) - (pb[i] ?? 0);
      if (d) return d > 0;
    }
    return false;
  }

  async function install(entry: IndexEntry) {
    busy[entry.id] = true;
    try {
      const info = await api.sourceInstall(entry);
      await sources.refresh();
      toast(`Installed ${info.name} ${info.version}`);
    } catch (e) {
      toast(`Couldn't install ${entry.name}: ${errorText(e)}`, undefined, 8000);
    } finally {
      busy[entry.id] = false;
    }
  }

  async function uninstall(pluginId: string, name: string) {
    const src = sources.valid.find((s) => s.pluginId === pluginId);
    if (!src) return;
    busy[pluginId] = true;
    try {
      await api.sourceUninstall(src.id);
      await sources.refresh();
      toast(`Removed ${name}`);
    } catch (e) {
      toast(errorText(e));
    } finally {
      busy[pluginId] = false;
    }
  }

  const q = $derived(filter.trim().toLowerCase());
  const shownSources = $derived(sources.valid.filter((s) => !q || s.name.toLowerCase().includes(q)));
  const shownIndex = $derived((index ?? []).filter((e) => !q || e.name.toLowerCase().includes(q)));
  const broken = $derived(sources.installed.filter((s) => s.error));
  const updates = $derived(
    (index ?? []).filter((e) => {
      const v = installedVersion(e.id);
      return v !== undefined && newer(e.version, v);
    }),
  );
</script>

<div class="screen">
  <header class="top-bar">
    <h1>Browse</h1>
    <div class="seg">
      <button class:on={kind === "novels"} onclick={() => (kind = "novels")}><Icon name="menu_book" size={18} />Novels</button>
      <button class:on={kind === "manga"} onclick={() => (kind = "manga")}><Icon name="auto_stories" size={18} />Manga</button>
    </div>
    <input class="text-field search" placeholder="Search sources" bind:value={filter} />
  </header>

  {#if kind === "manga"}
    <MangaBrowse {filter} />
  {:else}
  <div class="tabs">
    <button class:on={tab === "sources"} onclick={() => (tab = "sources")}>Sources</button>
    <button class:on={tab === "extensions"} onclick={() => (tab = "extensions")}>
      Extensions
      {#if updates.length}<span class="badge">{updates.length}</span>{/if}
    </button>
  </div>

  <div class="scroll">
    {#if tab === "sources"}
      {#if sources.loaded && sources.valid.length === 0}
        <div class="empty">
          <div class="face">(・o・;)</div>
          <div class="body-large">No sources installed</div>
          <button class="btn tonal" onclick={() => (tab = "extensions")}><Icon name="extension" size={18} />Get sources</button>
        </div>
      {:else}
        <ul class="list">
          {#each shownSources as s (s.id)}
            <li>
              <a class="row" href="/browse/{s.id}">
                <SourceIcon url={s.iconUrl} />
                <div class="text">
                  <div class="body-large">{s.name}</div>
                  <div class="body-small muted">{s.lang.toUpperCase()}</div>
                </div>
                <div class="actions">
                  <button
                    class="btn text"
                    onclick={(e) => { e.preventDefault(); e.stopPropagation(); goto(`/browse/${s.id}?tab=popular`); }}
                  >Popular</button>
                  {#if s.supportsLatest}
                    <button
                      class="btn text"
                      onclick={(e) => { e.preventDefault(); e.stopPropagation(); goto(`/browse/${s.id}?tab=latest`); }}
                    >Latest</button>
                  {/if}
                </div>
              </a>
            </li>
          {/each}
        </ul>
      {/if}
      {#if broken.length}
        <h2 class="section label-large">Failed to load</h2>
        <ul class="list">
          {#each broken as b}
            <li class="row"><Icon name="error" /><div class="text"><div>{b.file}</div><div class="body-small muted">{b.error}</div></div></li>
          {/each}
        </ul>
      {/if}
    {:else if indexError}
      <div class="empty">
        <div class="face">(╥﹏╥)</div>
        <div>Couldn't load the source list: {indexError}</div>
        <button class="btn tonal" onclick={loadIndex}><Icon name="refresh" size={18} />Retry</button>
      </div>
    {:else if index === null}
      <div class="empty"><div class="spinner"></div></div>
    {:else}
      <ul class="list">
        {#each shownIndex as e (e.id)}
          {@const v = installedVersion(e.id)}
          {@const hasUpdate = v !== undefined && newer(e.version, v)}
          <li class="row">
            <SourceIcon url={e.iconUrl} />
            <div class="text">
              <div class="body-large">{e.name}</div>
              <div class="body-small muted">
                {e.lang.toUpperCase()} · {v ? (hasUpdate ? `${v} → ${e.version}` : v) : e.version}
              </div>
            </div>
            {#if busy[e.id]}
              <div class="spinner small"></div>
            {:else if v === undefined}
              <button class="btn tonal" onclick={() => install(e)}><Icon name="download" size={18} />Install</button>
            {:else}
              {#if hasUpdate}<button class="btn" onclick={() => install(e)}>Update</button>{/if}
              <button class="icon-btn" title="Uninstall" onclick={() => uninstall(e.id, e.name)}><Icon name="delete" /></button>
            {/if}
          </li>
        {/each}
      </ul>
      <p class="body-small muted note">Royal Road and Webnovel are behind a Cloudflare check that the desktop app can't pass yet.</p>
    {/if}
  </div>
  {/if}
</div>

<style>
  .search {
    width: min(320px, 40vw);
  }
  .seg { display: flex; border: 1px solid var(--outline); border-radius: 20px; overflow: hidden; margin-right: 8px; }
  .seg button { display: flex; align-items: center; gap: 6px; height: 36px; padding: 0 14px; border: none; background: none; color: var(--on-surface-variant); font-weight: 500; }
  .seg button + button { border-left: 1px solid var(--outline); }
  .seg button.on { background: var(--secondary-container); color: var(--on-secondary-container); }
  .tabs {
    display: flex;
    gap: 8px;
    padding: 0 24px;
    border-bottom: 1px solid var(--outline-variant);
    flex: none;
  }
  .tabs button {
    display: flex;
    align-items: center;
    gap: 8px;
    height: 48px;
    padding: 0 16px;
    border: none;
    background: none;
    color: var(--on-surface-variant);
    font-weight: 500;
    border-bottom: 3px solid transparent;
  }
  .tabs button.on {
    color: var(--primary);
    border-bottom-color: var(--primary);
  }
  .list {
    list-style: none;
    margin: 0;
    padding: 8px 12px;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 16px;
    padding: 10px 12px;
    border-radius: var(--radius-m);
  }
  a.row:hover {
    background: color-mix(in srgb, var(--on-surface) 6%, transparent);
  }
  .text {
    flex: 1;
    min-width: 0;
  }
  .actions {
    display: flex;
    flex: none;
    gap: 4px;
  }
  .actions .btn {
    flex: none;
    width: auto;
  }
  .section {
    margin: 16px 24px 0;
    color: var(--error);
  }
  .note {
    padding: 0 24px 24px;
  }
  .spinner.small {
    width: 24px;
    height: 24px;
    border-width: 3px;
    margin: 0 16px;
  }
</style>
