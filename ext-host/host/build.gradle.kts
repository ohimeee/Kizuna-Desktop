plugins {
    id(libs.plugins.kotlin.jvm.get().pluginId)
    id(libs.plugins.kotlin.serialization.get().pluginId)
    application
}

application {
    mainClass.set("app.kizuna.exthost.MainKt")
    applicationName = "kizuna-ext-host"
}

dependencies {
    implementation(libs.bundles.shared)
    implementation(libs.bundles.okhttp)
    implementation(libs.okio)
    implementation(libs.android.stubs)
    implementation(libs.injekt)
    implementation(libs.serialization.xml.core)
    implementation(libs.serialization.xml)
    implementation(libs.sort)
    implementation(libs.asm)
    implementation(libs.cache4k)
    implementation(projects.androidCompat)
    implementation(projects.androidCompat.config)
    compileOnly(libs.jcef)
}

// One jar with everything, for jlink'd runtime packaging.
tasks.register<Jar>("fatJar") {
    archiveFileName.set("kizuna-ext-host.jar")
    duplicatesStrategy = DuplicatesStrategy.EXCLUDE
    manifest { attributes["Main-Class"] = "app.kizuna.exthost.MainKt" }
    from(sourceSets.main.get().output)
    dependsOn(configurations.runtimeClasspath)
    from({ configurations.runtimeClasspath.get().filter { it.name.endsWith(".jar") }.map { zipTree(it) } }) {
        exclude("META-INF/*.SF", "META-INF/*.DSA", "META-INF/*.RSA", "META-INF/versions/9/module-info.class", "module-info.class")
        // Windows-only app: drop other platforms' native libraries.
        exclude("native/Linux/**", "native/Mac/**")
        exclude("com/sun/jna/linux-*/**", "com/sun/jna/darwin*/**", "com/sun/jna/aix-*/**", "com/sun/jna/freebsd-*/**",
            "com/sun/jna/openbsd-*/**", "com/sun/jna/sunos-*/**", "com/sun/jna/dragonflybsd-*/**", "com/sun/jna/win32-aarch64/**")
        exclude("native/Windows/x86/**", "com/sun/jna/win32-x86/**", "jni/aarch64/**", "jni/x86_64/**", "jni/amd64/*.so")
        // Android framework UI resources from the android-jar stubs: extensions never read them.
        exclude("res/**", "resources.arsc")
        // Fonts Typeface.java never loads (it uses the upright Noto variable fonts only).
        exclude("font/NotoSans/NotoColorEmoji-Regular.ttf", "font/NotoSans/*Italic*")
    }
}
