# Plan: phone backup import and sync

Status: **step 1 (one-time import) done** (2026-11-26, `src-tauri/src/backup.rs`, More → Library).
Steps 2-3 (sync) are planned, not started.

Goal: bring the Android app's library (novels and manga) into Kizuna Desktop, first as a one-time
import, then as ongoing sync.

## What the Android backup contains

Kizuna Android writes Mihon backups (`.tachibk`): a gzip-compressed protobuf, written by
`BackupWriter` in `D:\Coding\Kizuna\app\src\main\java\eu\kanade\tachiyomi\data\backup\`.
Models are in `models/`. The fields that matter:

| Message | Fields used |
|---|---|
| `Backup` | 1 `backupManga` (repeated), 2 `backupCategories`, 101 `backupSources`, 106 `backupExtensionStores` |
| `BackupManga` | 1 source (int64), 2 url, 3 title, 4 artist, 5 author, 6 description, 7 genre (repeated), 8 status, 9 thumbnailUrl, 13 dateAdded, 16 chapters, 17 categories (category `order` values), 100 favorite, 104 history, 111 initialized, 112 memo (JSON bytes) |
| `BackupChapter` | 1 url, 2 name, 3 scanlator, 4 read, 5 bookmark, 6 lastPageRead, 7 dateFetch, 8 dateUpload, 9 chapterNumber (float, fixed32), 10 sourceOrder, 13 memo |
| `BackupCategory` | 1 name, 2 order, 3 id |
| `BackupHistory` | 1 chapter url, 2 lastRead |
| `BackupSource` | 1 name, 2 sourceId |
| `BackupExtensionStore` | 1 indexUrl, 2 name |

Encoding notes (kotlinx-serialization ProtoBuf): integers are varints, `Float` is fixed32,
repeated primitives may be packed or not (accept both), null fields are omitted. A file whose
first bytes are `{` is a JSON backup, which Android also rejects.

## How it maps onto desktop

- **Source IDs already match.** Desktop's `plugins::generate_id` mirrors Android's
  `JsNovelSource.generateId` (md5 of `"<id>/<lang>"`), and manga extensions report the same IDs on
  both. So `BackupManga.source` is the desktop `source_id` as-is.
- **Novel or manga:** a source is a novel if its ID matches an installed plugin or any entry in the
  novel plugin catalog (`plugins::fetch_index`, IDs via `generate_id`); everything else is a manga
  extension.
- **Chapter order:** Android's `sourceOrder` is newest-first (0 = newest). Desktop stores reading
  order (0 = chapter 1), so sort by `sourceOrder` descending and number from 0.
- **Progress:** `lastPageRead` is a paragraph index for novels and a page index for manga; both go
  into desktop's `chapters.last_paragraph`. `progress` = 100 when read, else 0 (Android keeps the
  novel percent outside the backup).
- **Categories:** match by name, create missing ones; map `BackupManga.categories` through
  `BackupCategory.order`, as Android's `MangaRestorer` does.
- **Memos:** `memo` bytes go into the `memo` columns (schema v3), so manga pages work right away.
- **History:** match `BackupHistory.url` to the chapter, keep the newer `lastRead`.
- **Extension repos:** `backupExtensionStores[].indexUrl` gets added to the `mangaRepos` setting.

## Merge rules (needed for sync, so build them in from the start)

Backups have no per-chapter modified time, so merging is "union, never lose progress":

- read and bookmark: once true on either side, stay true;
- `last_paragraph`: keep the larger value;
- history: keep the newer `last_read`;
- favorite: added if favorited in the backup, never removed by an import;
- categories: union;
- `date_fetch` = 0 for imported chapters, so they don't flood Updates.

Importing the same backup twice must change nothing.

## Steps

1. **One-time import.** A `backup.rs` module (small protobuf reader + `flate2` gzip, which is
   already in Cargo.lock through `zip`) with the merge rules above, one transaction, unit tests
   using a backup built in the test. "Import backup" in More: pick a file, then show a summary
   (novels, manga, chapters, categories, history, repos added) and which sources still need
   installing.
2. **Phone → desktop sync.** Undecided. Options:
   - *Shared folder:* phone auto-backups land in a folder synced to the PC (Google Drive,
     Syncthing, OneDrive). Desktop watches it and imports each new backup. No Android changes.
   - *Direct Wi-Fi sync:* the phone and PC talk over the local network with a "Sync now" button.
     Needs Android work and a new Android release.
3. **Desktop → phone (two-way).** Desktop writes a `.tachibk` the phone can restore. Android's
   restore already merges, so this direction just needs the writer.

## Open questions

- Sync transport (step 2) and whether two-way is wanted.
- Should the import also install the missing novel plugins automatically?
- Android-only data with no desktop equivalent (trackers, per-manga reader settings,
  preferences): skip it, and say so in the import summary.
