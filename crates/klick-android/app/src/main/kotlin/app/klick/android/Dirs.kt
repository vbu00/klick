package app.klick.android

import android.content.Context
import java.io.File

/**
 * Папки kl!ck — как на iOS: настройки, ключи и журнал движка (`klick/`), папка, общая с процессом
 * туннеля (`group/`, в ней `core/` — конфиг, серверы и наборы ядра), наборы и база стран из APK
 * (`resources/`). Оба процесса — одно приложение, поэтому папки у них общие без App Group.
 */
object Dirs {
    fun data(ctx: Context) = File(ctx.filesDir, "klick")
    fun group(ctx: Context) = File(ctx.filesDir, "group")
    fun core(ctx: Context) = File(group(ctx), "core")
    fun resources(ctx: Context) = File(ctx.filesDir, "resources")

    /** Конфиг ядра туннеля (`klick_service::ios::TUNNEL_CONFIG`). */
    fun tunnelConfig(ctx: Context) = File(core(ctx), "config.yaml")

    /**
     * Наборы, каталог сервисов и база стран лежат в APK (`assets/klick-resources`); движку нужны файлы.
     * Распаковываются, когда сменилась версия приложения.
     */
    fun unpackResources(ctx: Context) {
        val dir = resources(ctx)
        val stamp = File(dir, ".version")
        val version = "${BuildConfig.VERSION_NAME}+${BuildConfig.VERSION_CODE}"
        if (stamp.isFile && stamp.readText() == version) return
        dir.deleteRecursively()
        copyAssets(ctx, "klick-resources", dir)
        stamp.writeText(version)
    }

    private fun copyAssets(ctx: Context, path: String, to: File) {
        val children = ctx.assets.list(path).orEmpty()
        if (children.isEmpty()) {
            to.parentFile?.mkdirs()
            ctx.assets.open(path).use { input -> to.outputStream().use { input.copyTo(it) } }
            return
        }
        to.mkdirs()
        for (child in children) copyAssets(ctx, "$path/$child", File(to, child))
    }
}
