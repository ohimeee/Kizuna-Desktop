// Kizuna extension host: runs Mihon/Tachiyomi extensions for Kizuna Desktop.
// AndroidCompat and the eu.kanade.tachiyomi source API are taken from Suwayomi-Server (MPL-2.0).
enableFeaturePreview("TYPESAFE_PROJECT_ACCESSORS")
rootProject.name = "kizuna-ext-host"
include("AndroidCompat", "AndroidCompat:Config", "host")
