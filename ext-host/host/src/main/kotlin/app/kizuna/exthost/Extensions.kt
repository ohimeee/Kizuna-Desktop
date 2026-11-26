package app.kizuna.exthost

/*
 * Extension install/load for Kizuna's extension host. The APK handling (manifest metadata,
 * dex2jar, asset copying, class loading) is adapted from Suwayomi-Server's PackageTools.kt and
 * Extension.kt, which are licensed under MPL-2.0: https://mozilla.org/MPL/2.0/
 */

import android.content.pm.PackageInfo
import android.content.pm.Signature
import android.os.Bundle
import com.googlecode.d2j.dex.Dex2jar
import com.googlecode.d2j.reader.MultiDexFileReader
import com.googlecode.dex2jar.tools.BaksmaliBaseDexExceptionHandler
import eu.kanade.tachiyomi.source.Source
import eu.kanade.tachiyomi.source.SourceFactory
import eu.kanade.tachiyomi.source.online.HttpSource
import eu.kanade.tachiyomi.util.lang.Hash
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json
import net.dongliu.apk.parser.ApkFile
import net.dongliu.apk.parser.ApkParsers
import org.w3c.dom.Element
import org.w3c.dom.Node
import suwayomi.tachidesk.manga.impl.util.BytecodeEditor
import suwayomi.tachidesk.manga.impl.util.ChildFirstURLClassLoader
import xyz.nulldev.androidcompat.pm.InstalledPackage.Companion.toList
import xyz.nulldev.androidcompat.pm.toPackageInfo
import java.io.File
import java.net.URL
import java.net.URLClassLoader
import java.util.concurrent.ConcurrentHashMap
import java.util.zip.ZipEntry
import java.util.zip.ZipInputStream
import java.util.zip.ZipOutputStream
import javax.xml.parsers.DocumentBuilderFactory

private const val EXTENSION_FEATURE = "tachiyomi.extension"
private const val METADATA_SOURCE_CLASS = "tachiyomi.extension.class"
private const val METADATA_NSFW = "tachiyomi.extension.nsfw"
private const val METADATA_CONTENT_WARNING = "tachiyomix.contentWarning"
private const val LIB_VERSION_MIN = 1.3
private const val LIB_VERSION_MAX = 1.6

@Serializable
data class ExtensionInfo(
    val pkgName: String,
    val name: String,
    val versionName: String,
    val versionCode: Int,
    val lang: String,
    val nsfw: Boolean,
    val signatureHash: String?,
    val sources: List<SourceInfo>,
)

@Serializable
data class SourceInfo(
    /** Sent as a string: source ids are 64-bit and lose precision as JS numbers. */
    val id: String,
    val name: String,
    val lang: String,
    val supportsLatest: Boolean,
    val baseUrl: String?,
    val pkgName: String,
)

/** Everything the host has loaded. Each extension lives in `<root>/<pkg>.jar` + `<pkg>.json`. */
class ExtensionRegistry(private val root: File) {
    private val json = Json { ignoreUnknownKeys = true }
    private val loaders = ConcurrentHashMap<String, URLClassLoader>()
    val extensions = ConcurrentHashMap<String, ExtensionInfo>()
    val sources = ConcurrentHashMap<Long, Source>()

    init {
        root.mkdirs()
    }

    /** Loads every installed extension. Failures are reported, not fatal. */
    fun loadAll(): List<String> {
        val errors = mutableListOf<String>()
        root.listFiles { f -> f.extension == "json" }?.forEach { meta ->
            val jar = File(root, meta.nameWithoutExtension + ".jar")
            runCatching {
                val stored = json.decodeFromString<StoredMeta>(meta.readText())
                load(jar, stored)
            }.onFailure { errors += "${meta.nameWithoutExtension}: ${it.message}" }
        }
        return errors
    }

    /** Installs (or updates) an extension from an APK file. The APK is not kept. */
    fun install(apk: File): ExtensionInfo {
        val info = packageInfo(apk)
        val pkg = info.packageName
        val features = info.reqFeatures.orEmpty().map { it.name }
        require(EXTENSION_FEATURE in features) { "This file is not a Tachiyomi/Mihon extension" }
        val libVersion = info.versionName.substringBeforeLast('.').toDouble()
        require(libVersion in LIB_VERSION_MIN..LIB_VERSION_MAX) {
            "Extension lib $libVersion is not supported (only $LIB_VERSION_MIN to $LIB_VERSION_MAX)"
        }
        val meta = info.applicationInfo.metaData
        val sourceClass = meta.getString(METADATA_SOURCE_CLASS)!!.trim()
        val nsfw = (meta.getString(METADATA_CONTENT_WARNING) ?: meta.getString(METADATA_NSFW))?.toIntOrNull() == 1
        val stored = StoredMeta(
            pkgName = pkg,
            label = info.applicationInfo.nonLocalizedLabel?.toString() ?: pkg,
            versionName = info.versionName,
            versionCode = info.versionCode,
            nsfw = nsfw,
            signatureHash = signatureHash(info),
            classNames = sourceClass.split(";").map { it.trim() }.filter { it.isNotEmpty() }
                .map { if (it.startsWith(".")) pkg + it else it },
        )

        unload(pkg)
        val jar = File(root, "$pkg.jar")
        val tmp = File(root, "$pkg.jar.part")
        tmp.delete()
        dex2jar(apk, tmp)
        copyAssets(apk, tmp)
        jar.delete()
        check(tmp.renameTo(jar)) { "Couldn't write $jar" }
        val ext = load(jar, stored)
        File(root, "$pkg.json").writeText(json.encodeToString(StoredMeta.serializer(), stored))
        return ext
    }

    fun uninstall(pkg: String) {
        unload(pkg)
        File(root, "$pkg.jar").delete()
        File(root, "$pkg.json").delete()
    }

    private fun unload(pkg: String) {
        extensions.remove(pkg)?.sources?.forEach { sources.remove(it.id.toLong()) }
        loaders.remove(pkg)?.close()
    }

    private fun load(jar: File, stored: StoredMeta): ExtensionInfo {
        val loader = ChildFirstURLClassLoader(arrayOf<URL>(jar.toURI().toURL()))
        val loaded = stored.classNames.flatMap { className ->
            when (val instance = Class.forName(className, false, loader).getDeclaredConstructor().newInstance()) {
                is Source -> listOf(instance)
                is SourceFactory -> instance.createSources()
                else -> error("Unknown source class type ${instance.javaClass}")
            }
        }
        loaders[stored.pkgName] = loader
        val infos = loaded.map { s ->
            sources[s.id] = s
            SourceInfo(
                id = s.id.toString(),
                name = s.name,
                lang = s.lang,
                supportsLatest = s.supportsLatest,
                baseUrl = (s as? HttpSource)?.baseUrl,
                pkgName = stored.pkgName,
            )
        }
        val langs = infos.map { it.lang }.toSet()
        val ext = ExtensionInfo(
            pkgName = stored.pkgName,
            name = stored.label.removePrefix("Tachiyomi: "),
            versionName = stored.versionName,
            versionCode = stored.versionCode,
            lang = if (langs.size == 1) langs.first() else "all",
            nsfw = stored.nsfw,
            signatureHash = stored.signatureHash,
            sources = infos,
        )
        extensions[stored.pkgName] = ext
        return ext
    }

    @Serializable
    private data class StoredMeta(
        val pkgName: String,
        val label: String,
        val versionName: String,
        val versionCode: Int,
        val nsfw: Boolean,
        val signatureHash: String?,
        val classNames: List<String>,
    )
}

// ---- APK helpers (from Suwayomi's PackageTools / Extension) ------------------------------

private fun packageInfo(apk: File): PackageInfo =
    ApkParsers.getMetaInfo(apk).toPackageInfo(apk).apply {
        ApkFile(apk).use { parsed ->
            val doc = DocumentBuilderFactory.newInstance().newDocumentBuilder()
                .parse(parsed.manifestXml.byteInputStream())
            applicationInfo.metaData = Bundle().apply {
                doc.getElementsByTagName("application").item(0)
                    ?.childNodes?.toList().orEmpty()
                    .filter { it.nodeType == Node.ELEMENT_NODE }
                    .map { it as Element }
                    .filter { it.tagName == "meta-data" }
                    .forEach {
                        putString(
                            it.attributes.getNamedItem("android:name").nodeValue,
                            it.attributes.getNamedItem("android:value")?.nodeValue,
                        )
                    }
            }
            signatures = parsed.apkSingers.flatMap { it.certificateMetas }.map { Signature(it.data) }.toTypedArray()
        }
    }

private fun signatureHash(info: PackageInfo): String? =
    info.signatures?.firstOrNull()?.let { Hash.sha256(it.toByteArray()) }

private fun dex2jar(apk: File, jar: File) {
    val reader = MultiDexFileReader.open(apk.readBytes())
    val handler = BaksmaliBaseDexExceptionHandler()
    Dex2jar.from(reader)
        .withExceptionHandler(handler)
        .reUseReg(false)
        .topoLogicalSort()
        .skipDebug(true)
        .optimizeSynchronized(false)
        .printIR(false)
        .noCode(false)
        .skipExceptions(false)
        .dontSanitizeNames(true)
        .computeFrames(true)
        .to(jar.toPath())
    check(!handler.hasException()) { "dex2jar failed to convert the extension" }
    BytecodeEditor.fixAndroidClasses(jar.toPath())
}

/** Some extensions read bundled assets; copy the APK's `assets/` entries into the jar. */
private fun copyAssets(apk: File, jar: File) {
    val assets = mutableMapOf<String, ByteArray>()
    ZipInputStream(apk.inputStream()).use { zip ->
        generateSequence { zip.nextEntry }.forEach { e ->
            if (!e.isDirectory && e.name.startsWith("assets/")) assets[e.name] = zip.readBytes()
        }
    }
    if (assets.isEmpty()) return
    val out = File(jar.path + ".tmp")
    ZipInputStream(jar.inputStream()).use { zin ->
        ZipOutputStream(out.outputStream()).use { zout ->
            generateSequence { zin.nextEntry }.forEach { e ->
                if (!e.name.startsWith("META-INF/")) {
                    zout.putNextEntry(ZipEntry(e.name))
                    zin.copyTo(zout)
                    zout.closeEntry()
                }
            }
            assets.forEach { (name, bytes) ->
                zout.putNextEntry(ZipEntry(name)) // keeps "assets/..." like Suwayomi does
                zout.write(bytes)
                zout.closeEntry()
            }
        }
    }
    jar.delete()
    check(out.renameTo(jar)) { "Couldn't write $jar" }
}
