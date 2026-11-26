// Where downloads are saved (More -> Library and the onboarding). Changing it moves the
// existing downloads along.
import { open } from "@tauri-apps/plugin-dialog";
import { invoke } from "@tauri-apps/api/core";
import { toast } from "$lib/components/Toasts.svelte";
import { errorText } from "./api";

interface Location { path: string; isDefault: boolean }

class DownloadLocationStore {
  path = $state("");
  isDefault = $state(true);
  moving = $state(false);

  private apply(l: Location) {
    this.path = l.path;
    this.isDefault = l.isDefault;
  }

  async refresh() {
    try {
      this.apply(await invoke<Location>("downloads_location"));
    } catch (e) {
      toast(errorText(e));
    }
  }

  /** Folder picker, then move. */
  async choose() {
    const picked = await open({ directory: true, title: "Choose where to save downloads", defaultPath: this.path || undefined });
    if (typeof picked === "string" && picked) await this.set(picked);
  }

  async reset() {
    await this.set(null);
  }

  private async set(path: string | null) {
    this.moving = true;
    try {
      this.apply(await invoke<Location>("downloads_set_location", { path }));
      toast(path ? "Downloads now save to the new folder" : "Downloads now save to the default folder");
    } catch (e) {
      toast(`Couldn't change the folder: ${errorText(e)}`);
    } finally {
      this.moving = false;
    }
  }

  async openFolder() {
    try {
      await invoke("downloads_open_folder");
    } catch (e) {
      toast(`Couldn't open the folder: ${errorText(e)}`);
    }
  }
}

export const downloadLocation = new DownloadLocationStore();
