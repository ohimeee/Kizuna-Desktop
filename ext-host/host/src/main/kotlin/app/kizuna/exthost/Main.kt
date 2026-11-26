package app.kizuna.exthost

/*
 * Kizuna extension host: loads Mihon/Tachiyomi extensions and answers requests from Kizuna
 * Desktop over stdin/stdout, one JSON object per line.
 *
 *   request:  {"id": 1, "method": "popular", "params": {"source": "123", "page": 1}}
 *   response: {"id": 1, "result": ...}  or  {"id": 1, "error": "message"}
 *
 * Requests run concurrently; responses can arrive out of order (matched by id).
 * Logging goes to stderr (logback.xml), never stdout.
 *
 * Boot sequence follows Suwayomi-Server's ServerSetup.kt (MPL-2.0).
 */

import android.os.Looper
import eu.kanade.tachiyomi.App
import eu.kanade.tachiyomi.createAppModule
import eu.kanade.tachiyomi.network.NetworkHelper
import eu.kanade.tachiyomi.source.Source
import eu.kanade.tachiyomi.source.model.FilterList
import eu.kanade.tachiyomi.source.model.MangasPage
import eu.kanade.tachiyomi.source.model.Page
import eu.kanade.tachiyomi.source.model.SChapter
import eu.kanade.tachiyomi.source.model.SManga
import eu.kanade.tachiyomi.source.online.HttpSource
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.launch
import kotlinx.coroutines.runBlocking
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.encodeToJsonElement
import kotlinx.serialization.json.int
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import kotlinx.serialization.json.put
import org.koin.core.context.startKoin
import uy.kohesive.injekt.Injekt
import uy.kohesive.injekt.api.get
import xyz.nulldev.androidcompat.AndroidCompat
import xyz.nulldev.androidcompat.AndroidCompatInitializer
import xyz.nulldev.androidcompat.androidCompatModule
import xyz.nulldev.ts.config.configManagerModule
import java.io.File
import java.io.PrintStream

private val json = Json { ignoreUnknownKeys = true; encodeDefaults = true; explicitNulls = true }

fun main(args: Array<String>) {
    // stdout is the protocol channel. Anything else that prints (extensions, libraries) must not
    // corrupt it, so keep the real stdout for ourselves and point System.out at stderr.
    val protocol = PrintStream(java.io.FileOutputStream(java.io.FileDescriptor.out), true, "UTF-8")
    System.setOut(System.err)

    val dataRoot = File(args.firstOrNull() ?: error("usage: kizuna-ext-host <data dir>")).absoluteFile
    dataRoot.mkdirs()
    System.setProperty("suwayomi.tachidesk.config.server.rootDir", dataRoot.path)

    boot()
    val registry = ExtensionRegistry(File(dataRoot, "extensions"))
    val loadErrors = registry.loadAll()

    val out = Any()
    fun send(obj: JsonObject) = synchronized(out) { protocol.println(obj.toString()) }
    send(buildJsonObject {
        put("event", "ready")
        put("extensions", registry.extensions.size)
        put("errors", json.encodeToJsonElement(loadErrors))
    })

    val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)
    val input = System.`in`.bufferedReader(Charsets.UTF_8)
    val jobs = java.util.concurrent.ConcurrentLinkedQueue<kotlinx.coroutines.Job>()
    runBlocking {
        while (true) {
            val line = (input.readLine() ?: break).removePrefix("\uFEFF") // EOF: finish up and exit
            if (line.isBlank()) continue
            jobs += scope.launch {
                val req = runCatching { json.parseToJsonElement(line).jsonObject }.getOrNull()
                val id = req?.get("id") ?: JsonNull
                val reply = try {
                    requireNotNull(req) { "bad request" }
                    val method = req["method"]!!.jsonPrimitive.content
                    val params = req["params"] as? JsonObject ?: JsonObject(emptyMap())
                    buildJsonObject { put("id", id); put("result", handle(registry, method, params)) }
                } catch (e: Throwable) {
                    buildJsonObject { put("id", id); put("error", e.message ?: e.toString()) }
                }
                send(reply)
            }
        }
        jobs.forEach { it.join() } // let in-flight requests answer before exiting
    }
    kotlin.system.exitProcess(0)
}

/** Same order as Suwayomi's applicationSetup(): Android main loop, Koin, AndroidCompat. */
private fun boot() {
    Thread {
        @Suppress("DEPRECATION")
        Looper.prepareMainLooper()
        Looper.loop()
    }.apply { name = "android-main"; isDaemon = true }.start()

    val app = App()
    startKoin { modules(createAppModule(app), androidCompatModule(), configManagerModule()) }
    AndroidCompatInitializer().init()
    AndroidCompat().startApp(app)
    Injekt.get<NetworkHelper>() // initialise the shared client and cookie store early
}

// ---- requests ----------------------------------------------------------------------------

@Serializable
data class MangaDto(
    val url: String,
    val title: String,
    val thumbnailUrl: String? = null,
    val author: String? = null,
    val artist: String? = null,
    val description: String? = null,
    val genre: List<String> = emptyList(),
    val status: Int = 0,
    val initialized: Boolean = false,
)

@Serializable
data class MangaPageDto(val mangas: List<MangaDto>, val hasNextPage: Boolean)

@Serializable
data class ChapterDto(
    val url: String,
    val name: String,
    val dateUpload: Long,
    val chapterNumber: Float,
    val scanlator: String? = null,
)

@Serializable
data class PageDto(val index: Int, val url: String, val imageUrl: String?)

@Serializable
data class ImageDto(val path: String, val contentType: String?)

private fun SManga.toDto() = MangaDto(
    url = url,
    title = title,
    thumbnailUrl = thumbnail_url,
    author = author,
    artist = artist,
    description = description,
    genre = genre?.split(",")?.map { it.trim() }?.filter { it.isNotEmpty() }.orEmpty(),
    status = status,
    initialized = initialized,
)

private fun MangasPage.toDto() = MangaPageDto(mangas.map { it.toDto() }, hasNextPage)

private fun SChapter.toDto() = ChapterDto(url, name, date_upload, chapter_number, scanlator)

private fun mangaOf(params: JsonObject) = SManga.create().apply {
    url = params.str("url")
    title = params["title"]?.jsonPrimitive?.content ?: ""
}

private fun JsonObject.str(key: String) = this[key]?.jsonPrimitive?.content ?: error("missing \"$key\"")

private fun Map<Long, Source>.of(params: JsonObject): Source {
    val id = params.str("source").toLong()
    return this[id] ?: error("Source $id is not installed")
}

private suspend fun handle(reg: ExtensionRegistry, method: String, p: JsonObject): JsonElement =
    when (method) {
        "ping" -> JsonPrimitive("pong")
        "extensions" -> json.encodeToJsonElement(reg.extensions.values.sortedBy { it.name })
        "install" -> json.encodeToJsonElement(reg.install(File(p.str("apk"))))
        "uninstall" -> { reg.uninstall(p.str("pkg")); JsonNull }
        "popular" -> json.encodeToJsonElement(reg.sources.of(p).getPopularManga(p["page"]!!.jsonPrimitive.int).toDto())
        "latest" -> json.encodeToJsonElement(reg.sources.of(p).getLatestUpdates(p["page"]!!.jsonPrimitive.int).toDto())
        "search" -> {
            val source = reg.sources.of(p)
            val result = source.getSearchManga(p["page"]!!.jsonPrimitive.int, p.str("query"), source.getFilterList())
            json.encodeToJsonElement(result.toDto())
        }
        "details" -> {
            val update = reg.sources.of(p).getMangaUpdate(mangaOf(p), emptyList(), fetchDetails = true, fetchChapters = false)
            json.encodeToJsonElement(update.manga.toDto())
        }
        "chapters" -> {
            val update = reg.sources.of(p).getMangaUpdate(mangaOf(p), emptyList(), fetchDetails = false, fetchChapters = true)
            json.encodeToJsonElement(update.chapters.map { it.toDto() })
        }
        "pages" -> {
            val source = reg.sources.of(p)
            val chapter = SChapter.create().apply { url = p.str("url"); name = "" }
            val pages = source.getPageList(chapter).map { page ->
                PageDto(page.index, page.url, page.imageUrl)
            }
            json.encodeToJsonElement(pages)
        }
        "imageUrl" -> {
            // For pages whose image URL is resolved lazily from the page URL.
            val source = reg.sources.of(p) as? HttpSource ?: error("Source can't resolve image URLs")
            JsonPrimitive(source.getImageUrl(Page(0, p.str("pageUrl"))))
        }
        "image" -> {
            // Fetches an image (cover or page) with the source's own client and headers and
            // writes it to `out`, so bytes never go through this JSON pipe.
            val source = reg.sources.of(p)
            val outFile = File(p.str("out"))
            outFile.parentFile?.mkdirs()
            val response = if (source is HttpSource) {
                source.getImage(Page(0, imageUrl = p.str("url")))
            } else {
                Injekt.get<NetworkHelper>().client.newCall(okhttp3.Request.Builder().url(p.str("url")).build()).execute()
            }
            response.use { r ->
                check(r.isSuccessful) { "HTTP ${r.code} for ${p.str("url")}" }
                val tmp = File(outFile.path + ".part")
                r.body.byteStream().use { input -> tmp.outputStream().use { input.copyTo(it) } }
                outFile.delete()
                check(tmp.renameTo(outFile)) { "Couldn't write $outFile" }
                json.encodeToJsonElement(ImageDto(outFile.path, r.header("Content-Type")))
            }
        }
        else -> error("Unknown method \"$method\"")
    }
