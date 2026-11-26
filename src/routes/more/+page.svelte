<script lang="ts">
  import { getVersion } from "@tauri-apps/api/app";
  import Icon from "$lib/components/Icon.svelte";
  import { settings, type ThemeMode } from "$lib/settings.svelte";
  import { THEMES } from "$lib/theme/themes";
  import { isDark } from "$lib/theme/apply";
  import { downloads } from "$lib/downloads.svelte";
  import { updater } from "$lib/updater.svelte";
  import { toast } from "$lib/components/Toasts.svelte";
  import { mangaEngine } from "$lib/mangaEngine.svelte";
  import Dialog from "$lib/components/Dialog.svelte";
  import { api, errorText, type BackupImportResult } from "$lib/api";

  async function checkUpdates() {
    const found = await updater.check();
    if (!found) toast(updater.lastError ? "Couldn't check for updates" : "You're on the latest version");
  }
  import { onMount } from "svelte";

  let version = $state("");
  onMount(async () => (version = await getVersion()));
  onMount(() => { mangaEngine.refresh(); });

  const mb = (bytes: number) => `${Math.round(bytes / 1048576)} MB`;
  const engine = $derived(mangaEngine.status);

  let fileInput: HTMLInputElement;
  let importing = $state(false);
  let imported = $state<BackupImportResult | null>(null);
  let reposAdded = $state(0);
  let importDialog = $state(false);

  async function importBackup(e: Event) {
    const input = e.target as HTMLInputElement;
    const file = input.files?.[0];
    input.value = "";
    if (!file) return;
    importing = true;
    try {
      const result = await api.backupImport(new Uint8Array(await file.arrayBuffer()));
      const known = new Set(settings.values.mangaRepos);
      const fresh = [...new Set(result.repos.map((r) => r.trim()))].filter((r) => r && !known.has(r));
      if (fresh.length) settings.set("mangaRepos", [...settings.values.mangaRepos, ...fresh]);
      reposAdded = fresh.length;
      imported = result;
      importDialog = true;
    } catch (err) {
      toast(`Couldn't import: ${errorText(err)}`);
    } finally {
      importing = false;
    }
  }

  const plural = (n: number, one: string, many = `${one}s`) => `${n} ${n === 1 ? one : many}`;

  let removeDialog = $state(false);
  function removeEngine() {
    removeDialog = false;
    mangaEngine.remove();
  }

  const modes: { id: ThemeMode; label: string; icon: string }[] = [
    { id: "system", label: "System", icon: "contrast" },
    { id: "light", label: "Light", icon: "light_mode" },
    { id: "dark", label: "Dark", icon: "dark_mode" },
  ];
  const dark = $derived(isDark(settings.values.themeMode));
</script>

<div class="screen">
  <header class="top-bar"><h1>More</h1></header>
  <div class="scroll">
    <div class="content">
      <section>
        <h2 class="label-large">Appearance</h2>
        <div class="row">
          <span>Theme mode</span>
          <div class="seg">
            {#each modes as m}
              <button class:on={settings.values.themeMode === m.id} onclick={() => settings.set("themeMode", m.id)}>
                <Icon name={m.icon} size={18} />{m.label}
              </button>
            {/each}
          </div>
        </div>
        <div class="themes">
          {#each THEMES as t (t.id)}
            {@const sc = dark ? t.dark : t.light}
            <button class="theme" class:on={settings.values.appTheme === t.id} onclick={() => settings.set("appTheme", t.id)}>
              <div class="preview" style:background={sc["background"]}>
                <div class="bar" style:background={sc["surface-container"]}></div>
                <div class="blk" style:background={sc["primary"]}></div>
                <div class="blk2" style:background={sc["secondary-container"]}></div>
                <div class="blk3" style:background={sc["tertiary"]}></div>
                {#if settings.values.appTheme === t.id}<span class="check" style:background={sc["primary"]} style:color={sc["on-primary"]}><Icon name="check" size={16} /></span>{/if}
              </div>
              <span class="body-small">{t.label}</span>
            </button>
          {/each}
        </div>
        <label class="row toggle">
          <span>Pure black dark mode</span>
          <input type="checkbox" checked={settings.values.pureBlack} onchange={(e) => settings.set("pureBlack", (e.target as HTMLInputElement).checked)} />
        </label>
      </section>

      <section>
        <h2 class="label-large">Library</h2>
        <a class="row link" href="/more/categories"><Icon name="label" /><span>Edit categories</span></a>
        <a class="row link" href="/more/downloads">
          <Icon name="download" /><span>Download queue</span>
          {#if downloads.count}<span class="badge">{downloads.count}</span>{/if}
        </a>
        <button class="row link btn-row" disabled={importing} onclick={() => fileInput.click()}>
          <Icon name="settings_backup_restore" /><span>{importing ? "Importing…" : "Import backup from the phone app"}</span>
        </button>
        <input bind:this={fileInput} type="file" accept=".tachibk,.proto.gz,.gz" hidden onchange={importBackup} />
      </section>

      <section>
        <h2 class="label-large">Manga</h2>
        <div class="row">
          <div class="engine-text">
            <span>Manga engine</span>
            <span class="body-small muted">
              {#if !engine}
                Checking…
              {:else if engine.dev}
                Using the development engine (KIZUNA_JAVA / KIZUNA_EXT_HOST_JAR)
              {:else if mangaEngine.busy === "installing"}
                {mangaEngine.done > 0 && mangaEngine.done >= mangaEngine.total ? "Installing…" : `Downloading… ${mb(mangaEngine.done)} of ${mb(mangaEngine.total)}`}
              {:else if mangaEngine.updateAvailable}
                Update available (version {engine.latest})
              {:else if engine.installed}
                On{engine.version ? ` · version ${engine.version}` : ""}. Add extensions in Browse → Manga.
              {:else}
                Runs Mihon extensions for manga. One-time {mb(engine.downloadSize)} download.
              {/if}
            </span>
          </div>
          {#if engine && !engine.dev}
            {#if mangaEngine.busy === "installing"}
              <div class="spinner small"></div>
            {:else if !engine.installed}
              <button class="btn" onclick={() => mangaEngine.install()}><Icon name="download" size={18} />Enable manga</button>
            {:else}
              <div class="engine-actions">
                {#if mangaEngine.updateAvailable}
                  <button class="btn" onclick={() => mangaEngine.install()}>Update</button>
                {/if}
                <button class="btn text" disabled={!!mangaEngine.busy} onclick={() => (removeDialog = true)}>Remove</button>
              </div>
            {/if}
          {/if}
        </div>
        {#if mangaEngine.busy === "installing"}
          <progress class="engine-progress" max={mangaEngine.total || 1} value={mangaEngine.done}></progress>
        {/if}
        {#if mangaEngine.error}
          <div class="body-small error-text">{mangaEngine.error}</div>
        {/if}
      </section>

      <section>
        <h2 class="label-large">About</h2>
        <div class="row"><span>Kizuna Desktop</span><span class="muted">{version}</span></div>
        <button class="row link btn-row" disabled={updater.checking} onclick={checkUpdates}>
          <Icon name="update" /><span>{updater.checking ? "Checking…" : "Check for updates"}</span>
        </button>
        <div class="body-small muted">
          Light novels on Windows, sharing the Android app's source plugins. Reader fonts Lora and Nunito are under the SIL Open Font License.
        </div>
      </section>
    </div>
  </div>
</div>

<Dialog bind:open={importDialog} title="Backup imported" width={440}>
  {#if imported}
    <p>
      Added {plural(imported.novels, "novel")} and {imported.manga} manga, with {plural(imported.chapters, "chapter")}
      and {plural(imported.history, "history entry", "history entries")}.
      {#if imported.categoriesCreated}Created {plural(imported.categoriesCreated, "category", "categories")}.{/if}
      {#if reposAdded}Added {plural(reposAdded, "extension repo")}.{/if}
    </p>
    {#if imported.missingSources.length}
      <p class="body-small muted">
        These sources aren't installed yet. Their entries are in your library but won't open until you
        install them in Browse (novel sources) or Browse → Manga → Extensions (manga):
      </p>
      <ul class="missing">
        {#each imported.missingSources as m}
          <li><span>{m.name}</span><span class="muted body-small">{m.kind} · {plural(m.entries, "entry", "entries")}</span></li>
        {/each}
      </ul>
    {/if}
  {/if}
  {#snippet actions()}
    <button class="btn" onclick={() => (importDialog = false)}>Done</button>
  {/snippet}
</Dialog>

<Dialog bind:open={removeDialog} title="Remove the manga engine?">
  Your extensions and manga library stay, but manga won't open until you enable the engine again.
  {#snippet actions()}
    <button class="btn text" onclick={() => (removeDialog = false)}>Cancel</button>
    <button class="btn" onclick={removeEngine}>Remove</button>
  {/snippet}
</Dialog>

<style>
  .content { padding: 0 24px 32px; }
  section { margin-bottom: 24px; }
  h2 { color: var(--primary); margin: 16px 0 8px; }
  .row { display: flex; align-items: center; justify-content: space-between; gap: 16px; min-height: 52px; }
  .link { justify-content: flex-start; padding: 0 8px; border-radius: var(--radius-m); }
  .btn-row { width: 100%; border: none; background: none; text-align: left; }
  .link:hover { background: color-mix(in srgb, var(--on-surface) 6%, transparent); }
  .toggle { cursor: pointer; }
  .toggle input { width: 20px; height: 20px; accent-color: var(--primary); }
  .seg { display: flex; border: 1px solid var(--outline); border-radius: 20px; overflow: hidden; }
  .seg button { display: flex; align-items: center; gap: 6px; height: 36px; padding: 0 14px; border: none; background: none; color: var(--on-surface-variant); }
  .seg button + button { border-left: 1px solid var(--outline); }
  .seg button.on { background: var(--secondary-container); color: var(--on-secondary-container); }
  .missing { list-style: none; margin: 8px 0 0; padding: 0; max-height: 220px; overflow: auto; }
  .missing li { display: flex; justify-content: space-between; gap: 12px; padding: 4px 0; }
  .engine-text { display: flex; flex-direction: column; gap: 2px; padding: 6px 0; }
  .engine-actions { display: flex; gap: 8px; }
  .spinner.small { width: 24px; height: 24px; border-width: 3px; }
  .engine-progress { width: 100%; height: 4px; accent-color: var(--primary); }
  .error-text { color: var(--error); }
  .themes { display: grid; grid-template-columns: repeat(auto-fill, minmax(104px, 1fr)); gap: 12px; margin: 8px 0 12px; }
  .theme { display: flex; flex-direction: column; align-items: center; gap: 6px; border: none; background: none; padding: 0; }
  .preview {
    position: relative;
    width: 100%;
    aspect-ratio: 3 / 4;
    border-radius: var(--radius-m);
    border: 1px solid var(--outline-variant);
    overflow: hidden;
  }
  .theme.on .preview { box-shadow: 0 0 0 3px var(--primary); }
  .bar { position: absolute; left: 0; right: 0; bottom: 0; height: 18%; }
  .blk { position: absolute; left: 10%; top: 10%; width: 40%; height: 10%; border-radius: 6px; }
  .blk2 { position: absolute; left: 10%; top: 28%; width: 80%; height: 22%; border-radius: 8px; }
  .blk3 { position: absolute; right: 10%; top: 10%; width: 22%; height: 10%; border-radius: 6px; }
  .check { position: absolute; right: 8px; bottom: 24%; display: grid; place-items: center; width: 24px; height: 24px; border-radius: 50%; }
</style>
