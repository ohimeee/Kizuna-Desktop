<script lang="ts">
  import { onMount } from "svelte";
  import { api, type Category } from "$lib/api";
  import Dialog from "$lib/components/Dialog.svelte";
  import Icon from "$lib/components/Icon.svelte";

  let categories = $state<Category[]>([]);
  let dialog = $state(false);
  let editing = $state<Category | null>(null);
  let name = $state("");
  let confirmDelete = $state<Category | null>(null);
  let deleteOpen = $state(false);

  onMount(load);
  async function load() {
    categories = await api.categoriesList();
  }

  function openNew() { editing = null; name = ""; dialog = true; }
  function openEdit(c: Category) { editing = c; name = c.name; dialog = true; }

  async function save() {
    const n = name.trim();
    if (!n) return;
    if (editing) await api.categoryRename(editing.id, n);
    else await api.categoryCreate(n);
    dialog = false;
    await load();
  }

  async function move(i: number, d: number) {
    const j = i + d;
    if (j < 0 || j >= categories.length) return;
    const ids = categories.map((c) => c.id);
    [ids[i], ids[j]] = [ids[j], ids[i]];
    await api.categoriesReorder(ids);
    await load();
  }

  async function doDelete() {
    if (confirmDelete) await api.categoryDelete(confirmDelete.id);
    deleteOpen = false;
    await load();
  }
</script>

<div class="screen">
  <header class="top-bar">
    <a class="icon-btn" href="/more" title="Back"><Icon name="arrow_back" /></a>
    <h1>Edit categories</h1>
    <button class="btn" onclick={openNew}><Icon name="add" size={18} />Add</button>
  </header>
  <div class="scroll">
    {#if categories.length === 0}
      <div class="empty"><div class="body-large">No categories yet</div><div class="body-small">Categories become tabs in your library.</div></div>
    {/if}
    <ul>
      {#each categories as c, i (c.id)}
        <li>
          <Icon name="label" />
          <span class="name">{c.name}</span>
          <button class="icon-btn" title="Move up" disabled={i === 0} onclick={() => move(i, -1)}><Icon name="arrow_upward" size={20} /></button>
          <button class="icon-btn" title="Move down" disabled={i === categories.length - 1} onclick={() => move(i, 1)}><Icon name="arrow_downward" size={20} /></button>
          <button class="icon-btn" title="Rename" onclick={() => openEdit(c)}><Icon name="edit" size={20} /></button>
          <button class="icon-btn" title="Delete" onclick={() => { confirmDelete = c; deleteOpen = true; }}><Icon name="delete" size={20} /></button>
        </li>
      {/each}
    </ul>
  </div>
</div>

<Dialog bind:open={dialog} title={editing ? "Rename category" : "Add category"}>
  <!-- svelte-ignore a11y_autofocus -->
  <input class="text-field full" bind:value={name} autofocus placeholder="Name" onkeydown={(e) => e.key === "Enter" && save()} />
  {#snippet actions()}
    <button class="btn text" onclick={() => (dialog = false)}>Cancel</button>
    <button class="btn" disabled={!name.trim()} onclick={save}>{editing ? "Rename" : "Add"}</button>
  {/snippet}
</Dialog>

<Dialog bind:open={deleteOpen} title="Delete category?">
  Delete "{confirmDelete?.name}"? Novels in it stay in your library.
  {#snippet actions()}
    <button class="btn text" onclick={() => (deleteOpen = false)}>Cancel</button>
    <button class="btn" onclick={doDelete}>Delete</button>
  {/snippet}
</Dialog>

<style>
  ul { list-style: none; margin: 0; padding: 8px 16px; max-width: 760px; }
  li { display: flex; align-items: center; gap: 8px; padding: 6px 8px 6px 16px; border-radius: var(--radius-m); background: var(--surface-container-low); margin-bottom: 8px; }
  .name { flex: 1; margin-left: 8px; }
  .full { width: 100%; }
</style>
