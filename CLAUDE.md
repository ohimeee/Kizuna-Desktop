# Kizuna Desktop

Windows companion to [Kizuna](https://github.com/ohimeee/Kizuna) (Android, `D:\Coding\Kizuna`).
Tauri 2 + Rust core + Svelte 5 UI. Public repo: github.com/ohimeee/Kizuna-Desktop (v0.3.0 released).

## Rules
- Commit messages: imperative title, **no trailers ever** (no Co-Authored-By, no "Generated with").
- Novel reader UI follows `D:\Coding\Kizuna\docs\novel-reader-ui-notes.md`; the paragraph splitter
  (`src/lib/reader/paragraphs.ts`) must stay in parity with Android's (tests ported from it).
- Never make HTTP requests imitate a browser fingerprint to get past Cloudflare (that's bot-detection
  evasion; it was blocked and reverted). The extension host's `CloudflareInterceptor` is a
  pass-through stand-in on purpose. Cloudflare sites go through `src-tauri/src/browser.rs` instead: the
  real WebView2 makes the request (page `fetch` in a hidden window) and the user solves any check in it.
  Never auto-solve or hide an interactive check.
- Svelte `$effect`s that call loaders must wrap them in `untrack()` (an effect loop hit before).
- Ask before publishing releases or pushing to the public repos.

## Run / build
- Dev: `npm run tauri dev`. For manga, set first (PowerShell):
  `$env:KIZUNA_JAVA='C:\Program Files\Android\Android Studio\jbr\bin\java.exe'`,
  `$env:KIZUNA_EXT_HOST_JAR='D:\Coding\Kizuna-Desktop\ext-host\host\build\libs\kizuna-ext-host.jar'`.
- Release: set `TAURI_SIGNING_PRIVATE_KEY` to the contents of `~/.tauri/kizuna-desktop.key`
  (no password), then `npm run tauri build`; publish setup.exe, msi, both `.sig`, and `latest.json`.
- Extension host (Kotlin, `ext-host/`): PowerShell, `$env:JAVA_HOME='C:\Program Files\Android\Android Studio\jbr'`,
  `.\gradlew.bat :host:fatJar`. Never pipe gradlew through `tail`. JBR can't `jlink` (no jmods).
- Tests: `npm test` (run it from `D:\...`: a lowercase `d:\` working dir loads vitest twice and fails with
  "Cannot read properties of undefined (reading 'config')"), `cargo test --lib` (in src-tauri), `cargo run --example smoke -- D:\Coding\Kizuna\novel-sources`.

## Phase 4, manga add-on (own engine): done
`ext-host/` runs Mihon extensions (Suwayomi's AndroidCompat + source API, MPL-2.0, see `ext-host/NOTICE.md`);
Rust talks to it over JSON lines (`src-tauri/src/manga/`); UI: Browse → Manga, `/browse/manga/[id]`,
`/manga-reader/[id]`. Verified live (Keiyoushi "Aqua Manga"): install of the prebuilt JAR, popular,
details, chapters, pages, images (`cargo run --example manga_chain -- <repo> "<name>"` runs the whole
chain live). JARs are preferred over APKs; extension `memo` is stored (`memo` columns, schema v3) and passed back.
Engine packaging is done and tested in the app: `scripts/build-manga-engine.ps1 -Version N` builds
`dist/kizuna-manga-engine-N.zip` (jlink'd Temurin 25 runtime + host jar, ~123 MB) and prints the
version/size/SHA-256 to paste into `src-tauri/src/manga/engine.rs`; More → Manga downloads, verifies
and installs it into `<app data>/manga-engine/`. The host builds its own class cache (`manga/host.jsa`)
on exit. `KIZUNA_ENGINE_URL` overrides the download URL for local testing.
Released in v0.2.0. `kizuna-manga-engine-1.zip` must stay attached to the v0.2.0 release (the app
downloads it from there); a new engine needs a new ENGINE_VERSION and its zip on that app release.
Next (not started): JS manga plugins, browser fetching for manga extensions (novels done), phone sync
(backup import is done; sync planned in `docs/phone-backup-and-sync-plan.md`, transport undecided).
