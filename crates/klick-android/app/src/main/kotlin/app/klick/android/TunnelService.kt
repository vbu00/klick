package app.klick.android

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.graphics.drawable.Icon
import android.net.ConnectivityManager
import android.net.Network
import android.net.VpnService
import android.os.Build
import android.os.ParcelFileDescriptor
import android.util.Log

/**
 * Туннель kl!ck — аналог расширения VPN на iOS. Работает в своём процессе (`:tunnel`): система держит его,
 * пока VPN включён, с окном и без. Адаптер открывает Android, ядро mihomo (`TunnelCore`) читает его по
 * дескриптору, конфиг — `group/core/config.yaml`, который пишет движок в процессе приложения.
 *
 * Адреса — как у mihomo по умолчанию и у iOS: адаптер `198.18.0.1/30` и `fdfe:dcba:9876::1/126`, DNS —
 * `198.18.0.2` (ядро ловит запросы на любом адресе), подменные адреса сайтов `198.18.0.0/16` идут в
 * туннель. Сам kl!ck — мимо туннеля (`addDisallowedApplication`): иначе соединения ядра к серверам VPN
 * вернулись бы в тот же туннель.
 *
 * Движку о себе служба сообщает рассылкой [ACTION_STATE] внутри приложения: подключилась, перечитала
 * конфиг, не поднялась, остановилась (и выключил ли её человек).
 */
class TunnelService : VpnService() {
    private var tun: ParcelFileDescriptor? = null
    private var running = false
    private var stoppedByCommand = false
    private var networkCallback: ConnectivityManager.NetworkCallback? = null

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        // Android 8+: служба, запущенная startForegroundService, обязана показать уведомление за 5 секунд.
        goForeground()
        when (intent?.action) {
            ACTION_RELOAD -> reload()
            ACTION_STOP -> stopTunnel(byUser = false)
            ACTION_STOP_BY_USER -> stopTunnel(byUser = true)
            // ACTION_START, а ещё «Постоянный VPN» и плитка: система запускает службу с SERVICE_INTERFACE.
            else -> startTunnel()
        }
        return START_NOT_STICKY
    }

    private fun startTunnel() {
        if (running) {
            report("up")
            return
        }
        val config = Dirs.tunnelConfig(this)
        if (!config.isFile) {
            fail("нет конфига: откройте kl!ck и подключитесь оттуда")
            return
        }
        val pfd = try {
            Builder()
                .setSession("kl!ck")
                .setMtu(9000)
                .addAddress("198.18.0.1", 30)
                .addAddress("fdfe:dcba:9876::1", 126)
                .addRoute("0.0.0.0", 0)
                .addRoute("::", 0)
                .addDnsServer("198.18.0.2")
                .addDisallowedApplication(packageName)
                .apply { if (Build.VERSION.SDK_INT >= 29) setMetered(false) }
                .establish()
        } catch (e: Exception) {
            fail("адаптер VPN: ${e.message}")
            return
        } ?: run {
            fail("Android не дал включить VPN: разрешите kl!ck в настройках VPN")
            return
        }
        tun = pfd
        // Дескриптор переходит ядру: закрывает его ядро, когда остановится.
        val err = TunnelCore.start(Dirs.core(this).path, config.path, pfd.detachFd())
        if (err != null) {
            tun = null
            fail(err)
            return
        }
        running = true
        watchNetwork()
        report("up")
    }

    private fun reload() {
        if (!running) {
            startTunnel()
            return
        }
        val err = TunnelCore.reload()
        if (err != null) report("error", error = err) else report("reloaded")
    }

    private fun stopTunnel(byUser: Boolean) {
        stoppedByCommand = true
        shutdown()
        report("down", byUser = byUser)
        stopForeground(STOP_FOREGROUND_REMOVE)
        stopSelf()
    }

    private fun shutdown() {
        networkCallback?.let { (getSystemService(CONNECTIVITY_SERVICE) as ConnectivityManager).unregisterNetworkCallback(it) }
        networkCallback = null
        if (running) TunnelCore.stop()
        running = false
        tun = null
    }

    /** VPN выключили в настройках Android или его место занял другой VPN: движок не включает его снова. */
    override fun onRevoke() {
        stopTunnel(byUser = true)
    }

    override fun onDestroy() {
        if (!stoppedByCommand && running) {
            shutdown()
            report("down", byUser = false)
        }
        super.onDestroy()
    }

    /** Сеть сменилась (Wi-Fi ↔ сотовая): ядро закрывает соединения сразу, а не ждёт тайм-аута QUIC. */
    private fun watchNetwork() {
        val cm = getSystemService(CONNECTIVITY_SERVICE) as ConnectivityManager
        var current: Network? = null
        val callback = object : ConnectivityManager.NetworkCallback() {
            override fun onAvailable(network: Network) {
                if (current != null && current != network) TunnelCore.networkChanged()
                current = network
                setUnderlyingNetworks(arrayOf(network))
            }

            override fun onLost(network: Network) {
                if (network == current) setUnderlyingNetworks(null)
            }
        }
        // kl!ck мимо своего туннеля, поэтому его сеть по умолчанию — настоящая: Wi-Fi или сотовая.
        cm.registerDefaultNetworkCallback(callback)
        networkCallback = callback
    }

    private fun fail(message: String) {
        Log.w(TAG, "туннель не поднялся: $message")
        report("error", error = message)
        stoppedByCommand = true
        stopForeground(STOP_FOREGROUND_REMOVE)
        stopSelf()
    }

    private fun report(state: String, byUser: Boolean = false, error: String? = null) {
        sendBroadcast(
            Intent(ACTION_STATE).setPackage(packageName)
                .putExtra("state", state)
                .putExtra("byUser", byUser)
                .putExtra("error", error),
        )
    }

    private fun goForeground() {
        val nm = getSystemService(NOTIFICATION_SERVICE) as NotificationManager
        nm.createNotificationChannel(NotificationChannel(CHANNEL, "VPN", NotificationManager.IMPORTANCE_LOW).apply { setShowBadge(false) })
        val open = PendingIntent.getActivity(this, 0, Intent(this, MainActivity::class.java), PendingIntent.FLAG_IMMUTABLE)
        val off = PendingIntent.getService(this, 1, Intent(this, TunnelService::class.java).setAction(ACTION_STOP_BY_USER), PendingIntent.FLAG_IMMUTABLE)
        val notification = Notification.Builder(this, CHANNEL)
            .setSmallIcon(R.drawable.ic_stat_klick)
            .setContentTitle("kl!ck")
            .setContentText("VPN включён")
            .setContentIntent(open)
            .setOngoing(true)
            .addAction(Notification.Action.Builder(Icon.createWithResource(this, R.drawable.ic_stat_klick), "Отключить", off).build())
            .build()
        if (Build.VERSION.SDK_INT >= 34) {
            startForeground(NOTIFICATION_ID, notification, ServiceInfo.FOREGROUND_SERVICE_TYPE_SPECIAL_USE)
        } else {
            startForeground(NOTIFICATION_ID, notification)
        }
    }

    companion object {
        private const val TAG = "klick-tunnel"
        private const val CHANNEL = "vpn"
        private const val NOTIFICATION_ID = 1
        const val ACTION_START = "app.klick.android.START"
        const val ACTION_RELOAD = "app.klick.android.RELOAD"
        const val ACTION_STOP = "app.klick.android.STOP"
        const val ACTION_STOP_BY_USER = "app.klick.android.STOP_BY_USER"
        const val ACTION_STATE = "app.klick.android.TUNNEL_STATE"

        fun send(ctx: Context, action: String) {
            ctx.startForegroundService(Intent(ctx, TunnelService::class.java).setAction(action))
        }
    }
}
