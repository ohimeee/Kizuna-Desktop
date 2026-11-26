// In-app updates from GitHub Releases (latest.json, signed with the key in ~/.tauri).
import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";

class UpdaterStore {
  available = $state<Update | null>(null);
  checking = $state(false);
  installing = $state(false);
  /** 0..1 while downloading. */
  progress = $state(0);
  lastError = $state<string | null>(null);
  dialogOpen = $state(false);

  /** Returns true when an update was found. `quiet` hides errors (startup check). */
  async check(quiet = false): Promise<boolean> {
    if (this.checking) return false;
    this.checking = true;
    this.lastError = null;
    try {
      this.available = await check();
      if (this.available) this.dialogOpen = true;
      return !!this.available;
    } catch (e) {
      // No release published yet, offline, etc.
      if (!quiet) this.lastError = String(e);
      return false;
    } finally {
      this.checking = false;
    }
  }

  async install() {
    if (!this.available || this.installing) return;
    this.installing = true;
    this.progress = 0;
    let total = 0;
    let done = 0;
    try {
      await this.available.downloadAndInstall((ev) => {
        if (ev.event === "Started") total = ev.data.contentLength ?? 0;
        else if (ev.event === "Progress") {
          done += ev.data.chunkLength;
          this.progress = total ? done / total : 0;
        } else if (ev.event === "Finished") this.progress = 1;
      });
      await relaunch();
    } catch (e) {
      this.lastError = String(e);
      this.installing = false;
    }
  }
}

export const updater = new UpdaterStore();
