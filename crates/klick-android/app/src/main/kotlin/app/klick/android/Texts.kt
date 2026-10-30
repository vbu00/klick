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

    /** «0:42:05». */
    fun timer(since: Double, nowSec: Double): String {
        if (since <= 0) return "0:00:00"
        val s = (nowSec - since).toLong().coerceAtLeast(0)
        return "%d:%02d:%02d".format(s / 3600, s % 3600 / 60, s % 60)
    }
}
