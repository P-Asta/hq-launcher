package com.droiddeck.launcher.hq

import android.app.Activity
import android.app.Application
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Build
import android.os.Bundle
import android.provider.Settings
import com.droiddeck.launcher.SessionActivity
import com.droiddeck.launcher.agent.AgentGuest
import com.droiddeck.launcher.core.GameEnvironment
import com.droiddeck.launcher.core.PhantomProcessLimit
import com.droiddeck.launcher.frontend.Library
import com.droiddeck.launcher.runtime.LinuxRuntime
import com.droiddeck.launcher.runtime.LinuxRuntimeInstaller
import com.droiddeck.launcher.runtime.RuntimeInstallService
import com.droiddeck.launcher.session.GameEnvironmentStore
import com.droiddeck.launcher.session.SessionPrefs
import com.droiddeck.launcher.session.SessionService
import com.droiddeck.launcher.session.SessionState
import org.json.JSONObject
import org.json.JSONArray
import java.io.File

/** Fixed-game, app-private entry points. No credentials or external app data are imported. */
object HqRuntimeBridge {
    private const val GAME = "1966720"
    @Volatile private var installing = false
    @Volatile private var stage = ""
    @Volatile private var progress = -1
    @Volatile private var installError: String? = null
    @Volatile private var launchPending = false
    @Volatile private var focusRequest = 0
    @Volatile private var versionPending = false
    @Volatile private var versionError: String? = null

    fun initialize(app: Application) {
        val process = Application.getProcessName()
        if (process == "${app.packageName}:vr_recovery" || process == "${app.packageName}:rgb_diagnostic") return
        // HQ's English UI also applies to embedded runtime activities/services.
        com.droiddeck.launcher.core.AppLanguage.setChosen(app, "en")
        app.registerActivityLifecycleCallbacks(object : Application.ActivityLifecycleCallbacks {
            override fun onActivityResumed(activity: Activity) {
                if (activity is SessionActivity) launchPending = false
            }
            override fun onActivityDestroyed(activity: Activity) {
                if (activity is SessionActivity) launchPending = false
            }
            override fun onActivityCreated(activity: Activity, state: Bundle?) {}
            override fun onActivityStarted(activity: Activity) {}
            override fun onActivityPaused(activity: Activity) {}
            override fun onActivityStopped(activity: Activity) {}
            override fun onActivitySaveInstanceState(activity: Activity, state: Bundle) {}
        })
        com.droiddeck.launcher.session.CrashHandler.install(app)
        com.droiddeck.launcher.session.QuestExperimental.initialize(app)
        SessionPrefs.settleGpuDriverMode(app)
        com.droiddeck.launcher.core.DownloadSource.init(app)
        com.droiddeck.launcher.session.GpuClockPin.clearLeftover(app)
        com.droiddeck.launcher.glasses.Glasses.init(app)
        com.droiddeck.launcher.glasses.GlassesMenuMirror.initialize(app)
        com.droiddeck.launcher.runtime.BwrapSpawner.start(app)
        com.droiddeck.launcher.frontend.GameFileSync.start(app)
        com.droiddeck.launcher.stores.StoresState.init(app)
        com.droiddeck.launcher.stores.StoresState.register(com.droiddeck.launcher.stores.gog.GogBackend)
        com.droiddeck.launcher.stores.StoresState.register(com.droiddeck.launcher.stores.epic.EpicBackend)
        com.droiddeck.launcher.stores.StoresState.register(com.droiddeck.launcher.stores.amazon.AmazonBackend)
        // HQ owns application updates; upstream AppUpdates is deliberately not started.
    }

    fun status(context: Context): JSONObject = JSONObject().apply {
        val root = SessionPrefs.activeSteamRoot(context)
        put("runtime_mode", "internal")
        put("app_package", context.packageName)
        put("runtime_installed", LinuxRuntime.isInstalled(context))
        put("runtime_version", LinuxRuntimeInstaller.installedVersion(context) ?: JSONObject.NULL)
        put("installing", installing || LinuxRuntimeInstaller.isBusy())
        put("install_stage", stage)
        put("install_progress", progress)
        put("install_error", installError ?: JSONObject.NULL)
        put("version_installing", versionPending || HqGameVersions.status(context).optBoolean("installing"))
        put("version_install", HqGameVersions.status(context).apply {
            if (versionError != null) put("error", versionError)
        })
        put("steam_installed", File(root, "steam.sh").isFile || File(root, "ubuntu12_32/steam").isFile)
        put("steam_ui", SessionState.steamUi ?: JSONObject.NULL)
        val microphonePermission = context.checkSelfPermission(android.Manifest.permission.RECORD_AUDIO) == PackageManager.PERMISSION_GRANTED
        put("microphone_permission", microphonePermission)
        put("microphone_enabled", microphonePermission && SessionPrefs.micEnabled(context))
        put("game_installed", runCatching { gamePaths(context) }.isSuccess)
        // Upstream clears running before asynchronous guest teardown finishes.
        put("session_running", !sessionStopped(false))
        put("session_phase", SessionState.phase.name.lowercase())
        put("session_error", SessionState.failureMessage ?: JSONObject.NULL)
        put("phantom_blocked", PhantomProcessLimit.blocksSteam(PhantomProcessLimit.read(context)))
        put("device", Build.MODEL)
        put("android_version", Build.VERSION.RELEASE)
    }

    @Synchronized
    fun installRuntime(context: Context) {
        if (installing || LinuxRuntimeInstaller.isBusy()) return
        ensureStopped(context)
        if (LinuxRuntime.isInstalled(context)) return
        installing = true
        stage = "Checking Linux runtime download"
        progress = -1
        installError = null
        val app = context.applicationContext
        Thread({
            try {
                val release = LinuxRuntimeInstaller.fetchRelease()
                    ?: error("Could not fetch the Linux runtime. Check your internet connection.")
                RuntimeInstallService.start(app, release)
                val ok = LinuxRuntimeInstaller.install(app, release) { next, percent ->
                    stage = next
                    progress = percent
                }
                check(ok && LinuxRuntime.isInstalled(app)) { "Linux installation failed. Check storage and connection, then retry." }
                stage = "Linux runtime installed"
                progress = 100
            } catch (error: Exception) {
                installError = error.message ?: "Linux installation failed."
                android.util.Log.e("HqRuntime", "Runtime install failed", error)
            } finally {
                installing = false
            }
        }, "hq-runtime-install").start()
    }

    private fun requireReady(context: Context) {
        check(!installing && !LinuxRuntimeInstaller.isBusy()) { "Wait for Linux installation to finish." }
        check(LinuxRuntime.isInstalled(context)) { "Install the Linux runtime first." }
        check(!PhantomProcessLimit.blocksSteam(PhantomProcessLimit.read(context))) {
            "Enable Disable child process restrictions in Android developer options, then return."
        }
        check(!SessionState.stopRequested) { "Wait for the Steam session to stop." }
        if (!SessionState.running) check(sessionStopped(false)) { "Wait for the previous Steam session to stop completely." }
    }

    fun openSteam(activity: Activity) {
        requireReady(activity)
        ++focusRequest
        if (SessionState.running) {
            check(SessionState.mode == SessionService.MODE_STEAM) { "Stop the current session first." }
            check(SessionState.steamUi == "desktop") { "Stop the runtime, then select Install Lethal Company again." }
            resume(activity, installingGame = true)
        } else {
            startSession(activity, Intent(activity, SessionActivity::class.java)
                .putExtra(SessionService.EXTRA_MODE, SessionService.MODE_STEAM)
                .putExtra(SessionService.EXTRA_STEAM_UI, "desktop")
                .putExtra("hqInstallSession", true)
                .putExtra(SessionService.EXTRA_STEAM_URL, "steam://install/$GAME")
                .putExtra(SessionActivity.EXTRA_RETURN_HOME, false))
        }
    }

    fun cacheGameVersion(context: Context, depotManifest: String, version: Int) {
        require(depotManifest.matches(Regex("[0-9]{1,32}")) && version in 1..100000) { "Could not verify the game version." }
        context.getSharedPreferences("hq-game-controls", Context.MODE_PRIVATE).edit().putInt("depot-$depotManifest", version).apply()
    }

    fun launchGame(activity: Activity, depotManifest: String = "") {
        requireReady(activity)
        gamePaths(activity)
        val version = if (depotManifest.matches(Regex("[0-9]{1,32}")))
            activity.getSharedPreferences("hq-game-controls", Context.MODE_PRIVATE).getInt("depot-$depotManifest", -1) else -1
        if (SessionState.running) {
            check(SessionState.steamUi == "desktop") { "Stop the runtime, then select HQ Run again." }
            check(SessionState.steamUrl == "steam://rungameid/$GAME") { "Stop the installer session, then select HQ Run." }
            check(SessionService.launchGame(activity, GAME)) { "Wait for Steam to finish starting, then retry." }
            resume(activity, installingGame = false, gameVersion = version)
        } else {
            startSession(activity, Intent(activity, SessionActivity::class.java)
                .putExtra(SessionService.EXTRA_MODE, SessionService.MODE_STEAM)
                .putExtra(SessionService.EXTRA_STEAM_UI, "desktop")
                .putExtra(SessionService.EXTRA_STEAM_URL, "steam://rungameid/$GAME")
                .putExtra("hqInstallSession", false)
                .putExtra("hqGameVersion", version)
                .putExtra(SessionActivity.EXTRA_RETURN_HOME, false))
        }
        focusGameWhenReady(activity.applicationContext)
    }

    private fun focusGameWhenReady(context: Context) {
        val request = ++focusRequest
        Thread({
            val deadline = android.os.SystemClock.elapsedRealtime() + 180_000
            var sessionId: String? = null
            var started = false
            var steamFocused = false
            while (request == focusRequest && android.os.SystemClock.elapsedRealtime() < deadline) {
                if (SessionState.stopRequested) return@Thread
                if (SessionState.running) {
                    started = true
                    val current = SessionState.sessionId
                    if (sessionId != null && current != sessionId) return@Thread
                    if (current != null) sessionId = current
                    val snapshot = AgentGuest.focus(context)
                    val apps = snapshot?.optJSONArray("focusableApps")
                    val target = when {
                        apps == null -> null
                        (0 until apps.length()).any { apps.optLong(it) == GAME.toLong() } -> GAME
                        !steamFocused && snapshot?.isNull("focusedApp") == true &&
                            (0 until apps.length()).any { apps.optLong(it) == 769L } -> "769"
                        else -> null
                    }
                    if (target != null) {
                        try {
                            if (request != focusRequest) return@Thread
                            val result = AgentGuest.call(context, JSONObject().put("kind", "exec")
                                .put("argv", JSONArray(listOf("xprop", "-root", "-f",
                                    "GAMESCOPECTRL_BASELAYER_APPID", "32c", "-set",
                                    "GAMESCOPECTRL_BASELAYER_APPID", target)))
                                .put("timeout", 5), 7_000, trusted = true)
                            if (result.optBoolean("ok") && result.optInt("exitCode", -1) == 0) {
                                if (target == GAME) {
                                    android.util.Log.i("HqRuntime", "Lethal Company window selected")
                                    return@Thread
                                }
                                steamFocused = true
                            }
                        } catch (_: Exception) { /* Guest startup can race its first focus snapshot. */ }
                    }
                } else if (started) return@Thread
                Thread.sleep(500)
            }
            if (request == focusRequest) android.util.Log.w("HqRuntime", "Lethal Company window was not ready for focus")
        }, "hq-game-focus").start()
    }

    private fun resume(activity: Activity, installingGame: Boolean, gameVersion: Int = -1) = activity.startActivity(
        Intent(activity, SessionActivity::class.java).setAction(SessionService.ACTION_RESUME)
            .putExtra("hqInstallSession", installingGame)
            .putExtra("hqGameVersion", gameVersion)
            .putExtra(SessionActivity.EXTRA_RETURN_HOME, false))

    private fun startSession(activity: Activity, intent: Intent) {
        launchPending = true
        try {
            activity.startActivity(intent)
        } catch (error: Exception) {
            launchPending = false
            throw error
        }
    }

    fun stopSession(context: Context) {
        ++focusRequest
        HqGameVersions.cancel()
        if (!sessionStopped(false) && SessionState.phase.name != "STOPPING") SessionService.stop(context)
    }

    private fun sessionStopped(runtimeBusy: Boolean) = RuntimeRules.sessionStopped(
        SessionState.running, SessionState.phase.name.lowercase(), SessionState.stopRequested, runtimeBusy, launchPending)

    fun ensureStopped(context: Context) {
        check(sessionStopped(installing || versionPending || HqGameVersions.status(context).optBoolean("installing") || LinuxRuntimeInstaller.isBusy())) {
            "Stop Steam and wait for installation to finish before changing mods."
        }
    }

    private fun steamGamePaths(context: Context): JSONObject {
        val root = LinuxRuntime.rootDir(context).canonicalFile
        val game = Library.steamGames(context).firstOrNull { it.appId.toString() == GAME }
            ?.gameFiles ?: error("Finish installing Lethal Company in HQ internal Steam.")
        val directory = RuntimeRules.internalDirectory(context.filesDir, root, game)
        check(File(directory, "Lethal Company.exe").isFile) { "Game installation is incomplete." }
        val steamApps = requireNotNull(directory.parentFile?.parentFile) { "Could not verify the Steam game directory." }
        val manifest = File(steamApps, "appmanifest_$GAME.acf").canonicalFile
        check(manifest.isFile && manifest.toPath().startsWith(root.toPath())) { "Internal Steam installation information was not found." }
        return JSONObject().put("private_root", context.filesDir.canonicalPath)
            .put("runtime_root", root.path).put("game_dir", directory.path)
            .put("steam_manifest", manifest.path).put("source", "steam")
    }

    fun gamePaths(context: Context): JSONObject {
        val result = steamGamePaths(context)
        val selected = HqGameVersions.selected(context) ?: return result
        return result.apply {
            put("source", "managed")
            for (name in listOf("game_dir", "receipt", "version", "depot_manifest")) put(name, selected.get(name))
        }
    }

    fun versions(context: Context): JSONObject = JSONObject().apply {
        put("installed", HqGameVersions.installed(context))
        try { put("selected_version", HqGameVersions.selected(context)?.getInt("version") ?: 0) }
        catch (error: Exception) { put("selected_version", -1); put("selection_error", error.message) }
        put("install", HqGameVersions.status(context))
    }

    fun selectVersion(context: Context, version: Int, manifest: String) {
        ensureStopped(context)
        if (version != 0) {
            val entries = HqGameVersions.installed(context)
            check((0 until entries.length()).any { entries.getJSONObject(it).let { value ->
                value.optInt("version") == version && value.optString("depot_manifest") == manifest
            } }) { "Install this exact game version first." }
        }
        HqGameVersions.select(context, version)
    }

    fun installVersion(activity: Activity, version: Int, manifest: String) {
        ensureStopped(activity)
        steamGamePaths(activity)
        versionError = null
        openSteam(activity)
        HqGameVersions.startInstall(activity, version, manifest)
        versionPending = true
        val context = activity.applicationContext
        Thread({
            try {
                while (HqGameVersions.status(context).optBoolean("installing")) Thread.sleep(500)
                val result = HqGameVersions.status(context)
                if (!result.isNull("error")) return@Thread
                SessionService.stop(context)
                val deadline = android.os.SystemClock.elapsedRealtime() + 30_000
                while (!sessionStopped(false) && android.os.SystemClock.elapsedRealtime() < deadline) Thread.sleep(250)
                check(sessionStopped(false)) { "Stop the runtime before selecting the downloaded version." }
                HqGameVersions.select(context, version)
            } catch (error: Exception) { versionError = error.message }
            finally { versionPending = false }
        }, "hq-version-finish").start()
    }

    fun configureBepinex(context: Context) {
        ensureStopped(context)
        gamePaths(context)
        val config = GameEnvironmentStore.read(context)
        val overrides = config.games[GAME]
        val previous = if (overrides?.containsKey("WINEDLLOVERRIDES") == true)
            overrides["WINEDLLOVERRIDES"] else config.shared["WINEDLLOVERRIDES"]
        GameEnvironmentStore.save(context, GameEnvironment.withScopeValue(config, GAME,
            "WINEDLLOVERRIDES", RuntimeRules.withWinHttp(previous)))
    }

    fun setMicrophoneEnabled(context: Context, enabled: Boolean) {
        ensureStopped(context)
        check(!enabled || context.checkSelfPermission(android.Manifest.permission.RECORD_AUDIO) == PackageManager.PERMISSION_GRANTED) {
            "Allow microphone permission, then try again."
        }
        SessionPrefs.setMicEnabled(context, enabled)
    }

    fun openDeveloperSettings(activity: Activity) {
        activity.startActivity(Intent(Settings.ACTION_APPLICATION_DEVELOPMENT_SETTINGS))
    }
}
