package app.klick.android

import android.content.Context
import android.content.Intent
import android.graphics.Bitmap
import androidx.core.graphics.drawable.toBitmap

/**
 * Приложения мимо VPN (банки, Госуслуги, игры с проверкой страны). Своё у Android: на iOS и в движке такого
 * нет — список держит телефон, файлом в `files/`, чтобы его видел и процесс туннеля (`:tunnel`), который
 * передаёт его Android через `addDisallowedApplication`.
 */
object BypassApps {
    class App(val pkg: String, val label: String, val icon: Bitmap?)

    private fun file(ctx: Context) = ctx.filesDir.resolve("bypass-apps.txt")

    fun load(ctx: Context): Set<String> =
        runCatching { file(ctx).readLines().map { it.trim() }.filter { it.isNotEmpty() }.toSet() }.getOrDefault(emptySet())

    fun save(ctx: Context, pkgs: Set<String>) {
        file(ctx).writeText(pkgs.sorted().joinToString("\n"))
    }

    /** Приложения с ярлыком на рабочем столе, кроме kl!ck, по названию. Медленно (значки) — не на главном потоке. */
    fun installed(ctx: Context, iconPx: Int): List<App> {
        val pm = ctx.packageManager
        val launcher = Intent(Intent.ACTION_MAIN).addCategory(Intent.CATEGORY_LAUNCHER)
        @Suppress("DEPRECATION")
        return pm.queryIntentActivities(launcher, 0)
            .map { it.activityInfo.applicationInfo }
            .distinctBy { it.packageName }
            .filter { it.packageName != ctx.packageName }
            .map { ai ->
                App(ai.packageName, ai.loadLabel(pm).toString(), runCatching { ai.loadIcon(pm).toBitmap(iconPx, iconPx) }.getOrNull())
            }
            .sortedBy { it.label.lowercase() }
    }
}
