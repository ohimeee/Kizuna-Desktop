<script lang="ts">
  import { page } from "$app/state";
  import Icon from "./Icon.svelte";

  // Same destinations and order as the Android bottom bar.
  const items = [
    { href: "/library", label: "Library", icon: "local_library" },
    { href: "/updates", label: "Updates", icon: "release_alert" },
    { href: "/history", label: "History", icon: "history" },
    { href: "/browse", label: "Browse", icon: "explore" },
    { href: "/more", label: "More", icon: "more_horiz" },
  ];

  const active = (href: string) => page.url.pathname === href || page.url.pathname.startsWith(href + "/");
</script>

<nav class="rail">
  <img class="logo" src="/logo.png" alt="Kizuna" title="Kizuna" />
  {#each items as item}
    {@const on = active(item.href)}
    <a href={item.href} class="item" class:on aria-current={on ? "page" : undefined}>
      <span class="pill"><Icon name={item.icon} fill={on} /></span>
      <span class="label-medium">{item.label}</span>
    </a>
  {/each}
</nav>

<style>
  .rail {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 12px;
    width: var(--rail-width);
    flex: none;
    padding: 12px 0;
    background: var(--surface-container);
  }
  .logo {
    width: 48px;
    height: 48px;
    margin-bottom: 20px;
  }
  .item {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
    width: 100%;
    color: var(--on-surface-variant);
  }
  .pill {
    display: grid;
    place-items: center;
    width: 56px;
    height: 32px;
    border-radius: 16px;
    transition: background 200ms var(--ease);
  }
  .item:hover .pill {
    background: color-mix(in srgb, var(--on-surface) 8%, transparent);
  }
  .item.on {
    color: var(--on-surface);
  }
  .item.on .pill {
    background: var(--secondary-container);
    color: var(--on-secondary-container);
  }
</style>
