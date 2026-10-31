package app.klick.android

import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.net.Uri
import android.os.Build
import android.provider.Settings

/**
 * Прошивки, которые сверх Android выключают приложения в фоне (Xiaomi, Huawei, Oppo, Vivo…): у них свой
 * экран «Автозапуск», без него VPN гаснет через несколько минут после выключения экрана. Названия
 * экранов меняются от версии к версии — берём первый, который есть на телефоне.
 */
object Oem {
    class Hint(val title: String, val text: String, val intent: Intent)

    private val screens = listOf(
        "Xiaomi" to listOf(
            "com.miui.securitycenter/com.miui.permcenter.autostart.AutoStartManagementActivity",
            "com.miui.securitycenter/com.miui.powercenter.PowerSettings",
        ),
        "Huawei" to listOf(
            "com.huawei.systemmanager/com.huawei.systemmanager.startupmgr.ui.StartupNormalAppListActivity",
            "com.huawei.systemmanager/com.huawei.systemmanager.optimize.process.ProtectActivity",
        ),
        "Honor" to listOf("com.hihonor.systemmanager/com.hihonor.systemmanager.startupmgr.ui.StartupNormalAppListActivity"),
        "Oppo" to listOf(
            "com.coloros.safecenter/com.coloros.safecenter.permission.startup.StartupAppListActivity",
            "com.coloros.safecenter/com.coloros.safecenter.startupapp.StartupAppListActivity",
            "com.oppo.safe/com.oppo.safe.permission.startup.StartupAppListActivity",
        ),
        "Vivo" to listOf(
            "com.vivo.permissionmanager/com.vivo.permissionmanager.activity.BgStartUpManagerActivity",
            "com.iqoo.secure/com.iqoo.secure.ui.phoneoptimize.AddWhiteListActivity",
        ),
        "OnePlus" to listOf("com.oneplus.security/com.oneplus.security.chainlaunch.view.ChainLaunchAppListActivity"),
        "Asus" to listOf("com.asus.mobilemanager/com.asus.mobilemanager.powersaver.PowerSaverSettings"),
    )

    /** Экран автозапуска этой прошивки или, у Samsung, «Батарея» приложения. null — ничего особого не нужно. */
    fun hint(ctx: Context): Hint? {
        val pm = ctx.packageManager
        for ((brand, names) in screens) {
            for (name in names) {
                val intent = Intent().setComponent(ComponentName.unflattenFromString(name)).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
                @Suppress("DEPRECATION")
                if (pm.resolveActivity(intent, 0) != null) {
                    return Hint("Автозапуск ($brand)", "Включите kl!ck в списке автозапуска — иначе прошивка выключит VPN в фоне", intent)
                }
            }
        }
        if (Build.MANUFACTURER.equals("samsung", ignoreCase = true)) {
            val intent = Intent(Settings.ACTION_APPLICATION_DETAILS_SETTINGS, Uri.parse("package:${ctx.packageName}")).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
            return Hint("Батарея (Samsung)", "«Батарея» → «Без ограничений» — и kl!ck не попадёт в «спящие приложения»", intent)
        }
        return null
    }
}
