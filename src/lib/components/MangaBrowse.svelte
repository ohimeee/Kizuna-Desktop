<script lang="ts">
  // Browse → Manga: Sources (installed extension sources) and Extensions (repos, install/update).
  import { onMount } from "svelte";
  import { errorText } from "$lib/api";
  import Icon from "$lib/components/Icon.svelte";
  import SourceIcon from "$lib/components/SourceIcon.svelte";
  import { toast } from "$lib/components/Toasts.svelte";
  import { mangaApi, newerVersion, type CachedSource, type InstalledExtension, type MangaEngineStatus, type RepoExtension } from "$lib/manga";
  import { settings } from "$lib/settings.svelte";

  let { filter = "" }: { filter?: string } = $props();

  let tab = $state<"sources" | "extensions">("sources");
  let status = $state<MangaEngineStatus | null>(null);
  let cached = $state<Record<string, CachedSource>>({});
  let installed = $state<InstalledExtension[] | null>(null);
  let available = $state<RepoExtension[] | null>(null);
  let repoErrors = $state<string[]>([]);
  let loading = $state(false);
  let busy = $state<Record<string, boolean>>({});
  let newRepo = $state("");

  onMount(async () => {
    status = await mangaApi.status();
    cached = await mangaApi.sourcesCached();
  });

  $effect(() => {
    if (tab === "extensions" && status?.installed && installed === null && !loading) loadExtensions();
  });

  async function loadExtensions() {
    loading = true;
    try {
      const [inst, repo] = await Promise.all([
        mangaApi.extensions(),
        settings.values.mangaRepos.length ? mangaApi.repoList(settings.values.mangaRepos) : Promise.resolve({ extensions: [], errors: [] }),
      ]);
      installed = inst;
      available = repo.extensions;
      repoErrors = repo.errors;
      cached = await mangaApi.sourcesCached();
    } catch (e) {
      toast(errorText(e), undefined, 8000);
      installed = [];
      available = [];
    } finally {
      loading = false;
    }
  }

  function addRepo(e: Event) {
    e.preventDefault();
    const url = newRepo.trim();
    if (!/^https?:\/\//.test(url)) return toast("Enter the repo's https:// URL");
    if (settings.values.mangaRepos.includes(url)) return toast("Already added");
    settings.set("mangaRepos", [...settings.values.mangaRepos, url]);
    newRepo = "";
    installed = null; // reload
  }

  function removeRepo(url: string) {
    settings.set("mangaRepos", settings.values.mangaRepos.filter((r) => r !== url));
    installed = null;
  }

  async function install(e: RepoExtension) {
    busy[e.pkg] = true;
    try {
      const ext = await mangaApi.install(e);
      toast(`Installed ${ext.name} ${ext.versionName}`);
      await loadExtensions();
    } catch (err) {
      toast(`Couldn't install ${e.name}: ${errorText(err)}`, undefined, 9000);
    } finally {
      busy[e.pkg] = false;
    }
  }

  async function uninstall(pkg: string, name: string) {
    busy[pkg] = true;
    try {
      await mangaApi.uninstall(pkg);
      toast(`Removed ${name}`);
      await loadExtensions();
    } catch (err) {
      toast(errorText(err));
    } finally {
      busy[pkg] = false;
    }
  }

  const q = $derived(filter.trim().toLowerCase());
  const sources = $derived(
    Object.entries(cached)
      .map(([id, s]) => ({ id, ...s }))
      .filter((s) => !q || s.name.toLowerCase().includes(q))
      .sort((a, b) => a.name.localeCompare(b.name) || a.lang.localeCompare(b.lang)),
  );
  const installedByPkg = $derived(new Map((installed ?? []).map((e) => [e.pkgName, e])));
  const shownAvailable = $derived(
    (available ?? [])
      .filter((e) => settings.values.mangaShowNsfw || !e.nsfw)
      .filter((e) => !q || e.name.toLowerCase().includes(q))
      .sort((a, b) => a.name.localeCompare(b.name)),
  );
  const updates = $derived(
    (available ?? []).filter((e) => {
      const i = installedByPkg.get(e.pkg);
      return i && newerVersion(e.version, i.versionName);
    }),
  );
  const cleanName = (n: string) => n.replace(/^Tachiyomi:\s*/, "");
</script>

<div class="tabs">
  <button class:on={tab === "sources"} onclick={() => (tab = "sources")}>Sources</button>
  <button class:on={tab === "extensions"} onclick={() => (tab = "extensions")}>
    Extensions {#if updates.length}<span class="badge">{updates.length}</span>{/if}
  </button>
</div>

<div class="scroll">
  {#if status && !status.installed}
    <div class="empty">
      <div class="face">(・o・)</div>
      <div class="body-large">The manga add-on isn't installed</div>
      <div class="body-small">Manga uses Mihon extensions, which need the manga engine. Turn it on in More → Manga.</div>
      <a class="btn tonal" href="/more">Open More</a>
    </div>
  {:else if tab === "sources"}
    {#if sources.length === 0}
      <div class="empty">
        <div class="face">(・o・;)</div>
        <div class="body-large">No manga sources yet</div>
        <button class="btn tonal" onclick={() => (tab = "extensions")}><Icon name="extension" size={18} />Get extensions</button>
      </div>
    {:else}
      <ul class="list">
        {#each sources as s (s.id)}
          <li>
            <a class="row" href="/browse/manga/{s.id}">
              <SourceIcon url={s.baseUrl ? `https://www.google.com/s2/favicons?sz=64&domain=${new URL(s.baseUrl).hostname}` : null} />
              <div class="text">
                <div class="body-large">{s.name}</div>
                <div class="body-small muted">{s.lang.toUpperCase()}</div>
              </div>
            </a>
          </li>
        {/each}
      </ul>
    {/if}
  {:else}
    <section class="repos">
      <h2 class="label-large">Extension repos</h2>
      <p class="body-small muted">Paste the same repo URLs you use in Kizuna or Mihon on Android. No repos are built in.</p>
      {#each settings.values.mangaRepos as r}
        <div class="repo-row body-small"><Icon name="folder" size={18} /><span class="url">{r}</span>
          <button class="icon-btn" title="Remove repo" onclick={() => removeRepo(r)}><Icon name="delete" size={18} /></button></div>
      {/each}
      <form class="add" onsubmit={addRepo}>
        <input class="text-field" placeholder="https://…/index.min.json" bind:value={newRepo} />
        <button class="btn tonal" type="submit"><Icon name="add" size={18} />Add</button>
      </form>
      {#each repoErrors as err}<p class="body-small err">{err}</p>{/each}
    </section>

    {#if loading}
      <div class="empty small"><div class="spinner"></div><div class="body-small">Starting the manga engine…</div></div>
    {:else}
      {#if installed?.length}
        <h2 class="section label-large">Installed</h2>
        <ul class="list">
          {#each installed as e (e.pkgName)}
            {@const repoEntry = (available ?? []).find((a) => a.pkg === e.pkgName)}
            {@const hasUpdate = repoEntry && newerVersion(repoEntry.version, e.versionName)}
            <li class="row">
              <SourceIcon url={repoEntry?.iconUrl} />
              <div class="text">
                <div class="body-large">{cleanName(e.name)} {#if e.nsfw}<span class="nsfw label-small">18+</span>{/if}</div>
                <div class="body-small muted">{e.lang.toUpperCase()} · {e.versionName}{hasUpdate ? ` → ${repoEntry?.version}` : ""} · {e.sources.length} source{e.sources.length === 1 ? "" : "s"}</div>
              </div>
              {#if busy[e.pkgName]}<div class="spinner tiny"></div>
              {:else}
                {#if hasUpdate && repoEntry}<button class="btn" onclick={() => install(repoEntry)}>Update</button>{/if}
                <button class="icon-btn" title="Uninstall" onclick={() => uninstall(e.pkgName, cleanName(e.name))}><Icon name="delete" /></button>
              {/if}
            </li>
          {/each}
        </ul>
      {/if}

      <h2 class="section label-large">
        Available
        <label class="nsfw-toggle body-small"><input type="checkbox" checked={settings.values.mangaShowNsfw}
          onchange={(ev) => settings.set("mangaShowNsfw", (ev.target as HTMLInputElement).checked)} /> Show 18+</label>
      </h2>
      {#if !settings.values.mangaRepos.length}
        <p class="body-small muted pad">Add a repo above to see extensions.</p>
      {/if}
      <ul class="list">
        {#each shownAvailable.filter((e) => !installedByPkg.has(e.pkg)) as e (e.pkg)}
          <li class="row">
            <SourceIcon url={e.iconUrl} />
            <div class="text">
              <div class="body-large">{cleanName(e.name)} {#if e.nsfw}<span class="nsfw label-small">18+</span>{/if}</div>
              <div class="body-small muted">{e.lang.toUpperCase()} · {e.version}</div>
            </div>
            {#if busy[e.pkg]}<div class="spinner tiny"></div>
            {:else}<button class="btn tonal" onclick={() => install(e)}><Icon name="download" size={18} />Install</button>{/if}
          </li>
        {/each}
      </ul>
    {/if}
  {/if}
</div>

<style>
  .tabs { display: flex; gap: 8px; padding: 0 24px; border-bottom: 1px solid var(--outline-variant); flex: none; }
  .tabs button { display: flex; align-items: center; gap: 8px; height: 48px; padding: 0 16px; border: none; background: none; color: var(--on-surface-variant); font-weight: 500; border-bottom: 3px solid transparent; }
  .tabs button.on { color: var(--primary); border-bottom-color: var(--primary); }
  .list { list-style: none; margin: 0; padding: 8px 12px; max-width: 900px; }
  .row { display: flex; align-items: center; gap: 16px; padding: 10px 12px; border-radius: var(--radius-m); }
  a.row:hover { background: color-mix(in srgb, var(--on-surface) 6%, transparent); }
  .text { flex: 1; min-width: 0; }
  .repos { margin: 16px 24px 0; padding: 16px; border-radius: var(--radius-l); background: var(--surface-container-low); max-width: 860px; }
  .repos h2 { margin: 0 0 4px; }
  .repos p { margin: 0 0 8px; }
  .repo-row { display: flex; align-items: center; gap: 8px; color: var(--on-surface); }
  .url { flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .add { display: flex; gap: 8px; margin-top: 8px; }
  .add .text-field { flex: 1; }
  .err { color: var(--error); word-break: break-word; }
  .section { display: flex; align-items: center; gap: 16px; margin: 20px 24px 0; color: var(--primary); }
  .nsfw-toggle { display: flex; align-items: center; gap: 6px; color: var(--on-surface-variant); font-weight: 400; }
  .nsfw { padding: 1px 6px; border-radius: 4px; background: var(--error-container); color: var(--on-error-container); vertical-align: 2px; }
  .pad { padding: 0 24px; }
  .empty.small { height: auto; padding: 24px; }
  .spinner.tiny { width: 24px; height: 24px; border-width: 3px; margin: 0 16px; }
</style>
