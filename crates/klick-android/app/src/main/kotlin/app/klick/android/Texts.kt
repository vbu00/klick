package app.klick.android

import org.json.JSONObject

/** Тексты — как у iOS (`crates/klick-ios/native/Sources/Texts.swift`) и веб-версии (`ui/src/lib/i18n.ts`). */
object Texts {
    fun error(code: String): String = when (code) {
        "service.unreachable" -> "Движок kl!ck ещё не запустился"
        "vpn.no_connection" -> "Сначала добавьте подписку"
        "conn.exists" -> "Эта подписка уже добавлена"
        "conn.not_found" -> "Подключение не найдено — обновите список"
        "sub.no_servers" -> "В подписке нет серверов"
        "sub.refused" -> "Панель отказала: лимит устройств, срок или трафик"
        "sub.unreachable", "sub.download", "sub.fetch_failed" -> "Не удалось скачать подписку"
        "sub.http_status" -> "Панель подписки ответила ошибкой — возможно, ссылка больше не действует"
        "sub.not_subscription" -> "По ссылке открывается страница сайта, а не подписка"
        "sub.unknown_format", "input.unknown_format" -> "Формат не распознан"
        "input.bad_source", "input.empty" -> "Ссылка не подходит"
        "servers.busy" -> "Идёт проверка задержки — попробуйте через пару секунд"
        "core.start_failed" -> "VPN не включился"
        "core.crashed" -> "Ядро падает при запуске"
        "server.not_found" -> "Такого сервера больше нет в подписке — обновите её"
        "conn.secret_missing" -> "Ссылка подписки потерялась — добавьте её заново"
        "storage.write_failed" -> "Не удалось сохранить настройки"
        "latency.network_changed" -> "Пока мерили задержку, сеть сменилась дважды — проверьте ещё раз"
        "service.timeout" -> "kl!ck долго не отвечает — попробуйте ещё раз"
        "service.stopping" -> "Движок kl!ck перезапускается — попробуйте ещё раз"
        else -> "Что-то пошло не так"
    }

    fun notice(code: String, params: JSONObject?): String? {
        val server = params?.optString("server").orEmpty()
        return when (code) {
            "vpn.down" -> "Сервер не отвечает — проверяю снова"
            "vpn.restored" -> if (server.isEmpty()) "Связь восстановлена" else "Связь восстановлена · $server"
            "server.switched" -> "Сервер сменён · $server"
            "sub.expiring" -> "Подписка скоро закончится"
            "sub.expired" -> "Срок подписки закончился"
            "sub.traffic_out" -> "Трафик подписки закончился"
            else -> null
        }
    }

    fun status(vpn: String): String = when (vpn) {
        "connecting" -> "подключаюсь…"
        "connected" -> "подключено"
        "reconnecting" -> "переподключаюсь…"
        "server_down" -> "сервер не отвечает"
        "error" -> "VPN не включился"
        else -> "не подключено"
    }

    fun ping(delay: Int?): String = delay?.let { "$it мс" } ?: "нет ответа"

    fun protocol(kind: String): String = when (kind.lowercase()) {
        "vless" -> "VLESS"
        "vmess" -> "VMess"
        "trojan" -> "Trojan"
        "ss", "shadowsocks" -> "Shadowsocks"
        "hysteria2", "hysteria", "hy2" -> "Hysteria2"
        "tuic" -> "TUIC"
        "wireguard" -> "WireGuard"
        "anytls" -> "AnyTLS"
        else -> kind.uppercase()
    }

    fun noticeIsBad(code: String) = code in setOf("vpn.down", "sub.expired", "sub.traffic_out")

    // Форматы — как ui/src/lib/format.ts и iOS.
    private const val GB = 1_073_741_824.0
    private const val MB = 1_048_576.0

    /** «5.08 Mb/s». */
    fun rate(bytesPerSecond: Double): String {
        val mbit = bytesPerSecond * 8 / 1e6
        return if (mbit >= 1000) "%.2f Gb/s".format(java.util.Locale.US, mbit / 1000) else "%.2f Mb/s".format(java.util.Locale.US, mbit)
    }

    /** «0.6 МБ», «1.25 ГБ». */
    fun bytes(b: Double) = if (b >= GB) "%.2f ГБ".format(java.util.Locale.US, b / GB) else "%.1f МБ".format(java.util.Locale.US, b / MB)

    /** Потрачено без лимита: «393.7 ГБ». */
    fun used(b: Double) = if (b >= GB) "%.1f ГБ".format(java.util.Locale.US, b / GB) else "%.1f МБ".format(java.util.Locale.US, b / MB)

    /** «42.6 из 200 ГБ». */
    fun traffic(used: Double, total: Double): String {
        val t = total / GB
        val tt = if (Math.round(t * 10) % 10 == 0L) Math.round(t).toString() else "%.1f".format(java.util.Locale.US, t)
        return "%.1f из ".format(java.util.Locale.US, used / GB) + tt + " ГБ"
    }

    /** «52.3 Мбит/с», «187 Мбит/с». */
    fun mbps(v: Double) = if (v >= 100) "%.0f Мбит/с".format(java.util.Locale.US, v) else "%.1f Мбит/с".format(java.util.Locale.US, v)

    /** «14 декабря». */
    fun date(unix: Double): String =
        java.text.SimpleDateFormat("d MMMM", java.util.Locale("ru")).format(java.util.Date((unix * 1000).toLong()))

    fun daysLeft(unix: Double) = Math.round((unix - System.currentTimeMillis() / 1000.0) / 86_400).toInt().coerceAtLeast(0)

    /** «3 сервиса», «1 сайт». */
    fun plural(n: Int, one: String, few: String, many: String): String {
        val a = n % 10
        val b = n % 100
        val w = if (a == 1 && b != 11) one else if (a in 2..4 && b !in 12..14) few else many
        return "$n $w"
    }

    /** Ссылка klick://add?url=… из QR со страницы подписки — сама подписка. */
    fun fromDeepLink(text: String): String {
        val uri = runCatching { android.net.Uri.parse(text) }.getOrNull() ?: return text
        return if (uri.scheme == "klick") uri.getQueryParameter("url") ?: text else text
    }

    /** Что вставили в поле «Добавить» (ui/src/lib/source.ts): подписка, одиночная ссылка, список ссылок,
     * конфигурация текстом (`text` — добавляется как файл), пусто или не то. `tone`: dim, accent, red. */
    data class Detect(val kind: String, val title: String, val text: String, val tone: String, val name: String? = null)

    private val linkSchemes = setOf("vless", "vmess", "trojan", "ss", "ssr", "socks", "socks5", "hysteria", "hysteria2", "hy2", "hysteria2+realm", "hy2+realm", "tuic", "wireguard", "wg", "anytls")

    private fun scheme(s: String): String = s.indexOf("://").let { if (it < 0) "" else s.substring(0, it).lowercase() }

    private fun isLink(s: String) = scheme(s) in linkSchemes && '\n' !in s && s.substringBefore('#').none { it.isWhitespace() }

    fun detect(input: String): Detect {
        val s = input.trim()
        if (s.isEmpty()) return Detect("empty", "Ожидаем ссылку", "Скопируйте её у провайдера VPN и вставьте выше — тип определится сам.", "dim")
        val sch = scheme(s)
        val spaced = s.any { it.isWhitespace() }
        if ((sch == "https" || sch == "http") && !spaced) {
            return Detect("sub", "Ссылка на подписку", "Загрузим список серверов, лимит трафика и срок действия. Будет обновляться автоматически.", "accent")
        }
        if (sch == "ssconf" && !spaced) return Detect("sub", "Ключ доступа Outline", "Загрузим сервер Shadowsocks по ключу и будем обновлять его автоматически.", "accent")
        if (isLink(s)) return Detect("link", "Одиночная конфигурация · ${protocol(sch)}", "Прямое соединение с одним сервером. Без лимитов и срока — только адрес и ключ.", "accent")
        val byLine = s.lines().map { it.trim() }.filter { it.isNotEmpty() }
        val lines = if (byLine.size > 1 && byLine.all(::isLink)) byLine else s.split(Regex("\\s+"))
        if (lines.size > 1 && lines.all { scheme(it) in linkSchemes }) {
            return Detect("text", "Список серверов · ${lines.size}", "Добавим одно подключение со всеми этими серверами.", "accent", "Мои серверы")
        }
        if (s.startsWith("{") || s.startsWith("[") || s.lines().any { it.startsWith("[Interface]") || it.startsWith("proxies:") }) {
            return Detect("text", "Конфигурация текстом", "Clash / mihomo, sing-box, Xray или WireGuard — возьмём из неё серверы.", "accent", "Конфигурация")
        }
        return Detect("bad", "Формат не распознан", "Ожидается https://… или vless://, vmess://, trojan://, ss://, hysteria2:// — или конфигурация целиком", "red")
    }

    /** «0:42:05». */
    fun timer(since: Double, nowSec: Double): String {
        if (since <= 0) return "0:00:00"
        val s = (nowSec - since).toLong().coerceAtLeast(0)
        return "%d:%02d:%02d".format(s / 3600, s % 3600 / 60, s % 60)
    }
}
