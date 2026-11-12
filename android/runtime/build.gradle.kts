import groovy.json.JsonSlurper
import org.gradle.api.tasks.compile.JavaCompile

plugins {
    id("com.android.library")
    id("org.jetbrains.kotlin.android")
    id("org.jetbrains.kotlin.plugin.compose")
}

val repositoryRoot = projectDir.resolve("../..").canonicalFile
val pinnedMain = repositoryRoot.resolve(".android-runtime.local/DroidDeck/app/src/main")
val preparedRuntime = repositoryRoot.resolve(".android-runtime.local/prepared")
val hqBrandAssets = layout.buildDirectory.dir("generated/hq-brand-assets")
val prepareHqBrandAssets by tasks.registering(Copy::class) {
    from(repositoryRoot.resolve("public/icon.svg"))
    into(hqBrandAssets.map { it.dir("hq") })
}
val runtimeLock = JsonSlurper().parse(file("runtime.lock.json")) as Map<*, *>
val sourcePin = runtimeLock["source"] as Map<*, *>
val desktopPin = runtimeLock["desktop"] as Map<*, *>

android {
    namespace = "com.droiddeck.launcher"
    compileSdk = 36
    defaultConfig {
        minSdk = 26
        ndk { abiFilters += "arm64-v8a" }
        consumerProguardFiles("consumer-rules.pro")
        buildConfigField("String", "APPLICATION_ID", "\"asta.hq_launcher\"")
        buildConfigField("int", "VERSION_CODE", sourcePin["version_code"].toString())
        buildConfigField("String", "VERSION_NAME", "\"${sourcePin["version_name"]}\"")
        buildConfigField("String", "SOURCE_COMMIT", "\"${sourcePin["commit"]}\"")
        buildConfigField("String", "BUILD_LABEL", "\"HQ embedded runtime · ${sourcePin["commit"].toString().take(12)}\"")
        buildConfigField("long", "COMMIT_TIME", "${sourcePin["commit_time_ms"]}L")
        buildConfigField("int", "PR_NUMBER", "0")
        buildConfigField("boolean", "CI_BUILD", "false")
        // HQ owns updates and user-provided artwork credentials.
        buildConfigField("String", "RELEASE_SIGNER", "\"\"")
        buildConfigField("String", "SGDB_API_KEY", "\"\"")
        buildConfigField("String", "VARIANT_SUFFIXES", "\"asta.hq_launcher=\"")
        buildConfigField("String", "DESKTOP_KDE_TAG", "\"${desktopPin["tag"]}\"")
        buildConfigField("String", "DESKTOP_KDE_URL", "\"${desktopPin["url"]}\"")
        buildConfigField("String", "DESKTOP_KDE_SHA256", "\"${desktopPin["sha256"]}\"")
        buildConfigField("long", "DESKTOP_KDE_SIZE", "${desktopPin["size"]}L")
    }
    sourceSets.getByName("main") {
        // Keep HQ's bridge next to the unchanged, separately pinned GPL source.
        java.setSrcDirs(listOf("src/main/java", preparedRuntime.resolve("java")))
        res.setSrcDirs(listOf(pinnedMain.resolve("res")))
        assets.setSrcDirs(listOf("src/main/assets", preparedRuntime.resolve("assets"), hqBrandAssets))
        jniLibs.setSrcDirs(listOf(preparedRuntime.resolve("jniLibs")))
        manifest.srcFile(preparedRuntime.resolve("AndroidManifest.xml"))
    }
    buildFeatures {
        compose = true
        buildConfig = true
    }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    kotlinOptions { jvmTarget = "17" }
    packaging {
        resources.excludes += "META-INF/versions/9/OSGI-INF/MANIFEST.MF"
        jniLibs.useLegacyPackaging = true
        // Upstream deliberately replaces terminal-view's PTY implementation.
        // All other AAR-owned JNI is omitted from our donor source set.
        jniLibs.pickFirsts += "**/libtermux.so"
    }
}

val verifyHqRuntime by tasks.registering(Exec::class) {
    group = "verification"
    description = "Verify the pinned source and prepared local runtime before packaging."
    workingDir(repositoryRoot)
    commandLine(providers.environmentVariable("PYTHON").getOrElse("python"),
        repositoryRoot.resolve("scripts/prepare-android-runtime.py"), "--verify-only")
}
tasks.named("preBuild").configure { dependsOn(verifyHqRuntime, prepareHqBrandAssets) }
tasks.withType<JavaCompile>().configureEach { options.encoding = "UTF-8" }

dependencies {
    implementation("org.opencv:opencv:4.13.0")
    implementation("org.khronos.openxr:openxr_loader_for_android:1.1.49")
    implementation("com.github.luben:zstd-jni:1.5.7-4@aar")
    implementation("org.apache.commons:commons-compress:1.27.1")
    implementation("org.tukaani:xz:1.10")
    implementation(platform("androidx.compose:compose-bom:2024.02.00"))
    implementation("androidx.compose.ui:ui")
    implementation("androidx.compose.material3:material3")
    implementation("androidx.compose.ui:ui-tooling-preview")
    debugImplementation("androidx.compose.ui:ui-tooling")
    implementation("androidx.activity:activity-compose:1.8.2")
    implementation("androidx.compose.material:material-icons-extended")
    implementation("io.coil-kt:coil-compose:2.6.0")
    implementation("com.caverock:androidsvg-aar:1.4")
    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-android:1.7.3")
    implementation("com.flyfishxu:kadb-android:1.2.1")
    implementation("com.termux.termux-app:terminal-view:0.118.0")
    implementation("androidx.camera:camera-camera2:1.4.2")
    implementation("androidx.camera:camera-lifecycle:1.4.2")
    implementation("com.google.mediapipe:tasks-vision:1.0.0") {
        exclude(group = "com.google.auto.value", module = "auto-value")
    }
    testImplementation("junit:junit:4.13.2")
}
