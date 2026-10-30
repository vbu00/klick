package app.klick.android

import android.annotation.SuppressLint
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.net.ConnectivityManager
import android.net.NetworkCapabilities
import android.net.VpnService
import android.provider.Settings
import androidx.core.content.ContextCompat
import java.util.concurrent.LinkedBlockingQueue
import java.util.concurrent.TimeUnit

/**
 * Что движку (Rust, `crates/klick-android/src/host.rs`) даёт Android: туннель и ID устройства. Методы
 * зовутся из потоков движка и ждут ответа службы туннеля — с главного потока их не звать.
 * Ошибка — строкой, null — получилось.
 */
@SuppressLint("StaticFieldLeak") // контекст приложения, а не окна
object Bridge {
    private lateinit var app: Context

    /** Ответы службы туннеля на команду, которую ждёт движок. */
    private val replies = LinkedBlockingQueue<Intent>()

    fun init(ctx: Context) {
        app = ctx.applicationContext
        val receiver = object : BroadcastReceiver() {
            override fun onReceive(context: Context, intent: Intent) {
                val state = intent.getStringExtra("state") ?: return
                replies.offer(intent)
                when (state) {
                    "up" -> Engine.nativeTunnelChanged(true, false)
                    "down" -> Engine.nativeTunnelChanged(false, intent.getBooleanExtra("byUser", false))
                }
            }
        }
        ContextCompat.registerReceiver(app, receiver, IntentFilter(TunnelService.ACTION_STATE), ContextCompat.RECEIVER_NOT_EXPORTED)
    }

    /** Отправить команду службе и дождаться одного из ответов [wanted]. */
    private fun command(action: String, wanted: Set<String>, timeoutSec: Long): String? {
        replies.clear()
        TunnelService.send(app, action)
        val deadline = System.nanoTime() + TimeUnit.SECONDS.toNanos(timeoutSec)
        while (true) {
            val left = deadline - System.nanoTime()
            if (left <= 0) return "служба VPN не ответила за $timeoutSec с"
            val reply = replies.poll(left, TimeUnit.NANOSECONDS) ?: continue
            val state = reply.getStringExtra("state")
            if (state == "error") return reply.getStringExtra("error") ?: "туннель не поднялся"
            if (state in wanted) return null
        }
    }

    @JvmStatic
    fun tunnelStart(): String? {
        // Разрешение на VPN спрашивает окно перед «Подключить»: без него Android не даст открыть адаптер.
        if (VpnService.prepare(app) != null) return "Разрешите kl!ck включать VPN"
        return command(TunnelService.ACTION_START, setOf("up"), 25)
    }

    @JvmStatic
    fun tunnelReload(): String? = command(TunnelService.ACTION_RELOAD, setOf("reloaded", "up"), 25)

    @JvmStatic
    fun tunnelStop(): String? = command(TunnelService.ACTION_STOP, setOf("down"), 10)

    /** Работает ли VPN kl!ck: в системе есть сеть VPN, и разрешение VPN — у kl!ck. */
    @JvmStatic
    fun tunnelConnected(): Boolean {
        if (VpnService.prepare(app) != null) return false
        val cm = app.getSystemService(Context.CONNECTIVITY_SERVICE) as ConnectivityManager
        @Suppress("DEPRECATION")
        return cm.allNetworks.any { cm.getNetworkCapabilities(it)?.hasTransport(NetworkCapabilities.TRANSPORT_VPN) == true } &&
            Dirs.core(app).resolve("config.applied").isFile
    }

    /** ANDROID_ID: постоянный, пока kl!ck стоит на телефоне (свой у каждого приложения и пользователя). */
    @SuppressLint("HardwareIds")
    @JvmStatic
    fun deviceId(): String? = Settings.Secure.getString(app.contentResolver, Settings.Secure.ANDROID_ID)
}
