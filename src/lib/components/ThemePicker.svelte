<script lang="ts">
  // Theme mode, the theme grid and pure black: More -> Appearance and the onboarding.
  import Icon from "./Icon.svelte";
  import { settings, type ThemeMode } from "$lib/settings.svelte";
  import { THEMES } from "$lib/theme/themes";
  import { isDark } from "$lib/theme/apply";

  let { columns = 104 }: { columns?: number } = $props();

  const modes: { id: ThemeMode; label: string; icon: string }[] = [
    { id: "system", label: "System", icon: "contrast" },
    { id: "light", label: "Light", icon: "light_mode" },
    { id: "dark", label: "Dark", icon: "dark_mode" },
  ];
  const dark = $derived(isDark(settings.values.themeMode));
</script>

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
<div class="themes" style:grid-template-columns="repeat(auto-fill, minmax({columns}px, 1fr))">
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

<style>
  .row { display: flex; align-items: center; justify-content: space-between; gap: 16px; min-height: 52px; }
  .toggle { cursor: pointer; }
  .toggle input { width: 20px; height: 20px; accent-color: var(--primary); }
  .seg { display: flex; border: 1px solid var(--outline); border-radius: 20px; overflow: hidden; }
  .seg button { display: flex; align-items: center; gap: 6px; height: 36px; padding: 0 14px; border: none; background: none; color: var(--on-surface-variant); }
  .seg button + button { border-left: 1px solid var(--outline); }
  .seg button.on { background: var(--secondary-container); color: var(--on-secondary-container); }
  .themes { display: grid; gap: 12px; margin: 8px 0 12px; }
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
