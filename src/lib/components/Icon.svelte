<script lang="ts">
  // Material Symbols (Rounded, weight 400), the icon set the Android app uses. Only the icons
  // listed in scripts/gen_icons.py are bundled; add a name there and re-run it.
  import { ICONS } from "./icons";

  let { name, size = 24, fill = false, class: cls = "" }: { name: string; size?: number; fill?: boolean; class?: string } =
    $props();
  const svg = $derived((fill && ICONS[`${name}-fill`]) || ICONS[name] || "");
  $effect(() => {
    if (!ICONS[name]) console.warn("icon not bundled:", name);
  });
</script>

<span class="icon {cls}" style:width="{size}px" style:height="{size}px" aria-hidden="true">{@html svg}</span>

<style>
  .icon {
    display: inline-flex;
    flex: none;
    color: inherit;
  }
  .icon :global(svg) {
    width: 100%;
    height: 100%;
    fill: currentColor;
  }
</style>
