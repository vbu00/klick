package app.klick.android

import android.app.Application
import android.os.Build
import android.util.Log
import java.io.File
import kotlin.concurrent.thread

/**
 * Процесс приложения поднимает движок; процесс туннеля (`:tunnel`) — только ядро в [TunnelService],
 * движок там не нужен: два движка — два писателя одних настроек.
 */
class KlickApp : Application() {
    override fun onCreate() {
        super.onCreate()
        if (processName().endsWith(":tunnel")) return
        Bridge.init(this)
        // Не на главном потоке: распаковка ресурсов и запуск движка (а он ждёт ответов Android).
        thread(name = "klick-start") {
            val err = runCatching {
                Dirs.unpackResources(this)
                Engine.nativeStart(Dirs.data(this).path, Dirs.group(this).path, Dirs.resources(this).path)
            }.getOrElse { it.message ?: it.toString() }
            if (err != null) {
                Log.e("klick", "движок не запустился: $err")
                startError = err
            } else {
                Engine.nativeSubscribe()
                ready = true
            }
        }
    }

    private fun processName(): String =
        if (Build.VERSION.SDK_INT >= 28) getProcessName()
        else runCatching { File("/proc/self/cmdline").readText().trim('\u0000') }.getOrDefault("")

    companion object {
        @Volatile var ready = false
        @Volatile var startError: String? = null
    }
}
