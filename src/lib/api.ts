// Typed wrappers around the Rust commands in src-tauri/src/commands.rs.
import { invoke } from "@tauri-apps/api/core";

export interface FilterOption { label: string; value: string }
export interface FilterDef { id: string; name: string; options: FilterOption[] }

export interface SourceInfo {
  id: string;
  pluginId: string;
  name: string;
  lang: string;
  baseUrl: string;
  version: string;
  supportsLatest: boolean;
  filters: FilterDef[];
  iconUrl: string | null;
  file: string;
}

export type InstalledSource = Partial<SourceInfo> & { file: string; error: string | null };

export interface IndexEntry {
  id: string;
  name: string;
  lang: string;
  version: string;
  fileUrl: string;
  iconUrl: string | null;
}

export interface BrowseItem { id: number; title: string; url: string; cover: string | null; favorite: boolean }
export interface BrowsePage { novels: BrowseItem[]; hasNextPage: boolean }
export type BrowseQuery =
  | { kind: "popular" }
  | { kind: "latest" }
  | { kind: "search"; query: string; filters: Record<string, string> };

export interface Novel {
  id: number;
  sourceId: string;
  url: string;
  title: string;
  cover: string | null;
  author: string | null;
  description: string | null;
  genres: string[];
  status: string | null;
  favorite: boolean;
  initialized: boolean;
  dateAdded: number;
  lastUpdate: number;
}

export interface Chapter {
  id: number;
  novelId: number;
  url: string;
  name: string;
  chapterNumber: number;
  dateUpload: number;
  sourceOrder: number;
  read: boolean;
  bookmark: boolean;
  locked: boolean;
  lastParagraph: number;
  progress: number;
  dateFetch: number;
}

export interface NovelView {
  novel: Novel;
  chapters: Chapter[];
  categoryIds: number[];
  source: SourceInfo | null;
  downloaded: number[];
}

export interface QueueItem { chapterId: number; novelId: number; chapterName: string; novelTitle: string }
export interface DownloadState { active: QueueItem | null; queue: QueueItem[]; paused: boolean }
export type DownloadProgress =
  | { state: "queued"; chapterId: number }
  | { state: "downloading"; chapterId: number }
  | { state: "done"; chapterId: number; novelId: number }
  | { state: "failed"; chapterId: number; error: string }
  | { state: "removed"; chapterId: number };

export interface LibraryItem extends Novel { unread: number; total: number; lastRead: number; categoryIds: number[] }
export interface Category { id: number; name: string; sort: number }
export interface HistoryItem { chapter: Chapter; novelTitle: string; novelCover: string | null; lastRead: number }
export interface UpdateItem { chapter: Chapter; novelTitle: string; novelCover: string | null }
export interface LibraryUpdateResult { newChapters: number; checked: number; errors: string[] }

export const api = {
  sourcesInstalled: () => invoke<InstalledSource[]>("sources_installed"),
  sourcesIndex: (url?: string) => invoke<IndexEntry[]>("sources_index", { url }),
  sourceInstall: (entry: IndexEntry) => invoke<SourceInfo>("source_install", { entry }),
  sourceUninstall: (sourceId: string) => invoke<void>("source_uninstall", { sourceId }),
  sourceBrowse: (sourceId: string, query: BrowseQuery, page: number) =>
    invoke<BrowsePage>("source_browse", { sourceId, query, page }),

  novelGet: (id: number) => invoke<NovelView>("novel_get", { id }),
  novelRefresh: (id: number, details = true, chapters = true) =>
    invoke<NovelView>("novel_refresh", { id, details, chapters }),
  novelSetFavorite: (id: number, favorite: boolean, categoryIds?: number[]) =>
    invoke<void>("novel_set_favorite", { id, favorite, categoryIds }),
  novelSetCategories: (id: number, categoryIds: number[]) =>
    invoke<void>("novel_set_categories", { id, categoryIds }),

  chapterGet: (id: number) => invoke<Chapter>("chapter_get", { id }),
  chapterContent: (chapterId: number) => invoke<string>("chapter_content", { chapterId }),
  chapterSaveProgress: (chapterId: number, paragraph: number, percent: number, markRead: boolean) =>
    invoke<void>("chapter_save_progress", { chapterId, paragraph, percent, markRead }),
  chaptersSetRead: (ids: number[], read: boolean) => invoke<void>("chapters_set_read", { ids, read }),
  chaptersSetBookmark: (ids: number[], bookmark: boolean) => invoke<void>("chapters_set_bookmark", { ids, bookmark }),

  libraryList: () => invoke<LibraryItem[]>("library_list"),
  libraryUpdate: () => invoke<LibraryUpdateResult>("library_update"),
  categoriesList: () => invoke<Category[]>("categories_list"),
  categoryCreate: (name: string) => invoke<Category>("category_create", { name }),
  categoryRename: (id: number, name: string) => invoke<void>("category_rename", { id, name }),
  categoryDelete: (id: number) => invoke<void>("category_delete", { id }),
  categoriesReorder: (ids: number[]) => invoke<void>("categories_reorder", { ids }),

  historyList: () => invoke<HistoryItem[]>("history_list"),
  historyRemove: (novelId: number) => invoke<void>("history_remove", { novelId }),
  historyClear: () => invoke<void>("history_clear"),
  updatesList: () => invoke<UpdateItem[]>("updates_list"),

  downloadsEnqueue: (chapterIds: number[]) => invoke<void>("downloads_enqueue", { chapterIds }),
  downloadsCancel: (chapterIds: number[]) => invoke<void>("downloads_cancel", { chapterIds }),
  downloadsClear: () => invoke<void>("downloads_clear"),
  downloadsSetPaused: (paused: boolean) => invoke<void>("downloads_set_paused", { paused }),
  downloadsState: () => invoke<DownloadState>("downloads_state"),
  downloadsDelete: (novelId: number, chapterIds: number[]) => invoke<void>("downloads_delete", { novelId, chapterIds }),
  downloadsDeleteNovel: (novelId: number) => invoke<void>("downloads_delete_novel", { novelId }),
  downloadsSize: () => invoke<number>("downloads_size"),

  settingsGet: () => invoke<Record<string, unknown>>("settings_get"),
  settingsSet: (key: string, value: unknown) => invoke<void>("settings_set", { key, value }),
};

/** Error text from a failed command (commands reject with a plain string). */
export function errorText(e: unknown): string {
  const raw = typeof e === "string" ? e : e instanceof Error ? e.message : JSON.stringify(e);
  // "popularNovels() failed: Error: Couldn't reach x: timed out\n    at popularNovels (x.js:1:2)"
  // → "Couldn't reach x: timed out". Plugin script errors keep their text.
  return raw
    .replace(/^\w+\(\) failed: (?:Error: )?/, "")
    .replace(/\s+at [\s\S]*$/, "")
    .trim();
}
