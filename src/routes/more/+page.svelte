<script lang="ts">
  import { getVersion } from "@tauri-apps/api/app";
  import Icon from "$lib/components/Icon.svelte";
  import ThemePicker from "$lib/components/ThemePicker.svelte";
  import { settings } from "$lib/settings.svelte";
  import { downloads } from "$lib/downloads.svelte";
  import { backupImport } from "$lib/backupImport.svelte";
  import { downloadLocation } from "$lib/downloadLocation.svelte";
  import { onboarding } from "$lib/onboarding.svelte";
  import { updater } from "$lib/updater.svelte";
  import { toast } from "$lib/components/Toasts.svelte";
  import { mangaEngine } from "$lib/mangaEngine.svelte";
  import Dialog from "$lib/components/Dialog.svelte";

  async function checkUpdates() {
    const found = await updater.check();
    if (!found) toast(updater.lastError ? "Couldn't check for updates" : "You're on the latest version");
  }
  import { onMount } from "svelte";

  let version = $state("");
  onMount(async () => (version = await getVersion()));
  onMount(() => {
    mangaEngine.refresh();
    downloadLocation.refresh();
  });

  const mb = (bytes: number) => `${Math.round(bytes / 1048576)} MB`;
  const engine = $derived(mangaEngine.status);

  let removeDialog = $state(false);
  function removeEngine() {
    removeDialog = false;
    mangaEngine.remove();
  }

</script>

<div class="screen">
  <header class="top-bar"><h1>More</h1></header>
  <div class="scroll">
    <div class="content">
      <section>
        <h2 class="label-large">Appearance</h2>
        <ThemePicker />
      </section>

      <section>
        <h2 class="label-large">Library</h2>
        <a class="row link" href="/more/categories"><Icon name="label" /><span>Edit categories</span></a>
        <a class="row link" href="/more/downloads">
          <Icon name="download" /><span>Download queue</span>
          {#if downloads.count}<span class="badge">{downloads.count}</span>{/if}
        </a>
        <button class="row link btn-row" disabled={backupImport.importing} onclick={() => backupImport.pick()}>
          <Icon name="settings_backup_restore" /><span>{backupImport.importing ? "Importing…" : "Import backup from the phone app"}</span>
        </button>
        <div class="row location">
          <Icon name="folder" />
          <div class="loc-text">
            <span>Download location</span>
            <span class="body-small muted path" title={downloadLocation.path}>{downloadLocation.path}{downloadLocation.isDefault ? " (default)" : ""}</span>
          </div>
          <div class="loc-actions">
            <button class="btn text" onclick={() => downloadLocation.openFolder()}><Icon name="open_in_new" size={18} />Open</button>
            {#if !downloadLocation.isDefault}
              <button class="btn text" disabled={downloadLocation.moving} onclick={() => downloadLocation.reset()}>Use default</button>
            {/if}
            <button class="btn tonal" disabled={downloadLocation.moving} onclick={() => downloadLocation.choose()}>{downloadLocation.moving ? "Moving…" : "Change…"}</button>
          </div>
        </div>
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
        <button class="row link btn-row" onclick={() => (onboarding.open = true)}>
          <Icon name="explore" /><span>Onboarding guide</span>
        </button>
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
  .location { justify-content: flex-start; padding: 0 8px; }
  .loc-text { display: flex; flex-direction: column; gap: 2px; flex: 1; min-width: 0; padding: 6px 0; }
  .path { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .loc-actions { display: flex; gap: 8px; flex: none; }
  .engine-text { display: flex; flex-direction: column; gap: 2px; padding: 6px 0; }
  .engine-actions { display: flex; gap: 8px; }
  .spinner.small { width: 24px; height: 24px; border-width: 3px; }
  .engine-progress { width: 100%; height: 4px; accent-color: var(--primary); }
  .error-text { color: var(--error); }
</style>
