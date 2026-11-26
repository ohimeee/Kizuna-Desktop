<script lang="ts">
  import "../app.css";
  import { page } from "$app/state";
  import NavRail from "$lib/components/NavRail.svelte";
  import Toasts from "$lib/components/Toasts.svelte";
  import { settings } from "$lib/settings.svelte";
  import { downloads } from "$lib/downloads.svelte";
  import UpdateDialog from "$lib/components/UpdateDialog.svelte";
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
  const immersive = $derived(page.url.pathname.startsWith("/reader"));
</script>

{#if settings.loaded}
  <div class="app" class:immersive>
    {#if !immersive}<NavRail />{/if}
    <main>{@render children()}</main>
  </div>
{/if}
<Toasts />
<UpdateDialog />

<style>
  .app {
    display: flex;
    height: 100vh;
  }
  main {
    flex: 1;
    min-width: 0;
    height: 100%;
    background: var(--background);
  }
</style>
