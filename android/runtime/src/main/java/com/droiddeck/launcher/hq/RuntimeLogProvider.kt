package com.droiddeck.launcher.hq

/** Keeps the runtime's .logs authority separate from Tauri's file provider. */
class RuntimeLogProvider : androidx.core.content.FileProvider()
