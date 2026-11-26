// App + reader settings, persisted in the SQLite settings table. Defaults mirror the Android
// app's preferences (NovelReaderPreferences.kt) so the reader feels the same out of the box.
import { api } from "./api";

export type ReaderTheme = "follow" | "light" | "sepia" | "mint" | "gray" | "black";
export type TextAlign = "left" | "center" | "justify" | "right";
export type ReaderFont = "original" | "serif" | "sans" | "mono" | "lora" | "nunito";
export type ThemeMode = "system" | "light" | "dark";
export type LibrarySort = "title" | "lastRead" | "unread" | "dateAdded" | "totalChapters";

export const defaults = {
  // App
  appTheme: "tachiyomi",
  themeMode: "system" as ThemeMode,
  pureBlack: false,
  libraryColumns: 0, // 0 = auto by width
  librarySort: "title" as LibrarySort,
  librarySortDesc: false,
  libraryUnreadOnly: false,
  showUnreadBadge: true,
  libraryKind: "all" as "all" | "novel" | "manga",

  // Manga
  mangaRepos: [] as string[],
  mangaShowNsfw: false,
  mangaReaderMode: "webtoon" as "webtoon" | "paged-ltr" | "paged-rtl",
  mangaFit: "width" as "width" | "height" | "original",
  mangaPageGap: false,
  mangaMaxWidth: 900,

  // Reader (NovelReaderPreferences)
  fontSize: 16,
  lineSpacing: 1.4,
  contentPadding: 16,
  readerTheme: "follow" as ReaderTheme,
  textAlign: "left" as TextAlign,
  fontFamily: "original" as ReaderFont,
  /** Desktop only: max width of the text column in px (phones are always full width). */
  columnWidth: 760,
  fullscreen: false,
  showBatteryAndTime: true,
  showVerticalSeekbar: true,
  removeExtraSpacing: false,
  bionicReading: false,
  autoScroll: false,
  autoScrollSpeed: 5,
};

export type Settings = typeof defaults;

export const READER_LIMITS = {
  fontSize: [10, 32],
  lineSpacing: [1.0, 2.6],
  contentPadding: [0, 32],
  autoScrollSpeed: [1, 10],
  columnWidth: [480, 1400],
} as const;

/** Same colors as Android's NovelReaderTheme enum. */
export const READER_THEMES: Record<Exclude<ReaderTheme, "follow">, { bg: string; fg: string }> = {
  light: { bg: "#ffffff", fg: "#1a1a1a" },
  sepia: { bg: "#f4ecd8", fg: "#5b4636" },
  mint: { bg: "#e0ede4", fg: "#203a2c" },
  gray: { bg: "#2b2b2b", fg: "#e0e0e0" },
  black: { bg: "#000000", fg: "#d0d0d0" },
};

class SettingsStore {
  values = $state<Settings>({ ...defaults });
  loaded = $state(false);

  async load() {
    const stored = await api.settingsGet();
    const merged: Record<string, unknown> = { ...defaults };
    for (const [k, v] of Object.entries(stored)) {
      if (k in defaults && v !== null && typeof v === typeof (defaults as Record<string, unknown>)[k]) merged[k] = v;
    }
    this.values = merged as Settings;
    this.loaded = true;
  }

  set<K extends keyof Settings>(key: K, value: Settings[K]) {
    this.values[key] = value;
    api.settingsSet(key, value).catch((e) => console.error("saving setting", key, e));
  }
}

export const settings = new SettingsStore();
