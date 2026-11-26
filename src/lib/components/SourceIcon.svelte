<script lang="ts">
  import Icon from "./Icon.svelte";
  let { url, size = 40 }: { url: string | null | undefined; size?: number } = $props();
  let failed = $state(false);
</script>

<span class="src-icon" style:width="{size}px" style:height="{size}px">
  {#if url && !failed}
    <img src={url} alt="" onerror={() => (failed = true)} />
  {:else}
    <Icon name="public" size={size * 0.6} />
  {/if}
</span>

<style>
  .src-icon {
    display: grid;
    place-items: center;
    flex: none;
    border-radius: var(--radius-s);
    overflow: hidden;
    background: var(--surface-container-high);
    color: var(--on-surface-variant);
  }
  img {
    width: 100%;
    height: 100%;
    object-fit: contain;
  }
</style>
