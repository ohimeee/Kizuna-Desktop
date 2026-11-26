// Live download status, fed by the Rust worker's `download-progress` events.
import { listen } from "@tauri-apps/api/event";
import { api, type DownloadProgress, type QueueItem } from "./api";

export type ChapterDl = "queued" | "downloading" | "failed";

class DownloadsStore {
  /** Per-chapter transient state (downloaded chapters are tracked by the screens). */
  status = $state<Record<number, ChapterDl>>({});
  errors = $state<Record<number, string>>({});
  active = $state<QueueItem | null>(null);
  queue = $state<QueueItem[]>([]);
  paused = $state(false);
  /** Bumps when a chapter finishes, with its novel id, so open screens can mark it. */
  lastDone = $state<{ chapterId: number; novelId: number } | null>(null);
  private started = false;

  async start() {
    if (this.started) return;
    this.started = true;
    await this.refresh();
    await listen<DownloadProgress>("download-progress", (e) => this.apply(e.payload));
  }

  async refresh() {
    const s = await api.downloadsState();
    this.active = s.active;
    this.queue = s.queue;
    this.paused = s.paused;
    const status: Record<number, ChapterDl> = {};
    for (const q of s.queue) status[q.chapterId] = "queued";
    if (s.active) status[s.active.chapterId] = "downloading";
    this.status = status;
  }

  private apply(p: DownloadProgress) {
    switch (p.state) {
      case "queued":
        this.status[p.chapterId] = "queued";
        delete this.errors[p.chapterId];
        break;
      case "downloading":
        this.status[p.chapterId] = "downloading";
        break;
      case "done":
        delete this.status[p.chapterId];
        this.lastDone = { chapterId: p.chapterId, novelId: p.novelId };
        break;
      case "failed":
        this.status[p.chapterId] = "failed";
        this.errors[p.chapterId] = p.error;
        break;
      case "removed":
        delete this.status[p.chapterId];
        break;
    }
    // The queue list itself is cheap to re-read.
    this.refreshQueueSoon();
  }

  private timer: ReturnType<typeof setTimeout> | undefined;
  private refreshQueueSoon() {
    clearTimeout(this.timer);
    this.timer = setTimeout(async () => {
      const s = await api.downloadsState();
      this.active = s.active;
      this.queue = s.queue;
      this.paused = s.paused;
    }, 150);
  }

  get count() {
    return this.queue.length + (this.active ? 1 : 0);
  }
}

export const downloads = new DownloadsStore();
