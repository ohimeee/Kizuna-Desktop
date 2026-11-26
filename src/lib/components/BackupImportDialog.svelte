<script lang="ts">
  // What a phone-backup import added (mounted once in the layout; see backupImport.svelte.ts).
  import Dialog from "./Dialog.svelte";
  import { backupImport } from "$lib/backupImport.svelte";

  const plural = (n: number, one: string, many = `${one}s`) => `${n} ${n === 1 ? one : many}`;
  const r = $derived(backupImport.result);
</script>

<Dialog bind:open={backupImport.dialogOpen} title="Backup imported" width={440}>
  {#if r}
    <p>
      Added {plural(r.novels, "novel")} and {r.manga} manga, with {plural(r.chapters, "chapter")}
      and {plural(r.history, "history entry", "history entries")}.
      {#if r.categoriesCreated}Created {plural(r.categoriesCreated, "category", "categories")}.{/if}
      {#if backupImport.reposAdded}Added {plural(backupImport.reposAdded, "extension repo")}.{/if}
    </p>
    {#if r.missingSources.length}
      <p class="body-small muted">
        These sources aren't installed yet. Their entries are in your library but won't open until you
        install them in Browse (novel sources) or Browse → Manga → Extensions (manga):
      </p>
      <ul class="missing">
        {#each r.missingSources as m}
          <li><span>{m.name}</span><span class="muted body-small">{m.kind} · {plural(m.entries, "entry", "entries")}</span></li>
        {/each}
      </ul>
    {/if}
  {/if}
  {#snippet actions()}
    <button class="btn" onclick={() => (backupImport.dialogOpen = false)}>Done</button>
  {/snippet}
</Dialog>

<style>
  .missing { list-style: none; margin: 8px 0 0; padding: 0; max-height: 220px; overflow: auto; }
  .missing li { display: flex; justify-content: space-between; gap: 12px; padding: 4px 0; }
</style>
