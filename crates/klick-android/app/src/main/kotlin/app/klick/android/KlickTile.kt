package app.klick.android

import android.app.PendingIntent
import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.net.VpnService
import android.os.Build
import android.service.quicksettings.Tile
import android.service.quicksettings.TileService
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch

/**
 * Плитка kl!ck в шторке: включить и выключить VPN, не открывая приложение. Выключает служба туннеля сама
 * (как кнопка «Отключить» в уведомлении), включает движок — он в процессе приложения, Android поднимет его
 * вместе с плиткой. Нет разрешения на VPN — открывается kl!ck: спросить его можно только из окна.
 */
class KlickTile : TileService() {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main)

    override fun onStartListening() {
        refresh()
    }

    override fun onClick() {
        if (Bridge.tunnelConnected()) {
            TunnelService.send(this, TunnelService.ACTION_STOP_BY_USER)
            show(on = false, busy = false)
            return
        }
        if (VpnService.prepare(this) != null) {
            openApp()
            return
        }
        show(on = false, busy = true)
        scope.launch {
            // Движок поднимается вместе с процессом приложения — подождать его.
            repeat(100) { if (!KlickApp.ready) delay(100) }
            runCatching { Engine.call("connect") }
            refresh()
        }
    }

    private fun refresh() = show(Bridge.tunnelConnected(), busy = false)

    private fun show(on: Boolean, busy: Boolean) {
        val tile = qsTile ?: return
        tile.state = if (on) Tile.STATE_ACTIVE else Tile.STATE_INACTIVE
        tile.label = "kl!ck"
        if (Build.VERSION.SDK_INT >= 29) tile.subtitle = if (busy) "подключаю…" else if (on) "подключено" else "выключен"
        tile.updateTile()
    }

    private fun openApp() {
        val intent = Intent(this, MainActivity::class.java).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
        if (Build.VERSION.SDK_INT >= 34) {
            startActivityAndCollapse(PendingIntent.getActivity(this, 0, intent, PendingIntent.FLAG_IMMUTABLE))
        } else {
            @Suppress("DEPRECATION", "StartActivityAndCollapseDeprecated")
            startActivityAndCollapse(intent)
        }
    }

    companion object {
        /** VPN включился или выключился — плитка перерисуется, даже если шторка уже открыта. */
        fun update(ctx: Context) {
            runCatching { requestListeningState(ctx, ComponentName(ctx, KlickTile::class.java)) }
        }
    }
}
