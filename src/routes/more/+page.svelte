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
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { renderReleaseNotes } from "$lib/releaseNotes";
  import FeedbackDialog from "$lib/components/FeedbackDialog.svelte";

  const REPO = "https://github.com/ohimeee/Kizuna-Desktop";
  const ANDROID = "https://github.com/ohimeee/Kizuna";
  const KOFI = "https://ko-fi.com/kizunaorg";

  /** "Windows 11 (build 26300)": Windows 11 still reports NT 10.0, so ask for the platform version. */
  async function windowsVersion(): Promise<string> {
    const uaData = (navigator as unknown as { userAgentData?: { getHighEntropyValues(h: string[]): Promise<{ platformVersion?: string }> } }).userAgentData;
    try {
      const v = (await uaData?.getHighEntropyValues(["platformVersion"]))?.platformVersion;
      if (v) return parseInt(v, 10) >= 13 ? "Windows 11" : "Windows 10";
    } catch { /* fall through */ }
    return /Windows NT 10/.test(navigator.userAgent) ? "Windows 10 or 11" : "Windows";
  }

  // Report a problem / suggest an idea: an in-app form (FeedbackDialog) that sends via GitHub.
  let feedbackOpen = $state(false);
  let feedbackKind = $state<"bug" | "idea">("bug");
  let env = $state("");
  async function openIssue(kind: "bug" | "idea") {
    env = `Kizuna Desktop ${version} · ${await windowsVersion()}`;
    feedbackKind = kind;
    feedbackOpen = true;
  }

  // What's new: this version's release notes from GitHub, in the same format as the update dialog.
  let whatsNewOpen = $state(false);
  let whatsNew = $state<{ html: string } | { error: string } | null>(null);
  async function showWhatsNew() {
    whatsNewOpen = true;
    if (whatsNew && "html" in whatsNew) return;
    whatsNew = null;
    try {
      const r = await fetch(`https://api.github.com/repos/ohimeee/Kizuna-Desktop/releases/tags/v${version}`);
      if (!r.ok) throw new Error(`GitHub answered ${r.status}`);
      const body = ((await r.json()) as { body?: string }).body ?? "";
      whatsNew = { html: renderReleaseNotes(body) || "<p>No notes for this version.</p>" };
    } catch {
      whatsNew = { error: "Couldn't load the notes. Check your internet connection, or read them on GitHub." };
    }
  }

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
        <label class="row link toggle">
          <Icon name="visibility_off" />
          <span class="two-line"><span>Incognito mode</span><span class="body-small muted">Pauses reading history: what you read isn't saved to History, progress or read marks</span></span>
          <input type="checkbox" checked={settings.values.incognito} onchange={(e) => settings.set("incognito", (e.target as HTMLInputElement).checked)} />
          <span class="switch"></span>
        </label>
      </section>
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
        <button class="row link btn-row" onclick={showWhatsNew}>
          <Icon name="article" /><span>What's new in {version}</span>
        </button>
        <button class="row link btn-row" disabled={updater.checking} onclick={checkUpdates}>
          <Icon name="update" /><span>{updater.checking ? "Checking…" : "Check for updates"}</span>
        </button>
        <button class="row link btn-row" onclick={() => (onboarding.open = true)}>
          <Icon name="explore" /><span>Onboarding guide</span>
        </button>
      </section>

      <section>
        <h2 class="label-large">Help & support</h2>
        <button class="row link btn-row" onclick={() => openIssue("bug")}>
          <Icon name="bug_report" />
          <span class="two-line"><span>Report a problem</span><span class="body-small muted">Tell us what went wrong; your app and Windows version are added for you</span></span>
        </button>
        <button class="row link btn-row" onclick={() => openIssue("idea")}>
          <Icon name="lightbulb" />
          <span class="two-line"><span>Suggest a feature or a source</span><span class="body-small muted">Ideas and sites you'd like to read from</span></span>
        </button>
        <button class="row link btn-row" onclick={() => openUrl(KOFI)}>
          <Icon name="favorite" fill />
          <span class="two-line"><span>Support Kizuna</span><span class="body-small muted">Tip the developer on Ko-fi. Nothing in the app changes or unlocks</span></span>
          <Icon name="open_in_new" size={18} class="ext" />
        </button>
        <button class="row link btn-row" onclick={() => openUrl(REPO)}>
          <Icon name="code" />
          <span class="two-line"><span>Kizuna Desktop on GitHub</span><span class="body-small muted">Source code and releases. A star helps others find it</span></span>
          <Icon name="open_in_new" size={18} class="ext" />
        </button>
        <button class="row link btn-row" onclick={() => openUrl(ANDROID)}>
          <Icon name="android" />
          <span class="two-line"><span>Kizuna for Android</span><span class="body-small muted">The phone app, with the same sources</span></span>
          <Icon name="open_in_new" size={18} class="ext" />
        </button>
        <div class="body-small muted note">
          Free and open source under the Apache 2.0 license. Reader fonts Lora and Nunito are under the SIL Open Font License.
        </div>
      </section>
    </div>
  </div>
</div>


<FeedbackDialog bind:open={feedbackOpen} bind:kind={feedbackKind} {env} repo={REPO} />

<Dialog bind:open={whatsNewOpen} title="What's new in {version}" width={560}>
  {#if !whatsNew}
    <p class="body-medium muted">Loading…</p>
  {:else if "html" in whatsNew}
    <!-- renderReleaseNotes escapes all text before adding its own tags -->
    <div class="notes">{@html whatsNew.html}</div>
  {:else}
    <p class="body-medium">{whatsNew.error}</p>
  {/if}
  {#snippet actions()}
    <button class="btn text" onclick={() => openUrl(`${REPO}/releases/tag/v${version}`)}>View on GitHub</button>
    <button class="btn" onclick={() => (whatsNewOpen = false)}>Close</button>
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
  .location { justify-content: flex-start; padding: 0 8px; }
  .loc-text { display: flex; flex-direction: column; gap: 2px; flex: 1; min-width: 0; padding: 6px 0; }
  .path { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .loc-actions { display: flex; gap: 8px; flex: none; }
  .engine-text { display: flex; flex-direction: column; gap: 2px; padding: 6px 0; }
  .engine-actions { display: flex; gap: 8px; }
  .spinner.small { width: 24px; height: 24px; border-width: 3px; }
  .engine-progress { width: 100%; height: 4px; accent-color: var(--primary); }
  .error-text { color: var(--error); }
  .toggle { cursor: pointer; }
  .toggle input { position: absolute; opacity: 0; pointer-events: none; }
  .switch {
    position: relative; width: 52px; height: 32px; flex: none; border-radius: 16px;
    background: var(--surface-container-highest); border: 2px solid var(--outline); transition: background 150ms;
  }
  .switch::after {
    content: ""; position: absolute; top: 6px; left: 6px; width: 16px; height: 16px; border-radius: 50%;
    background: var(--outline); transition: all 150ms var(--ease);
  }
  .toggle input:checked + .switch { background: var(--primary); border-color: var(--primary); }
  .toggle input:checked + .switch::after { left: 22px; top: 2px; width: 24px; height: 24px; background: var(--on-primary); }
  .toggle input:focus-visible + .switch { outline: 2px solid var(--primary); outline-offset: 2px; }
  .two-line { display: flex; flex-direction: column; gap: 2px; flex: 1; min-width: 0; padding: 8px 0; }
  .link :global(.ext) { color: var(--on-surface-variant); }
  .link :global(.icon) { flex: none; }
  .note { margin: 8px 8px 0; }
  .notes {
    max-height: min(56vh, 460px); overflow-y: auto; overscroll-behavior: contain;
    padding: 4px 18px 14px 16px; border-radius: var(--radius-m);
    background: var(--surface-container); color: var(--on-surface-variant); line-height: 1.55;
  }
  .notes :global(h3) { margin: 14px 0 6px; font-size: 0.75rem; font-weight: 700; letter-spacing: 0.06em; text-transform: uppercase; color: var(--primary); }
  .notes :global(p) { margin: 12px 0 0; }
  .notes :global(ul) { margin: 0; padding-left: 20px; }
  .notes :global(li) { margin: 6px 0; }
  .notes :global(li::marker) { color: var(--primary); }
  .notes :global(strong) { color: var(--on-surface); font-weight: 600; }
  .notes :global(code) { padding: 1px 5px; border-radius: 4px; font-size: 0.9em; background: var(--surface-container-highest); color: var(--on-surface); }
</style>
