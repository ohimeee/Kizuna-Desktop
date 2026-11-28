<script lang="ts">
  // Browse → Manga: Sources (installed extension sources) and Extensions (repos, install/update).
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import { errorText } from "$lib/api";
  import Icon from "$lib/components/Icon.svelte";
  import SourceIcon from "$lib/components/SourceIcon.svelte";
  import { toast } from "$lib/components/Toasts.svelte";
  import { mangaApi, newerVersion, type CachedSource, type InstalledExtension, type MangaEngineStatus, type RepoExtension } from "$lib/manga";
  import { settings } from "$lib/settings.svelte";
  import { isSameRepo, repoCandidates } from "$lib/repoLink";
  import { openUrl } from "@tauri-apps/plugin-opener";

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

  // Smart paste: accept a repo's index link, its folder, its GitHub page or an "Add to Mihon"
  // link, try each likely URL, and add the first that really lists extensions.
  let adding = $state(false);
  let addMsg = $state<{ ok: boolean; text: string } | null>(null);
  let showGuide = $state(false);

  async function addRepo(e: Event) {
    e.preventDefault();
    if (adding) return;
    const candidates = repoCandidates(newRepo);
    if (!candidates.length) {
      addMsg = { ok: false, text: "That doesn't look like a link. Copy the repo's link from its website or GitHub page and paste it here." };
      return;
    }
    if (isSameRepo(newRepo, settings.values.mangaRepos)) {
      addMsg = { ok: false, text: "You've already added this repo." };
      return;
    }
    adding = true;
    addMsg = null;
    try {
      let lastError = "";
      for (const c of candidates) {
        const res = await mangaApi.repoList([c]).catch((err) => ({ extensions: [], errors: [errorText(err)] }));
        if (res.extensions.length) {
          settings.set("mangaRepos", [...settings.values.mangaRepos, c]);
          addMsg = { ok: true, text: `Added. ${res.extensions.length} extension${res.extensions.length === 1 ? "" : "s"} found: install the ones for the sites you read below.` };
          newRepo = "";
          installed = null; // reload
          return;
        }
        lastError = res.errors[0] ?? lastError;
      }
      addMsg = {
        ok: false,
        text: /dns|connect|timed? ?out|network|offline/i.test(lastError)
          ? "Couldn't reach that link. Check your internet connection and try again."
          : "That link didn't lead to an extension repo. Make sure you copied the repo's link (it often ends in index.min.json), not the address of a manga site.",
      };
    } finally {
      adding = false;
    }
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
              <div class="actions">
                <button class="btn text" onclick={(e) => { e.preventDefault(); e.stopPropagation(); goto(`/browse/manga/${s.id}?tab=popular`); }}>Popular</button>
                {#if s.supportsLatest}
                  <button class="btn text" onclick={(e) => { e.preventDefault(); e.stopPropagation(); goto(`/browse/manga/${s.id}?tab=latest`); }}>Latest</button>
                {/if}
              </div>
            </a>
          </li>
        {/each}
      </ul>
    {/if}
  {:else}
    <section class="repos">
      <div class="repos-head">
        <h2 class="label-large">Extension repos</h2>
        {#if settings.values.mangaRepos.length}
          <button class="btn text small" onclick={() => (showGuide = !showGuide)}>{showGuide ? "Hide help" : "How does this work?"}</button>
        {/if}
      </div>

      {#if !settings.values.mangaRepos.length || showGuide}
        <div class="guide">
          <p class="body-medium lead">Manga in Kizuna comes from <b>extensions</b>: small add-ons, one per manga site. Extensions are shared in <b>repos</b>, lists made by the community. Kizuna doesn't come with any, so you add the ones you trust.</p>
          <ol class="steps body-medium">
            <li><span class="num">1</span><div><b>Find a repo.</b> Repos are made for Mihon, the manga app for Android, and Kizuna uses the same ones. Each has a website or GitHub page with its link; the community or friends you read with can point you to one. New to this? <button class="link" onclick={() => openUrl("https://mihon.app/docs/guides/getting-started#adding-sources")}>Mihon's getting-started guide</button> explains how repos work.</div></li>
            <li><span class="num">2</span><div><b>Copy its link and paste it below.</b> The repo's link, its GitHub page or its "Add to Mihon" link all work. Kizuna checks it before adding it.</div></li>
            <li><span class="num">3</span><div><b>Install an extension</b> for each site you read from the list that appears, then find it under <b>Sources</b>.</div></li>
          </ol>
          <p class="body-small warn"><Icon name="info" size={16} /> Only add repos you trust: an extension runs inside the app and could contain malware. Repos and extensions are made by other people, not by Kizuna.</p>
          <p class="body-small muted">Already use Mihon or Kizuna on your phone? Use the same repo links (Mihon: More → Settings → Browse → Extension repos), or import your phone backup from More → Library.</p>
        </div>
      {/if}

      {#each settings.values.mangaRepos as r}
        <div class="repo-row body-small"><Icon name="folder" size={18} /><span class="url">{r}</span>
          <button class="icon-btn" title="Remove repo" onclick={() => removeRepo(r)}><Icon name="delete" size={18} /></button></div>
      {/each}
      <form class="add" onsubmit={addRepo}>
        <input class="text-field" placeholder="Paste a repo link" bind:value={newRepo} oninput={() => (addMsg = null)} disabled={adding} />
        <button class="btn tonal" type="submit" disabled={adding || !newRepo.trim()}>
          {#if adding}<Icon name="refresh" size={18} class="spin" />Checking…{:else}<Icon name="add" size={18} />Add{/if}
        </button>
      </form>
      {#if addMsg}<p class="body-small add-msg" class:ok={addMsg.ok} class:err={!addMsg.ok}>{addMsg.text}</p>{/if}
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
  .list { list-style: none; margin: 0; padding: 8px 12px; }
  .row { display: flex; align-items: center; gap: 16px; padding: 10px 12px; border-radius: var(--radius-m); }
  a.row:hover { background: color-mix(in srgb, var(--on-surface) 6%, transparent); }
  .text { flex: 1; min-width: 0; }
  .actions { display: flex; flex: none; gap: 4px; }
  .actions .btn { flex: none; width: auto; }
  .repos { margin: 16px 24px 0; padding: 16px; border-radius: var(--radius-l); background: var(--surface-container-low); }
  .repos h2 { margin: 0 0 4px; }
  .repos p { margin: 0 0 8px; }
  .repo-row { display: flex; align-items: center; gap: 8px; color: var(--on-surface); }
  .repos-head { display: grid; grid-template-columns: 1fr auto; align-items: center; gap: 8px; margin-bottom: 4px; }
  .repos-head h2 { margin: 0; }
  .btn.small { padding: 4px 10px; min-height: 0; margin-left: auto; }
  .guide { margin: 8px 0 14px; padding: 16px 18px; border-radius: var(--radius-m); background: var(--surface-container); }
  .guide b { color: var(--on-surface); font-weight: 600; }
  .guide .lead { margin: 0 0 12px; color: var(--on-surface-variant); line-height: 1.5; }
  .steps { list-style: none; margin: 0 0 12px; padding: 0; display: grid; gap: 10px; color: var(--on-surface-variant); line-height: 1.5; }
  .steps li { display: flex; gap: 12px; align-items: flex-start; }
  .num {
    flex: none; display: grid; place-items: center; width: 26px; height: 26px; border-radius: 50%;
    background: var(--primary-container); color: var(--on-primary-container); font-weight: 700; font-size: 0.8rem;
  }
  .guide p.muted { margin: 0; }
  .guide .warn { display: flex; gap: 8px; align-items: flex-start; margin: 0 0 10px; color: var(--on-surface-variant); }
  .guide .warn :global(.icon) { flex: none; margin-top: 1px; color: var(--primary); }
  .link { border: none; background: none; padding: 0; font: inherit; color: var(--primary); text-decoration: underline; cursor: pointer; }
  .add-msg { margin: 8px 0 0; }
  .add-msg.ok { color: var(--primary); }
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
