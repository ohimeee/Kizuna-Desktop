<div align="center">

<a href="https://github.com/ohimeee/Kizuna-Desktop">
    <img src="./assets/icon.png" alt="Kizuna logo" title="Kizuna logo" width="80"/>
</a>

# Kizuna Desktop [App](https://github.com/ohimeee/Kizuna-Desktop/releases/latest)

### Light novel reader for Windows
Read light novels on your PC, using the same sources as [Kizuna](https://github.com/ohimeee/Kizuna) for Android.

[![License: Apache-2.0](https://img.shields.io/badge/License-Apache%202.0-0877d2.svg)](/LICENSE)

## Download

[![Kizuna Desktop Stable](https://img.shields.io/github/release/ohimeee/Kizuna-Desktop.svg?maxAge=3600&label=Stable&labelColor=06599d&color=043b69)](https://github.com/ohimeee/Kizuna-Desktop/releases/latest)

*Requires Windows 10 or 11. Download `Kizuna_x.y.z_x64-setup.exe` and run it; no admin rights needed. The app updates itself.*

*The installer isn't code-signed, so Windows SmartScreen may warn about an unrecognized app. Click **More info → Run anyway** to continue.*

## Support

Kizuna is free and open source. If it makes your reading better, you can tip the developer on Ko-fi.

[![Support on Ko-fi](https://img.shields.io/badge/Ko--fi-Support%20Kizuna-ff5e5b?logo=ko-fi&logoColor=white)](https://ko-fi.com/kizunaorg)

## About

Kizuna (絆, "bond/connection") Desktop is the Windows companion to the
[Kizuna](https://github.com/ohimeee/Kizuna) Android app. It installs the same JS novel-source
plugins from the same list, and its reader is a port of the Android novel reader: same paragraph
handling, same title styling, same reading themes. It's built with
[Tauri](https://tauri.app) (a Rust core) and Svelte, so the installer is about 5 MB.

Manga and other image-based reading is planned as an optional add-on.

## Features

<div align="left">

* Novel sources from the Kizuna plugin list (NovelFire, WTR-LAB, Novel Phoenix, NovelPing, Azure Chronicles).
* A native text reader with:
  * font size and font family (including bundled Lora and Nunito)
  * reading themes, line spacing, padding and column width
  * bionic reading and auto-scroll
  * a vertical seekbar and a battery/progress/clock strip
  * keyboard controls (← → chapters, C chapter list, S settings, F11 fullscreen)
* Offline downloads with a download queue.
* Categories to organize your library, with sorting and unread badges.
* Checking your library for new chapters, plus History and Updates screens.
* All 14 Kizuna themes, in light, dark and pure black.
* Automatic updates.

*Known issue: Royal Road and Webnovel don't work yet. Both sites show a Cloudflare check the app can't pass.*

</div>

## Building from source

<div align="left">

You need Node.js 20 or later, Rust (stable, MSVC), and the Visual Studio C++ Build Tools.

```sh
npm install
npm run tauri dev     # run with hot reload
npm run tauri build   # build the installers
npm test              # frontend tests (from src-tauri/: cargo test --lib)
```

</div>

## Contributing

Pull requests are welcome. For major changes, please open an issue first to discuss what you would like to change.

### Credits

Kizuna Desktop's reader follows the [Kizuna](https://github.com/ohimeee/Kizuna) Android reader,
which builds on [Mihon](https://github.com/mihonapp/mihon) and patterns from
[LNReader](https://github.com/LNReader/lnreader). Thank you to all contributors of those projects.

### Disclaimer

The developer(s) of this application does not have any affiliation with the content providers available, and this application hosts zero content.

### License

<pre>
Copyright © 2026 Kizuna Contributors

Licensed under the Apache License, Version 2.0 (the "License");
you may not use this file except in compliance with the License.
You may obtain a copy of the License at

http://www.apache.org/licenses/LICENSE-2.0

Unless required by applicable law or agreed to in writing, software
distributed under the License is distributed on an "AS IS" BASIS,
WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
See the License for the specific language governing permissions and
limitations under the License.
</pre>

The bundled Lora and Nunito fonts are licensed under the SIL Open Font License. See
[docs/third-party-licenses/fonts](./docs/third-party-licenses/fonts).

</div>
