<script lang="ts">
  import "../app.css";
  import { page } from "$app/state";
  import NavRail from "$lib/components/NavRail.svelte";
  import Toasts from "$lib/components/Toasts.svelte";
  import { settings } from "$lib/settings.svelte";
  import { downloads } from "$lib/downloads.svelte";
  import { applyTheme } from "$lib/theme/apply";
  import { onMount } from "svelte";

  let { children } = $props();

  onMount(() => {
    settings.load();
    downloads.start();
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
