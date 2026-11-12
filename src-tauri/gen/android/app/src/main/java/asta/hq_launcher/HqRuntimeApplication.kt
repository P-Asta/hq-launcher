package asta.hq_launcher

import android.app.Application
import android.content.Context
import android.content.res.Configuration
import com.droiddeck.launcher.core.AppLanguage
import com.droiddeck.launcher.hq.HqRuntimeBridge

class HqRuntimeApplication : Application() {
    override fun attachBaseContext(base: Context) = super.attachBaseContext(AppLanguage.wrap(base))
    override fun onConfigurationChanged(newConfig: Configuration) {
        super.onConfigurationChanged(newConfig)
        AppLanguage.systemChanged(newConfig)
    }
    override fun onCreate() {
        super.onCreate()
        HqRuntimeBridge.initialize(this)
    }
}
