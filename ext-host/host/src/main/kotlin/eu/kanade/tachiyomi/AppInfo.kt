package eu.kanade.tachiyomi

/*
 * Kizuna Desktop replacement for Suwayomi's AppInfo (which read their server build config).
 * Used by extensions.
 */
object AppInfo {
    /** Reported to extensions as the host app version (extension-lib 1.3). */
    fun getVersionCode(): Int = 1

    fun getVersionName(): String = "0.2.0"

    /** Image types the reader can display (extension-lib 1.5). */
    fun getSupportedImageMimeTypes(): List<String> =
        listOf("image/jpeg", "image/png", "image/gif", "image/webp", "image/avif", "image/jxl", "image/heif")
}
