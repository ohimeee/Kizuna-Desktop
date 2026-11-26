# Kizuna Desktop

Windows companion to [Kizuna](https://github.com/ohimeee/Kizuna) (Android, `D:\Coding\Kizuna`).
Tauri 2 + Rust core + Svelte 5 UI. Public repo: github.com/ohimeee/Kizuna-Desktop (v0.1.0 released).

## Rules
- Commit messages: imperative title, **no trailers ever** (no Co-Authored-By, no "Generated with").
- Novel reader UI follows `D:\Coding\Kizuna\docs\novel-reader-ui-notes.md`; the paragraph splitter
  (`src/lib/reader/paragraphs.ts`) must stay in parity with Android's (tests ported from it).
- Never make HTTP requests imitate a browser fingerprint to get past Cloudflare (that's bot-detection
  evasion; it was blocked and reverted). The extension host's `CloudflareInterceptor` is a
  pass-through stand-in on purpose.
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
- Tests: `npm test`, `cargo test --lib` (in src-tauri), `cargo run --example smoke -- D:\Coding\Kizuna\novel-sources`.

## Current work: phase 4, manga add-on (own engine)
`ext-host/` runs Mihon extensions (Suwayomi's AndroidCompat + source API, MPL-2.0, see `ext-host/NOTICE.md`);
Rust talks to it over JSON lines (`src-tauri/src/manga/`); UI: Browse → Manga, `/browse/manga/[id]`,
`/manga-reader/[id]`. Verified live (Keiyoushi "Aqua Manga"): install of the prebuilt JAR, popular,
details, chapters. Next, in order:
1. Rust: add `jar_url` to `RepoExtension` (v2 index `resources.jarUrl`) and download the JAR instead
   of the APK when present (Keiyoushi's minified code fails dex2jar; the host installs `.jar` as-is).
2. Store extension `memo` JSON for manga and chapters (DB columns `memo TEXT DEFAULT '{}'` on novels +
   chapters), save from browse/details/chapters, pass back in details/chapters/pages calls (the host
   already accepts and returns `memo`). Pages fail without it ("Refresh the chapter list").
3. Re-test the chain through pages + images, then hand to the user to try in the real app.
4. Packaging (user chose option 1): download Eclipse Temurin JDK 25 (approved) to `jlink` a trimmed
   runtime with an AppCDS archive; add an "Enable manga" download in More; release v0.2.0.
Later: JS manga plugins, "verify in browser" for Cloudflare sites, phone backup import.
