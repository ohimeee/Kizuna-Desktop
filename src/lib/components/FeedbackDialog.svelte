<script lang="ts">
  // Report a problem / suggest an idea, filled in inside the app. Sending opens the finished
  // GitHub issue in the browser (one click to submit there); there's no server to post to, and a
  // GitHub token can't be shipped in a public app. "Copy report" is the no-account fallback.
  import Dialog from "./Dialog.svelte";
  import Icon from "./Icon.svelte";
  import { toast } from "./Toasts.svelte";
  import { openUrl } from "@tauri-apps/plugin-opener";

  let {
    open = $bindable(false),
    kind = $bindable<"bug" | "idea">("bug"),
    env,
    repo,
  }: { open: boolean; kind: "bug" | "idea"; env: string; repo: string } = $props();

  let title = $state("");
  let what = $state("");
  let extra = $state("");

  const labels = $derived(
    kind === "bug"
      ? { what: "What happened?", whatHint: "What did you see, and what did you expect instead?", extra: "How can we make it happen?", extraHint: "Which source, novel or chapter? What did you click?" }
      : { what: "Your idea", whatHint: "A feature, a change, or a site you'd like to read from", extra: "Why would it help?", extraHint: "Optional" },
  );
  const ready = $derived(title.trim().length > 0 && what.trim().length > 0);

  function report(): string {
    const [h1, h2] = kind === "bug" ? ["What happened?", "Steps"] : ["The idea", "Why it would help"];
    return `**${h1}**\n${what.trim()}\n\n**${h2}**\n${extra.trim() || "_not given_"}\n\n---\n${env}`;
  }

  function send() {
    const params = new URLSearchParams({ labels: kind === "bug" ? "bug" : "enhancement", title: title.trim(), body: report() });
    openUrl(`${repo}/issues/new?${params}`);
    toast("Opened in your browser: press Submit new issue there to send it");
    reset();
  }

  async function copy() {
    await navigator.clipboard.writeText(`${title.trim()}\n\n${report()}`);
    toast("Report copied");
  }

  function reset() {
    title = ""; what = ""; extra = "";
    open = false;
  }
</script>

<Dialog bind:open title={kind === "bug" ? "Report a problem" : "Suggest an idea"} width={560}>
  <div class="seg">
    <button class:on={kind === "bug"} onclick={() => (kind = "bug")}><Icon name="bug_report" size={18} />Problem</button>
    <button class:on={kind === "idea"} onclick={() => (kind = "idea")}><Icon name="lightbulb" size={18} />Idea</button>
  </div>

  <label class="field">
    <span class="label-medium">Title</span>
    <input class="text-field" bind:value={title} maxlength="120"
      placeholder={kind === "bug" ? "e.g. Webnovel chapters don't load" : "e.g. Add a sepia theme for the whole app"} />
  </label>
  <label class="field">
    <span class="label-medium">{labels.what}</span>
    <textarea class="text-field area" rows="4" bind:value={what} placeholder={labels.whatHint}></textarea>
  </label>
  <label class="field">
    <span class="label-medium">{labels.extra}</span>
    <textarea class="text-field area" rows="3" bind:value={extra} placeholder={labels.extraHint}></textarea>
  </label>

  <p class="body-small env"><Icon name="info" size={16} /> Added automatically: {env}</p>
  <p class="body-small muted how">Sending opens it on GitHub, ready to submit (a free GitHub account is needed). No account? Copy the report and share it instead.</p>

  {#snippet actions()}
    <button class="btn text" onclick={() => (open = false)}>Cancel</button>
    <button class="btn tonal" disabled={!ready} onclick={copy}>Copy report</button>
    <button class="btn" disabled={!ready} onclick={send}>Send on GitHub</button>
  {/snippet}
</Dialog>

<style>
  .seg { display: inline-flex; margin-bottom: 16px; border: 1px solid var(--outline); border-radius: 20px; overflow: hidden; }
  .seg button {
    display: inline-flex; align-items: center; gap: 6px; padding: 7px 16px; border: none;
    background: none; color: var(--on-surface-variant); font: inherit; font-weight: 500; cursor: pointer;
  }
  .seg button + button { border-left: 1px solid var(--outline); }
  .seg button.on { background: var(--secondary-container); color: var(--on-secondary-container); }
  .field { display: flex; flex-direction: column; gap: 6px; margin-bottom: 14px; color: var(--on-surface); }
  .area { height: auto; min-height: 0; padding-top: 10px; padding-bottom: 10px; resize: vertical; font: inherit; line-height: 1.45; }
  .env { display: flex; align-items: center; gap: 6px; margin: 2px 0 6px; color: var(--on-surface-variant); }
  .env :global(.icon) { color: var(--primary); }
  .how { margin: 0; }
</style>
