<p align="center"><img src="assets/icon.png" width="96" alt="Kizuna"></p>

# Kizuna Desktop

A light novel reader for Windows. It's the desktop companion to [Kizuna](https://github.com/ohimeee/Kizuna) for Android and uses the same novel sources.

## Download

Get **`Kizuna_x.y.z_x64-setup.exe`** from the [latest release](https://github.com/ohimeee/Kizuna-Desktop/releases/latest) and run it. It installs for your user only and doesn't need admin rights. Once installed, the app updates itself.

The installer isn't code-signed, so Windows SmartScreen may warn about an unrecognized app. Click **More info → Run anyway** to continue.

Requires Windows 10 or 11. The Microsoft Edge WebView2 runtime comes with Windows 11, and the installer fetches it on Windows 10 if it's missing.

## Features

- **Sources:** installs the same JS source plugins as the Android app, from the same list.
- **Library:** categories, sorting, unread badges, search, and checking for new chapters.
- **Reader:**
  - fonts, including Lora and Nunito
  - text size, line height, padding and column width
  - reading themes, text alignment and bionic reading
  - auto-scroll and a vertical seekbar
  - a battery, progress and clock strip
  - full keyboard controls
- **Offline:** download chapters and read without a connection.
- **History and Updates** screens.
- **Themes:** the 14 Kizuna themes, in light, dark and pure black.

Royal Road and Webnovel don't work yet. Both sites show a Cloudflare check that the app can't pass.

### Reader keys

| Key | Action |
|---|---|
| ↑ ↓ / mouse wheel | Scroll |
| PgUp / PgDn / Space | Page up / down |
| ← → | Previous / next chapter |
| C | Chapter list |
| S | Reader settings |
| B | Bookmark |
| F11 | Fullscreen |
| Esc | Back |

## Building from source

You need Node.js 20 or later, Rust (stable, MSVC), and the Visual Studio C++ Build Tools.

```sh
npm install
npm run tauri dev     # run with hot reload
npm run tauri build   # build the installers
npm test              # frontend tests
```

From `src-tauri/`, run `cargo test --lib` for the Rust tests. To check every plugin against the live sites, run `cargo run --example smoke -- <path to novel-sources>`.

## License

Apache-2.0. See [LICENSE](LICENSE). The bundled fonts Lora and Nunito are under the SIL Open Font License; see [docs/third-party-licenses/fonts](docs/third-party-licenses/fonts).
