// Manga: Mihon extensions run by the extension host (see src-tauri/src/manga).
import { invoke } from "@tauri-apps/api/core";
import type { BrowsePage } from "./api";

export interface RepoSource { id: string; name: string; lang: string; baseUrl: string | null }
export interface RepoExtension {
  name: string;
  pkg: string;
  apkUrl: string;
  jarUrl?: string | null;
  lang: string;
  code: number;
  version: string;
  nsfw: boolean;
  sources: RepoSource[];
  repo: string;
  iconUrl: string;
}
export interface InstalledSource { id: string; name: string; lang: string; supportsLatest: boolean; baseUrl: string | null; pkgName: string }
export interface InstalledExtension {
  pkgName: string;
  name: string;
  versionName: string;
  versionCode: number;
  lang: string;
  nsfw: boolean;
  signatureHash: string | null;
  sources: InstalledSource[];
}
export interface CachedSource { name: string; lang: string; pkgName: string; supportsLatest: boolean; baseUrl: string | null }
export interface MangaEngineStatus {
  installed: boolean;
  running: boolean;
  java: string;
  jar: string;
  /** Installed download's version; null when not installed or on dev overrides. */
  version: number | null;
  /** Version this app ships with. */
  latest: number;
  downloadSize: number;
  /** Running from KIZUNA_JAVA / KIZUNA_EXT_HOST_JAR. */
  dev: boolean;
}
export interface MangaPageRef { index: number; image: string | null; pageUrl: string }

export const mangaApi = {
  status: () => invoke<MangaEngineStatus>("manga_engine_status"),
  engineInstall: () => invoke<MangaEngineStatus>("manga_engine_install"),
  engineRemove: () => invoke<MangaEngineStatus>("manga_engine_remove"),
  repoList: (repos: string[]) => invoke<{ extensions: RepoExtension[]; errors: string[] }>("manga_repo_list", { repos }),
  extensions: () => invoke<InstalledExtension[]>("manga_extensions"),
  install: (entry: RepoExtension) => invoke<InstalledExtension>("manga_extension_install", { entry }),
  uninstall: (pkg: string) => invoke<void>("manga_extension_uninstall", { pkg }),
  sourcesCached: () => invoke<Record<string, CachedSource>>("manga_sources_cached"),
  browse: (sourceId: string, kind: "popular" | "latest" | "search", page: number, query?: string) =>
    invoke<BrowsePage>("manga_browse", { sourceId, kind, page, query }),
  pages: (chapterId: number) => invoke<MangaPageRef[]>("manga_pages", { chapterId }),
};

/** The kimg route: on Windows, custom schemes are reached as http://<scheme>.localhost/. */
const KIMG = navigator.userAgent.includes("Windows") ? "http://kimg.localhost/i" : "kimg://localhost/i";

/** Image URL that the extension host fetches with the source's own headers. */
export function mangaImage(sourceId: string, url: string | null | undefined): string | null {
  if (!url) return null;
  return `${KIMG}?s=${encodeURIComponent(sourceId)}&u=${encodeURIComponent(url)}`;
}

/** A reader page whose image URL the source resolves from the page URL. */
export function mangaPageImage(sourceId: string, page: MangaPageRef): string {
  return page.image
    ? mangaImage(sourceId, page.image)!
    : `${KIMG}?s=${encodeURIComponent(sourceId)}&p=${encodeURIComponent(page.pageUrl)}`;
}

/** Cover src for a library/browse item of either kind. */
export function coverSrc(kind: string | undefined, sourceId: string, cover: string | null): string | null {
  return kind === "manga" ? mangaImage(sourceId, cover) : cover;
}

/** Semver-ish "is a newer than b". */
export function newerVersion(a: string, b: string): boolean {
  const pa = a.split(".").map(Number), pb = b.split(".").map(Number);
  for (let i = 0; i < Math.max(pa.length, pb.length); i++) {
    const d = (pa[i] ?? 0) - (pb[i] ?? 0);
    if (d) return d > 0;
  }
  return false;
}
