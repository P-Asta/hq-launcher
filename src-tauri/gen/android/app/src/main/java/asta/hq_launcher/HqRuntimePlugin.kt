package asta.hq_launcher

import android.app.Activity
import android.Manifest
import android.content.pm.PackageManager
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.Permission
import app.tauri.annotation.PermissionCallback
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import com.droiddeck.launcher.hq.HqRuntimeBridge
import org.json.JSONObject

@InvokeArg
class MicrophoneArgs { var enabled: Boolean = false }

@InvokeArg
class GameLaunchArgs { var depotManifest: String = "" }

@InvokeArg
class GameVersionArgs { var depotManifest: String = ""; var gameVersion: Int = -1 }

@TauriPlugin(permissions = [Permission(strings = [Manifest.permission.RECORD_AUDIO], alias = "microphone")])
class HqRuntimePlugin(private val activity: Activity) : Plugin(activity) {
    private fun call(invoke: Invoke, block: () -> JSONObject?) {
        try {
            val value = block()
            if (value == null) invoke.resolve() else invoke.resolve(JSObject().apply {
                value.keys().forEach { key -> put(key, value.get(key)) }
            })
        } catch (error: Exception) {
            invoke.reject(error.message ?: "Internal runtime request failed.")
        }
    }

    @Command fun status(invoke: Invoke) = call(invoke) { HqRuntimeBridge.status(activity) }
    @Command fun gamePaths(invoke: Invoke) = call(invoke) { HqRuntimeBridge.gamePaths(activity) }
    @Command fun ensureStopped(invoke: Invoke) = call(invoke) { HqRuntimeBridge.ensureStopped(activity); null }
    @Command fun configureBepinex(invoke: Invoke) = call(invoke) { HqRuntimeBridge.configureBepinex(activity); null }
    @Command fun installRuntime(invoke: Invoke) = call(invoke) { HqRuntimeBridge.installRuntime(activity); null }
    @Command fun stopSession(invoke: Invoke) = call(invoke) { HqRuntimeBridge.stopSession(activity); null }
    @Command fun cacheGameVersion(invoke: Invoke) = call(invoke) {
        val args = invoke.parseArgs(GameVersionArgs::class.java)
        HqRuntimeBridge.cacheGameVersion(activity, args.depotManifest, args.gameVersion); null
    }
    @Command fun versions(invoke: Invoke) = call(invoke) { HqRuntimeBridge.versions(activity) }
    @Command fun selectVersion(invoke: Invoke) = call(invoke) {
        val args = invoke.parseArgs(GameVersionArgs::class.java)
        HqRuntimeBridge.selectVersion(activity, args.gameVersion, args.depotManifest); null
    }
    @Command fun installVersion(invoke: Invoke) = activity.runOnUiThread {
        call(invoke) {
            val args = invoke.parseArgs(GameVersionArgs::class.java)
            HqRuntimeBridge.installVersion(activity, args.gameVersion, args.depotManifest); null
        }
    }
    @Command fun openSteam(invoke: Invoke) = activity.runOnUiThread {
        call(invoke) { HqRuntimeBridge.openSteam(activity); null }
    }
    @Command fun launchGame(invoke: Invoke) = activity.runOnUiThread {
        call(invoke) { HqRuntimeBridge.launchGame(activity, invoke.parseArgs(GameLaunchArgs::class.java).depotManifest); null }
    }
    @Command fun openDeveloperSettings(invoke: Invoke) = activity.runOnUiThread {
        call(invoke) { HqRuntimeBridge.openDeveloperSettings(activity); null }
    }
    @Command fun setMicrophone(invoke: Invoke) = activity.runOnUiThread {
        try {
            HqRuntimeBridge.ensureStopped(activity)
            val enabled = invoke.parseArgs(MicrophoneArgs::class.java).enabled
            if (enabled && !microphoneGranted()) {
                requestPermissionForAlias("microphone", invoke, "microphonePermissionResult")
            } else {
                call(invoke) { HqRuntimeBridge.setMicrophoneEnabled(activity, enabled); null }
            }
        } catch (error: Exception) { invoke.reject(error.message ?: "Could not change microphone settings.") }
    }

    private fun microphoneGranted() = activity.checkSelfPermission(Manifest.permission.RECORD_AUDIO) == PackageManager.PERMISSION_GRANTED

    @PermissionCallback
    fun microphonePermissionResult(invoke: Invoke) = call(invoke) {
        if (!microphoneGranted()) {
            HqRuntimeBridge.setMicrophoneEnabled(activity, false)
            error("Microphone permission was denied. Allow microphone access in Android app permissions, then try again.")
        }
        HqRuntimeBridge.setMicrophoneEnabled(activity, true)
        null
    }
}
