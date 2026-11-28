<script lang="ts">
  import "../app.css";
  import { page } from "$app/state";
  import NavRail from "$lib/components/NavRail.svelte";
  import Icon from "$lib/components/Icon.svelte";
  import Toasts from "$lib/components/Toasts.svelte";
  import { settings } from "$lib/settings.svelte";
  import { downloads } from "$lib/downloads.svelte";
  import UpdateDialog from "$lib/components/UpdateDialog.svelte";
  import Onboarding from "$lib/components/Onboarding.svelte";
  import BackupImportDialog from "$lib/components/BackupImportDialog.svelte";
  import { onboarding } from "$lib/onboarding.svelte";
  import { updater } from "$lib/updater.svelte";
  import { applyTheme } from "$lib/theme/apply";
  import { onMount, tick } from "svelte";
  import { getCurrentWindow } from "@tauri-apps/api/window";

  let { children } = $props();

  onMount(() => {
    // The window starts hidden (tauri.conf.json) so it never flashes white; the splash in
    // app.html is already painted by now, so show it.
    getCurrentWindow().show().catch(() => {});
    const shownAt = performance.now();
    settings.load().then(async () => {
      onboarding.checkFirstRun();
      await tick();
      // Startup is quicker than a blink, so hold the splash long enough to be seen (asked
      // 2026-11-26: ~0.8 s). The app keeps loading underneath; nothing waits on this.
      const SPLASH_MIN_MS = 800;
      await new Promise((r) => setTimeout(r, Math.max(0, SPLASH_MIN_MS - (performance.now() - shownAt))));
      const splash = document.getElementById("splash");
      if (splash) {
        splash.classList.add("hide");
        setTimeout(() => splash.remove(), 300);
      }
    });
    downloads.start();
    // Quiet update check shortly after start (skipped in dev builds).
    if (!import.meta.env.DEV) setTimeout(() => updater.check(true), 4000);
    const mq = window.matchMedia("(prefers-color-scheme: dark)");
    const onChange = () => applyTheme(settings.values.appTheme, settings.values.themeMode, settings.values.pureBlack);
    mq.addEventListener("change", onChange);
    return () => mq.removeEventListener("change", onChange);
  });

  $effect(() => {
    applyTheme(settings.values.appTheme, settings.values.themeMode, settings.values.pureBlack);
  });

  // The reader is full-window, no rail.
  const immersive = $derived(page.url.pathname.startsWith("/reader") || page.url.pathname.startsWith("/manga-reader"));
</script>

{#if settings.loaded}
  <div class="shell">
    {#if settings.values.incognito && !immersive}
      <!-- like Android's banner: never forget nothing is being saved (not in the full-window readers) -->
      <!-- the whole bar turns it off; the pill is the visible hint -->
      <button class="incognito-bar" title="Turn off incognito mode" onclick={() => settings.set("incognito", false)}>
        <span class="inc-msg">
          <Icon name="visibility_off" size={18} />
          <b>Incognito mode</b>
          <span class="inc-sub">History and reading progress aren't being saved</span>
        </span>
        <span class="inc-off">Turn off</span>
      </button>
    {/if}
    <div class="app" class:immersive>
      {#if !immersive}<NavRail />{/if}
      <main>{@render children()}</main>
    </div>
  </div>
{/if}
{#if settings.loaded && onboarding.open}<Onboarding />{/if}
<BackupImportDialog />
<Toasts />
<UpdateDialog />

<style>
  .shell { display: flex; flex-direction: column; height: 100vh; }
  .app {
    display: flex;
    flex: 1;
    min-height: 0;
  }
  .incognito-bar {
    flex: none;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 14px;
    min-height: 36px;
    padding: 0 16px;
    width: 100%;
    border: none;
    background: var(--tertiary-container, var(--secondary-container));
    color: var(--on-tertiary-container, var(--on-secondary-container));
    font: inherit;
    font-size: 13px;
    cursor: pointer;
  }
  .incognito-bar:hover .inc-off { background: color-mix(in srgb, currentColor 14%, transparent); }
  .inc-msg { display: inline-flex; align-items: center; gap: 8px; min-width: 0; }
  .inc-msg b { font-weight: 600; white-space: nowrap; }
  .inc-sub { opacity: 0.8; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .inc-sub::before { content: "·"; margin-right: 8px; opacity: 0.7; }
  .inc-off {
    flex: none;
    padding: 2px 12px;
    border: 1px solid currentColor;
    border-radius: 12px;
    background: transparent;
    color: inherit;
    font-size: 12px;
    font-weight: 600;
  }
  main {
    flex: 1;
    min-width: 0;
    height: 100%;
    background: var(--background);
  }
</style>
