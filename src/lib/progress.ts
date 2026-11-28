// The one place reading progress is saved from the readers. In incognito mode nothing is saved:
// no history, no position, no read marks, matching Kizuna on Android.
import { api } from "./api";
import { settings } from "./settings.svelte";

export function saveProgress(chapterId: number, paragraph: number, percent: number, markRead: boolean): Promise<void> {
  if (settings.values.incognito) return Promise.resolve();
  return api.chapterSaveProgress(chapterId, paragraph, percent, markRead);
}
