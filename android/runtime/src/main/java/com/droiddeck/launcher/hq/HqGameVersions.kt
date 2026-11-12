package com.droiddeck.launcher.hq

import android.app.Activity
import android.content.Context
import android.util.AtomicFile
import com.droiddeck.launcher.agent.AgentGuest
import com.droiddeck.launcher.runtime.LinuxRuntime
import com.droiddeck.launcher.session.SessionPrefs
import com.droiddeck.launcher.session.SessionService
import com.droiddeck.launcher.session.SessionState
import org.json.JSONArray
import org.json.JSONObject
import java.io.File
import java.io.FileOutputStream
import java.nio.file.Files
import java.nio.file.Path
import java.nio.file.SimpleFileVisitor
import java.nio.file.FileVisitResult
import java.nio.file.attribute.BasicFileAttributes
import java.util.UUID

/** HQ-owned historical depots. Steam keeps ownership, authentication and the normal game launch. */
object HqGameVersions {
    private const val GAME = "1966720"
    private const val DEPOT = "1966721"
    private const val WRAPPER = "/usr/local/bin/hq-lethal-launch.py"
    private const val SOURCE = "steam-download-depot"
    private const val STORE = "root/.local/share/hq-launcher"
    @Volatile private var installing = false
    @Volatile private var requestedVersion: Int? = null
    @Volatile private var stage = ""
    @Volatile private var installError: String? = null
    @Volatile private var cancelRequested = false

    fun status(context: Context): JSONObject = JSONObject()
        .put("installing", installing)
        .put("version", requestedVersion ?: JSONObject.NULL)
        .put("stage", stage)
        .put("error", installError ?: JSONObject.NULL)
        .put("cancel_requested", cancelRequested)

    private fun store(context: Context): File {
        val runtime = LinuxRuntime.rootDir(context).canonicalFile
        val expected = File(context.filesDir.canonicalFile, "linuxfs")
        check(runtime == expected) { "HQ's internal Linux environment is unavailable." }
        return checked(runtime, File(runtime, STORE))
    }

    private fun checked(base: File, target: File): File {
        val root = base.canonicalFile
        val absolute = target.absoluteFile.toPath().normalize()
        check(absolute.startsWith(base.absoluteFile.toPath().normalize())) { "HQ version path escapes internal storage." }
        var part = base.absoluteFile.toPath().normalize()
        check(!Files.isSymbolicLink(part)) { "Symbolic link in HQ version store." }
        for (element in part.relativize(absolute)) {
            part = part.resolve(element)
            check(!Files.isSymbolicLink(part)) { "Symbolic link in HQ version installation." }
        }
        val canonical = target.canonicalFile
        check(canonical.toPath().startsWith(root.toPath())) { "HQ version path escapes internal storage." }
        return canonical
    }

    private fun identity(version: Int, manifest: String): JSONObject {
        require(version in 1..100000 && manifest.matches(Regex("[0-9]{1,32}"))) { "Invalid HQ game version." }
        return JSONObject().put("format", 1).put("version", version).put("depot_manifest", manifest)
    }

    private fun readJson(base: File, file: File): JSONObject {
        val path = checked(base, file)
        check(path.isFile && path.length() <= 1024 * 1024) { "HQ version installation metadata is missing or too large." }
        return JSONObject(path.readText(Charsets.UTF_8))
    }

    private fun writeAtomic(base: File, file: File, bytes: ByteArray) {
        val target = checked(base, file)
        check(target.parentFile!!.isDirectory || target.parentFile!!.mkdirs()) { "Could not create HQ version storage." }
        val atomic = AtomicFile(target)
        val output = atomic.startWrite()
        try {
            output.write(bytes)
            atomic.finishWrite(output)
        } catch (error: Exception) {
            atomic.failWrite(output)
            throw error
        }
    }

    private fun writeJson(base: File, file: File, value: JSONObject) =
        writeAtomic(base, file, value.toString().toByteArray(Charsets.UTF_8))

    private fun validateGame(base: File, directory: File) {
        val game = checked(base, directory)
        check(game.isDirectory) { "The downloaded game folder is missing." }
        val exe = checked(base, File(game, "Lethal Company.exe"))
        val unity = checked(base, File(game, "UnityPlayer.dll"))
        val data = checked(base, File(game, "Lethal Company_Data"))
        check(exe.isFile && exe.length() > 0 && unity.isFile && unity.length() > 0 && data.isDirectory) {
            "Steam's depot is missing required Lethal Company game files."
        }
    }

    private fun installedEntry(context: Context, version: Int, manifest: String): JSONObject {
        identity(version, manifest)
        val base = store(context)
        val directory = checked(base, File(base, "versions/$version-$manifest"))
        val receiptFile = checked(base, File(directory, "install.json"))
        val receipt = readJson(base, receiptFile)
        check(receipt.opt("format") is Int && receipt.optInt("format") == 1 && receipt.opt("source") == SOURCE &&
            receipt.opt("app_id") == GAME && receipt.opt("depot_id") == DEPOT &&
            receipt.opt("version") is Int && receipt.optInt("version") == version && receipt.opt("depot_manifest") == manifest &&
            receipt.opt("complete") == true) { "The HQ game installation is incomplete or has changed." }
        val game = checked(base, File(directory, "game"))
        val compat = checked(base, File(directory, "compatdata/$GAME"))
        validateGame(base, game)
        return JSONObject().put("version", version).put("depot_manifest", manifest)
            .put("game_dir", game.path).put("receipt", receiptFile.path).put("compat_data", compat.path)
    }

    fun installed(context: Context): JSONArray {
        val base = store(context)
        val versions = checked(base, File(base, "versions"))
        val entries = versions.listFiles().orEmpty().mapNotNull { directory ->
            val match = Regex("([0-9]{1,6})-([0-9]{1,32})").matchEntire(directory.name) ?: return@mapNotNull null
            runCatching { installedEntry(context, match.groupValues[1].toInt(), match.groupValues[2]) }.getOrNull()
        }.sortedBy { it.getInt("version") }
        return JSONArray(entries)
    }

    /** Null means the original Steam installation; invalid selections fail rather than silently changing versions. */
    fun selected(context: Context): JSONObject? {
        val base = store(context)
        val selection = checked(base, File(base, "selection.json"))
        if (!selection.exists()) return null
        val value = readJson(base, selection)
        check(value.opt("format") is Int && value.optInt("format") == 1 && value.opt("version") is Int &&
            value.opt("depot_manifest") is String) { "Unsupported HQ game selection format." }
        return installedEntry(context, value.getInt("version"), value.getString("depot_manifest"))
    }

    /** The caller holds the shared operation lock and requires a fully stopped runtime. */
    @Synchronized
    fun select(context: Context, version: Int) {
        check(!installing) { "Wait for the game download to finish." }
        val base = store(context)
        val selection = checked(base, File(base, "selection.json"))
        if (version == 0) {
            check(!selection.exists() || selection.delete()) { "Could not select the Steam installation." }
            return
        }
        val entries = installed(context)
        val matching = (0 until entries.length()).map { entries.getJSONObject(it) }.filter { it.getInt("version") == version }
        check(matching.size == 1) { "Install this game version first. Multiple revisions must be resolved before selecting it." }
        val entry = matching.single()
        writeJson(base, selection, identity(version, entry.getString("depot_manifest")))
    }

    fun cancel() {
        if (installing) {
            cancelRequested = true
            stage = "Cancelling. Waiting for Steam's current depot operation to finish."
        }
    }

    /** Start only after openSteam; every potentially blocking operation runs on this worker. */
    @Synchronized
    fun startInstall(activity: Activity, version: Int, manifest: String) {
        identity(version, manifest)
        check(!installing) { "A game version is already downloading." }
        installing = true
        requestedVersion = version
        stage = "Waiting for Steam sign-in."
        installError = null
        cancelRequested = false
        val context = activity.applicationContext
        Thread({ install(context, version, manifest) }, "hq-version-install").start()
    }

    private fun cancelled() {
        check(!cancelRequested) { "Game version installation cancelled. Steam's depot files were left unchanged." }
    }

    private fun waitForSteam(context: Context) {
        val deadline = android.os.SystemClock.elapsedRealtime() + 180_000L
        while (android.os.SystemClock.elapsedRealtime() < deadline) {
            cancelled()
            check(!SessionState.stopRequested) { "Steam was stopped before the game download could start." }
            if (SessionState.running && SessionState.mode == SessionService.MODE_STEAM && AgentGuest.hello(context) != null) {
                // The guest agent starts before Steam's web client. A live
                // agent alone does not imply a CDP port, login or loaded licenses.
                val ready = runCatching { evaluate(context, """
                    (async () => {
                      if (typeof SteamClient === 'undefined' ||
                          typeof SteamClient.Console?.ExecCommand !== 'function' ||
                          typeof SteamClient.Console?.RegisterForSpewOutput !== 'function' ||
                          typeof SteamClient.Apps?.RegisterForAppDetails !== 'function' ||
                          typeof SteamClient.Apps?.GetIsSubscribedApp !== 'function') return false;
                      return await SteamClient.Apps.GetIsSubscribedApp($GAME) === true;
                    })()
                """.trimIndent()) == true }.getOrDefault(false)
                if (ready) return
            }
            Thread.sleep(500)
        }
        error("Steam is not ready. Sign in with an account that owns Lethal Company and retry.")
    }

    private fun evaluate(context: Context, expression: String): Any? {
        val result = AgentGuest.steamEval(context, expression, 20_000)
        check(result.optBoolean("ok")) {
            result.optJSONObject("error")?.optString("message") ?: "Steam did not accept the HQ request."
        }
        return result.opt("value")?.takeIf { it != JSONObject.NULL }
    }

    private fun launchOptions(context: Context): String {
        val expression = """
            new Promise((resolve, reject) => {
              let handle = null, finished = false;
              const timer = setTimeout(() => { try { handle?.unregister(); } catch (e) {} reject(new Error('Steam app details are not ready.')); }, 10000);
              const finish = value => {
                if (finished || typeof value?.strLaunchOptions !== 'string') return;
                finished = true; clearTimeout(timer);
                try { handle?.unregister(); } catch (e) {}
                resolve(value.strLaunchOptions);
              };
              handle = SteamClient.Apps.RegisterForAppDetails($GAME, finish);
              if (finished) { try { handle?.unregister(); } catch (e) {} }
            })
        """.trimIndent()
        return evaluate(context, expression) as? String ?: error("Steam did not return this game's launch options.")
    }

    /** Run on a worker after Steam/AgentGuest is ready, before accepting a managed installation. */
    fun configureLaunch(context: Context) {
        check(SessionState.running && SessionState.mode == SessionService.MODE_STEAM && !SessionState.stopRequested) {
            "Open Steam before configuring HQ game versions."
        }
        val runtime = LinuxRuntime.rootDir(context).canonicalFile
        val helper = checked(runtime, File(runtime, WRAPPER.removePrefix("/")))
        val bytes = context.assets.open("hq-lethal-launch.py").use { it.readBytes() }
        writeAtomic(runtime, helper, bytes)
        check(helper.setExecutable(true, true)) { "Could not prepare HQ's game launch helper." }
        val original = launchOptions(context)
        val occurrences = Regex(Regex.escape(WRAPPER)).findAll(original).count()
        if (occurrences == 1 && Regex("(^|\\s)${Regex.escape(WRAPPER)} %command%(?=\\s|$)").containsMatchIn(original)) return
        check(occurrences == 0) { "Steam's HQ launch options were changed. Restore the game's previous launch options before retrying." }
        val tokenCount = Regex("%command%").findAll(original).count()
        val managed = when (tokenCount) {
            0 -> "$WRAPPER %command%" + if (original.isEmpty()) "" else " $original"
            1 -> {
                check(Regex("(^|\\s)%command%(?=\\s|$)").containsMatchIn(original)) { "The existing Steam launch command cannot be safely wrapped." }
                original.replace("%command%", "$WRAPPER %command%")
            }
            else -> error("The existing Steam launch command contains multiple command substitutions.")
        }
        val base = store(context)
        writeJson(base, File(base, "launch-options.json"), JSONObject().put("format", 1)
            .put("original", original).put("managed", managed))
        evaluate(context, "(() => { SteamClient.Apps.SetAppLaunchOptions($GAME, ${JSONObject.quote(managed)}); return true; })()")
        repeat(12) {
            if (launchOptions(context) == managed) return
            Thread.sleep(250)
        }
        error("Steam did not save HQ's launch options. The game version was not activated.")
    }

    private fun beginDownload(context: Context, token: String, manifest: String) {
        val expression = """
            (() => {
              const key = ${JSONObject.quote(token)}, manifest = ${JSONObject.quote(manifest)};
              const jobs = globalThis.__hqDepotJobs || (globalThis.__hqDepotJobs = {});
              const job = { state: 'downloading', path: '', message: '', lines: [] };
              jobs[key] = job;
              job.subscription = SteamClient.Console.RegisterForSpewOutput((...args) => {
                const output = args[0], text = typeof output === 'string' ? output : output?.spew;
                if (typeof text !== 'string') return;
                const complete = text.match(/Depot download complete\s*:\s*"([^"]+)"\s*\([^)]*\bmanifest\s+([0-9]+)\)/i);
                if (complete && complete[2] === manifest && complete[1].replace(/\\/g, '/').includes('/app_$GAME/depot_$DEPOT')) {
                  job.state = 'complete'; job.path = complete[1]; job.message = 'Steam depot download complete.';
                } else if (/Depot download failed/i.test(text)) {
                  job.state = 'failed'; job.message = text.trim().slice(-2000);
                }
                if (/Downloading depot $DEPOT|Depot download (complete|failed)/i.test(text)) {
                  job.lines.push(text.trim().slice(-2000)); job.lines = job.lines.slice(-12);
                }
              });
              SteamClient.Console.ExecCommand('download_depot $GAME $DEPOT ' + manifest);
              return true;
            })()
        """.trimIndent()
        check(evaluate(context, expression) == true) { "Steam did not accept the depot download command." }
    }

    private fun downloadState(context: Context, token: String): JSONObject {
        val result = evaluate(context, """
            (() => {
              const job = globalThis.__hqDepotJobs?.[${JSONObject.quote(token)}];
              return job ? { state: job.state, path: job.path, message: job.message, lines: job.lines } : null;
            })()
        """.trimIndent())
        return result as? JSONObject ?: error("Steam restarted during the depot download. Retry after Steam is ready.")
    }

    private fun endDownload(context: Context, token: String) {
        runCatching { evaluate(context, """
            (() => {
              const key = ${JSONObject.quote(token)}, job = globalThis.__hqDepotJobs?.[key];
              try { job?.subscription?.unregister(); } catch (e) {}
              if (globalThis.__hqDepotJobs) delete globalThis.__hqDepotJobs[key];
              return true;
            })()
        """.trimIndent()) }
    }

    private fun sourceDirectory(context: Context, path: String): File {
        val normalized = path.replace('\\', '/').trimEnd('/')
        check(normalized.startsWith("/") && normalized.split('/').none { it == ".." || it == "." } &&
            normalized.endsWith("/steamapps/content/app_$GAME/depot_$DEPOT")) { "Steam returned an unexpected depot download path." }
        val runtime = LinuxRuntime.rootDir(context).canonicalFile
        val source = checked(runtime, File(runtime, normalized.removePrefix("/")))
        val steam = checked(runtime, SessionPrefs.activeSteamRoot(context))
        check(source.toPath().startsWith(steam.toPath())) { "Steam's depot download is outside HQ's active internal Steam library." }
        validateGame(runtime, source)
        return source
    }

    private fun copyTree(source: File, destination: File) {
        var count = 0
        fun copy(from: File, to: File, depth: Int) {
            cancelled()
            check(depth < 128 && ++count <= 100000) { "Steam's depot contains too many nested files." }
            check(!Files.isSymbolicLink(from.toPath())) { "Symbolic link in Steam depot game files." }
            if (from.isDirectory) {
                check(to.mkdir()) { "Could not create the installed game folder. Check free storage." }
                val children = from.listFiles() ?: error("Could not read Steam's depot game files.")
                for (child in children) copy(child, File(to, child.name), depth + 1)
            } else {
                check(from.isFile) { "Unsupported file in Steam's depot game files." }
                from.inputStream().use { input -> FileOutputStream(to).use { output -> input.copyTo(output); output.fd.sync() } }
            }
        }
        copy(source, destination, 0)
    }

    private fun removeStaging(base: File, directory: File) {
        val root = checked(base, directory)
        check(root.parentFile == checked(base, File(base, "versions")) && root.name.startsWith(".install-"))
        Files.walkFileTree(root.toPath(), object : SimpleFileVisitor<Path>() {
            override fun visitFile(file: Path, attributes: BasicFileAttributes): FileVisitResult {
                Files.delete(file)
                return FileVisitResult.CONTINUE
            }
            override fun postVisitDirectory(directory: Path, error: java.io.IOException?): FileVisitResult {
                if (error != null) throw error
                Files.delete(directory)
                return FileVisitResult.CONTINUE
            }
        })
    }

    private fun install(context: Context, version: Int, manifest: String) {
        val token = UUID.randomUUID().toString()
        var staging: File? = null
        var downloadStarted = false
        try {
            waitForSteam(context)
            val base = store(context)
            if (runCatching { installedEntry(context, version, manifest) }.isSuccess) {
                configureLaunch(context)
                stage = "Game version already installed."
                return
            }
            cancelled()
            val session = SessionState.sessionId
            stage = "Steam is downloading game v$version."
            downloadStarted = true
            beginDownload(context, token, manifest)
            var result: JSONObject
            while (true) {
                check(SessionState.running && !SessionState.stopRequested && SessionState.sessionId == session) {
                    "Steam stopped during the download. No game version was installed."
                }
                result = downloadState(context, token)
                if (result.optString("state") == "failed") error(result.optString("message", "Steam could not download this depot manifest."))
                if (result.optString("state") == "complete") break
                if (!cancelRequested) {
                    val lines = result.optJSONArray("lines")
                    stage = if (lines != null && lines.length() > 0) lines.optString(lines.length() - 1).take(500)
                        else "Steam is downloading game v$version."
                }
                Thread.sleep(1000)
            }
            cancelled()
            val source = sourceDirectory(context, result.getString("path"))
            val versions = checked(base, File(base, "versions"))
            check(versions.isDirectory || versions.mkdirs()) { "Could not create the game version store." }
            val destination = checked(base, File(versions, "$version-$manifest"))
            check(!destination.exists()) { "This version's installation folder already exists but is incomplete. It was left unchanged." }
            val temporary = checked(base, File(versions, ".install-$token"))
            check(temporary.mkdir()) { "Could not create temporary game version storage." }
            staging = temporary
            stage = "Copying Steam's game files into the separate version folder."
            copyTree(source, File(temporary, "game"))
            validateGame(base, File(temporary, "game"))
            check(File(temporary, "compatdata/$GAME").mkdirs()) { "Could not create the version's Proton prefix." }
            cancelled()
            stage = "Preparing authenticated Steam launch."
            configureLaunch(context)
            cancelled()
            val receipt = identity(version, manifest).put("source", SOURCE).put("app_id", GAME).put("depot_id", DEPOT)
                .put("complete", true).put("installed_at", System.currentTimeMillis())
            writeJson(base, File(temporary, "install.json"), receipt)
            check(temporary.renameTo(destination)) { "Could not publish the completed game version. Check free storage." }
            staging = null
            stage = "Game v$version installed. Stop Steam to select this version."
        } catch (error: Exception) {
            installError = error.message ?: "Could not install the game version."
            stage = if (cancelRequested) "Game version installation cancelled." else "Game version installation failed."
            android.util.Log.w("HqGameVersions", "Version installation did not complete: ${error.message}")
        } finally {
            if (downloadStarted && SessionState.running && !SessionState.stopRequested) endDownload(context, token)
            staging?.let { directory -> runCatching { removeStaging(store(context), directory) } }
            installing = false
        }
    }
}
