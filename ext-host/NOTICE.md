# kizuna-ext-host notice

This module runs Mihon/Tachiyomi extensions for Kizuna Desktop.

The following parts come from [Suwayomi-Server](https://github.com/Suwayomi/Suwayomi-Server) and
remain under the **Mozilla Public License 2.0** (see `LICENSE-MPL-2.0`), with their original
headers where present:

- `AndroidCompat/` (Android API compatibility layer, including `AndroidCompat/Config`)
- `host/src/main/kotlin/eu/kanade/tachiyomi/` (Tachiyomi source API and networking; Suwayomi's
  FlareSolverr-based `CloudflareInterceptor` is replaced by a pass-through stand-in, `source/local` and storage utilities are not included, and `AppInfo`
  and `NetworkHelper` are modified)
- `host/src/main/kotlin/suwayomi/tachidesk/manga/impl/util/` (`BytecodeEditor`,
  `ChildFirstURLClassLoader`, and small language helpers)
- `host/src/main/kotlin/app/kizuna/exthost/Extensions.kt` (adapted from `PackageTools.kt` and
  `Extension.kt`)

Everything else in Kizuna Desktop is under Apache-2.0.
