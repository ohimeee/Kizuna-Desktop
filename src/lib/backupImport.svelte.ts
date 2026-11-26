// Importing a phone backup (More -> Library and the onboarding share it). The file picker,
// the import itself and the summary it leaves for <BackupImportDialog>.
import { toast } from "$lib/components/Toasts.svelte";
import { api, errorText, type BackupImportResult } from "./api";
import { settings } from "./settings.svelte";

class BackupImportStore {
  importing = $state(false);
  result = $state<BackupImportResult | null>(null);
  reposAdded = $state(0);
  dialogOpen = $state(false);

  /** Opens the file picker; must run from a click. */
  pick() {
    if (this.importing) return;
    const input = document.createElement("input");
    input.type = "file";
    input.accept = ".tachibk,.proto.gz,.gz";
    input.onchange = () => {
      const file = input.files?.[0];
      if (file) this.run(file);
    };
    input.click();
  }

  private async run(file: File) {
    this.importing = true;
    try {
      const result = await api.backupImport(new Uint8Array(await file.arrayBuffer()));
      const known = new Set(settings.values.mangaRepos);
      const fresh = [...new Set(result.repos.map((r) => r.trim()))].filter((r) => r && !known.has(r));
      if (fresh.length) settings.set("mangaRepos", [...settings.values.mangaRepos, ...fresh]);
      this.reposAdded = fresh.length;
      this.result = result;
      this.dialogOpen = true;
    } catch (err) {
      toast(`Couldn't import: ${errorText(err)}`);
    } finally {
      this.importing = false;
    }
  }
}

export const backupImport = new BackupImportStore();
