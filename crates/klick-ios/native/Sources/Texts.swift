import Foundation

/// Тексты и форматирование — как в веб-версии kl!ck (ui/src/lib/i18n.ts, format.ts), только нужное здесь.
enum Texts {
    static func error(_ code: String) -> String {
        switch code {
        case "service.unreachable": return "Движок kl!ck ещё не запустился"
        case "vpn.no_connection": return "Сначала добавьте подписку"
        case "conn.exists": return "Эта подписка уже добавлена"
        case "conn.not_found": return "Подключение не найдено — обновите список"
        case "sub.no_servers": return "В подписке нет серверов"
        case "sub.refused": return "Панель отказала: лимит устройств, срок или трафик"
        case "sub.unreachable", "sub.download": return "Не удалось скачать подписку"
        case "input.bad_source", "input.empty": return "Ссылка не подходит"
        case "speed.vpn_off": return "Включите VPN — скорость меряется через сервер"
        case "speed.busy": return "Скорость уже меряется"
        case "speed.failed": return "Через сервер ничего не скачалось"
        case "servers.busy": return "Идёт проверка задержки — попробуйте через пару секунд"
        case "core.start_failed": return "Ядро VPN не запустилось"
        default: return "Ошибка: \(code)"
        }
    }

    static func notice(_ code: String, _ params: [String: Any]) -> String? {
        let server = params["server"] as? String ?? ""
        switch code {
        case "vpn.down": return "Сервер не отвечает — проверяю снова"
        case "vpn.restored": return server.isEmpty ? "Связь восстановлена" : "Связь восстановлена · \(server)"
        case "server.switched": return "Сервер сменён · \(server)"
        case "sub.expiring": return "Подписка скоро закончится"
        case "sub.expired": return "Срок подписки закончился"
        case "sub.traffic_out": return "Трафик подписки закончился"
        default: return nil
        }
    }

    static func noticeIsBad(_ code: String) -> Bool {
        ["vpn.down", "sub.expired", "sub.traffic_out"].contains(code)
    }

    static func status(_ vpn: String?) -> String {
        switch vpn {
        case "connecting": return "подключаюсь…"
        case "connected": return "подключено"
        case "reconnecting": return "переподключаюсь…"
        case "server_down": return "сервер не отвечает"
        case "error": return "VPN не включился"
        default: return "не подключено"
        }
    }

    static func protocolName(_ kind: String) -> String {
        switch kind.lowercased() {
        case "vless": return "VLESS"
        case "vmess": return "VMess"
        case "trojan": return "Trojan"
        case "ss", "shadowsocks": return "Shadowsocks"
        case "hysteria2", "hysteria", "hy2": return "Hysteria2"
        case "tuic": return "TUIC"
        case "wireguard": return "WireGuard"
        case "anytls": return "AnyTLS"
        default: return kind.uppercased()
        }
    }

    static func gb(_ bytes: Double) -> String {
        bytes >= 1_073_741_824 ? String(format: "%.1f ГБ", bytes / 1_073_741_824) : String(format: "%.0f МБ", bytes / 1_048_576)
    }

    static func rate(_ bytesPerSecond: Double) -> String {
        let mbit = bytesPerSecond * 8 / 1_000_000
        return mbit >= 1 ? String(format: "%.1f Мбит/с", mbit) : String(format: "%.0f Кбит/с", mbit * 1000)
    }

    static func mbps(_ value: Double) -> String {
        value >= 100 ? String(format: "%.0f Мбит/с", value) : String(format: "%.1f Мбит/с", value)
    }

    static func date(_ unix: Double) -> String {
        let f = DateFormatter()
        f.locale = Locale(identifier: "ru_RU")
        f.dateFormat = "d MMMM"
        return f.string(from: Date(timeIntervalSince1970: unix))
    }

    static func daysLeft(_ unix: Double) -> Int {
        max(0, Int(ceil((unix - Date().timeIntervalSince1970) / 86_400)))
    }

    /// Ссылка klick://add?url=… из QR со страницы подписки — сама подписка.
    static func fromDeepLink(_ text: String) -> String {
        guard let url = URLComponents(string: text), url.scheme == "klick",
              let inner = url.queryItems?.first(where: { $0.name == "url" })?.value else { return text }
        return inner
    }

    /// Что вставили: подписка (`sub`), одиночная ссылка (`link`), пусто или не то.
    static func kind(_ input: String) -> String {
        let s = input.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !s.isEmpty else { return "empty" }
        let scheme = s.split(separator: ":").first.map { $0.lowercased() } ?? ""
        if (scheme == "https" || scheme == "ssconf"), !s.contains(" ") { return "sub" }
        let links = ["vless", "vmess", "trojan", "ss", "ssr", "socks", "socks5", "hysteria", "hysteria2", "hy2", "tuic", "wireguard", "wg", "anytls"]
        if links.contains(scheme), s.contains("://") { return "link" }
        return "bad"
    }
}
