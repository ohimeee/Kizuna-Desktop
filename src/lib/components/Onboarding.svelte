<script lang="ts">
  // First-run setup, after the Android app's onboarding (theme, storage, guides; desktop has no
  // permissions to ask for). Shown once on a fresh install; More -> Onboarding guide reopens it.
  import { goto } from "$app/navigation";
  import Icon from "./Icon.svelte";
  import ThemePicker from "./ThemePicker.svelte";
  import { backupImport } from "$lib/backupImport.svelte";
  import { downloadLocation } from "$lib/downloadLocation.svelte";
  import { mangaEngine } from "$lib/mangaEngine.svelte";
  import { onboarding } from "$lib/onboarding.svelte";
  import { onMount } from "svelte";

  const STEPS = [
    { title: "Make it yours", icon: "palette" },
    { title: "Where downloads go", icon: "folder" },
    { title: "Get started", icon: "explore" },
  ];
  let step = $state(0);
  let direction = $state(1);
  const last = $derived(step === STEPS.length - 1);

  onMount(() => {
    downloadLocation.refresh();
    mangaEngine.refresh();
  });

  function go(to: number) {
    direction = to > step ? 1 : -1;
    step = Math.max(0, Math.min(STEPS.length - 1, to));
  }

  function finish(path?: string) {
    onboarding.finish();
    if (path) goto(path);
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === "ArrowRight" && !last) go(step + 1);
    else if (e.key === "ArrowLeft" && step > 0) go(step - 1);
  }

  const mb = (bytes: number) => `${Math.round(bytes / 1048576)} MB`;
  const engine = $derived(mangaEngine.status);
</script>

<svelte:window onkeydown={onKey} />

<div class="backdrop">
  <div class="card" role="dialog" aria-modal="true" aria-label="Welcome to Kizuna">
    <header>
      <img class="logo" src="/logo.png" alt="" />
      <div class="heading">
        <h1 class="headline-small">Welcome to Kizuna!</h1>
        <p class="body-medium muted">Let's set a few things up. You can change all of this later in More.</p>
      </div>
    </header>

    <nav class="steps" aria-label="Steps">
      {#each STEPS as s, i}
        <button class="step" class:on={i === step} class:done={i < step} onclick={() => go(i)}>
          <span class="num">{#if i < step}<Icon name="check" size={16} />{:else}{i + 1}{/if}</span>
          <span class="label-large">{s.title}</span>
        </button>
        {#if i < STEPS.length - 1}<span class="sep"></span>{/if}
      {/each}
    </nav>

    <div class="body">
      {#key step}
        <div class="pane" style:--from="{direction * 32}px">
          {#if step === 0}
            <ThemePicker columns={96} />
          {:else if step === 1}
            <p class="body-medium">
              Kizuna saves chapters you download here, one folder per title. A folder on a roomier drive is a good
              idea if you download a lot of manga.
            </p>
            <div class="location">
              <Icon name="folder" />
              <div class="loc-text">
                <span class="body-small muted">{downloadLocation.isDefault ? "Default folder" : "Chosen folder"}</span>
                <span class="path" title={downloadLocation.path}>{downloadLocation.path}</span>
              </div>
            </div>
            <div class="row-actions">
              <button class="btn tonal" disabled={downloadLocation.moving} onclick={() => downloadLocation.choose()}><Icon name="folder" size={18} />Choose folder…</button>
              {#if !downloadLocation.isDefault}
                <button class="btn text" disabled={downloadLocation.moving} onclick={() => downloadLocation.reset()}>Use the default</button>
              {/if}
            </div>
          {:else}
            <div class="choices">
              <div class="choice">
                <Icon name="settings_backup_restore" size={28} />
                <div class="choice-text">
                  <span class="title-medium">Coming from the phone?</span>
                  <span class="body-small muted">Import a backup from Kizuna on Android (Settings → Data and storage → Create backup). Your library, progress, history and manga repos come across.</span>
                </div>
                <button class="btn tonal" disabled={backupImport.importing} onclick={() => backupImport.pick()}>{backupImport.importing ? "Importing…" : "Import backup"}</button>
              </div>
              <div class="choice">
                <Icon name="explore" size={28} />
                <div class="choice-text">
                  <span class="title-medium">New here?</span>
                  <span class="body-small muted">Install novel sources in Browse → Extensions, then find something to read.</span>
                </div>
                <button class="btn tonal" onclick={() => finish("/browse")}>Open Browse</button>
              </div>
              <div class="choice">
                <Icon name="auto_stories" size={28} />
                <div class="choice-text">
                  <span class="title-medium">Read manga too?</span>
                  <span class="body-small muted">
                    {#if engine?.installed}
                      The manga engine is on. Add your extension repos in Browse → Manga.
                    {:else if mangaEngine.busy === "installing"}
                      Downloading the manga engine… {mb(mangaEngine.done)} of {mb(mangaEngine.total)}
                    {:else}
                      Manga uses Mihon extensions, which need a one-time {engine ? mb(engine.downloadSize) : "120 MB"} download. Optional.
                    {/if}
                  </span>
                </div>
                {#if engine?.installed}
                  <span class="ok"><Icon name="check_circle" fill size={24} /></span>
                {:else if mangaEngine.busy === "installing"}
                  <div class="spinner small"></div>
                {:else if engine && !engine.dev}
                  <button class="btn tonal" onclick={() => mangaEngine.install()}>Enable manga</button>
                {/if}
              </div>
            </div>
          {/if}
        </div>
      {/key}
    </div>

    <footer>
      <button class="btn text" onclick={() => finish()}>Skip</button>
      <span class="grow"></span>
      {#if step > 0}<button class="btn text" onclick={() => go(step - 1)}>Back</button>{/if}
      {#if last}
        <button class="btn" onclick={() => finish()}>Get started</button>
      {:else}
        <button class="btn" onclick={() => go(step + 1)}>Next<Icon name="chevron_right" size={18} /></button>
      {/if}
    </footer>
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 400;
    display: grid;
    place-items: center;
    padding: 24px;
    background: var(--background);
    animation: fade 200ms;
  }
  .card {
    display: flex;
    flex-direction: column;
    width: min(760px, 100%);
    height: min(640px, 100%);
    border-radius: var(--radius-l);
    background: var(--surface-container-low);
    box-shadow: 0 8px 32px rgb(0 0 0 / 0.18);
    overflow: hidden;
  }
  header { display: flex; align-items: center; gap: 16px; padding: 24px 28px 8px; }
  .logo { width: 56px; height: 56px; border-radius: 50%; flex: none; }
  h1 { margin: 0 0 4px; }
  .heading p { margin: 0; }
  .steps { display: flex; align-items: center; gap: 8px; padding: 12px 28px; }
  .step { display: flex; align-items: center; gap: 8px; border: none; background: none; padding: 4px; color: var(--on-surface-variant); border-radius: var(--radius-m); }
  .step.on { color: var(--on-surface); }
  .num {
    display: grid;
    place-items: center;
    width: 26px;
    height: 26px;
    border-radius: 50%;
    border: 2px solid var(--outline);
    font-size: 13px;
    font-weight: 600;
  }
  .step.on .num { border-color: var(--primary); background: var(--primary); color: var(--on-primary); }
  .step.done .num { border-color: var(--tertiary); background: var(--tertiary); color: var(--on-tertiary); }
  .sep { flex: 1; height: 2px; min-width: 16px; background: var(--outline-variant); border-radius: 1px; }
  .body { flex: 1; min-height: 0; margin: 4px 20px; padding: 16px 12px; overflow: auto; border-radius: var(--radius-m); background: var(--surface-container); }
  .pane { animation: slide 220ms cubic-bezier(0.2, 0, 0, 1); }
  .pane > p { margin: 0 0 16px; }
  .location { display: flex; align-items: center; gap: 12px; padding: 12px 16px; border-radius: var(--radius-m); background: var(--surface-container-high); }
  .loc-text { display: flex; flex-direction: column; gap: 2px; min-width: 0; }
  .path { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .row-actions { display: flex; gap: 8px; margin-top: 16px; }
  .choices { display: flex; flex-direction: column; gap: 12px; }
  .choice { display: flex; align-items: center; gap: 16px; padding: 16px; border-radius: var(--radius-m); background: var(--surface-container-high); }
  .choice :global(svg:first-child) { color: var(--primary); flex: none; }
  .choice-text { display: flex; flex-direction: column; gap: 4px; flex: 1; min-width: 0; }
  .choice .btn { flex: none; }
  .ok { color: var(--tertiary); display: grid; }
  .spinner.small { width: 24px; height: 24px; border-width: 3px; }
  footer { display: flex; align-items: center; gap: 8px; padding: 12px 20px 20px; }
  .grow { flex: 1; }
  @keyframes slide { from { opacity: 0; transform: translateX(var(--from)); } to { opacity: 1; transform: none; } }
  @keyframes fade { from { opacity: 0; } }
</style>
