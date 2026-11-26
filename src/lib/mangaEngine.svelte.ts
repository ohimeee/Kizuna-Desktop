// The optional manga engine (More → Manga): status, download progress, install/update/remove.
// A store so a download keeps its progress while the user moves around the app.
import { listen } from "@tauri-apps/api/event";
import { errorText } from "./api";
import { mangaApi, type MangaEngineStatus } from "./manga";

class MangaEngineStore {
  status = $state<MangaEngineStatus | null>(null);
  busy = $state<"installing" | "removing" | null>(null);
  done = $state(0);
  total = $state(0);
  error = $state<string | null>(null);
  #listening = false;

  get updateAvailable() {
    const s = this.status;
    return !!s && s.installed && !s.dev && (s.version ?? 0) < s.latest;
  }

  async refresh() {
    try {
      this.status = await mangaApi.status();
    } catch (e) {
      this.error = errorText(e);
    }
  }

  async install() {
    if (this.busy) return;
    if (!this.#listening) {
      this.#listening = true;
      await listen<{ done: number; total: number }>("manga-engine-progress", (e) => {
        this.done = e.payload.done;
        this.total = e.payload.total;
      });
    }
    this.busy = "installing";
    this.error = null;
    this.done = 0;
    this.total = this.status?.downloadSize ?? 0;
    try {
      this.status = await mangaApi.engineInstall();
    } catch (e) {
      this.error = errorText(e);
    } finally {
      this.busy = null;
    }
  }

  async remove() {
    if (this.busy) return;
    this.busy = "removing";
    this.error = null;
    try {
      this.status = await mangaApi.engineRemove();
    } catch (e) {
      this.error = errorText(e);
    } finally {
      this.busy = null;
    }
  }
}

export const mangaEngine = new MangaEngineStore();
