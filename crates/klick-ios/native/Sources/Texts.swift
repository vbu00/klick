import Foundation
import SwiftUI

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
        case "core.crashed": return "Ядро падает при запуске"
        case "sub.fetch_failed": return "Не удалось скачать подписку"
        case "sub.http_status": return "Панель подписки ответила ошибкой — возможно, ссылка больше не действует"
        case "sub.not_subscription": return "По ссылке открывается страница сайта, а не подписка"
        case "sub.unknown_format", "input.unknown_format": return "Формат не распознан"
        case "server.not_found": return "Такого сервера больше нет в подписке — обновите её"
        case "conn.secret_missing": return "Ссылка подписки потерялась — добавьте её заново"
        case "conn.file_missing": return "Файл подписки пропал — добавьте её заново"
        case "storage.write_failed": return "Не удалось сохранить настройки"
        case "latency.network_changed": return "Пока мерили задержку, сеть сменилась дважды — проверьте ещё раз"
        case "probe.network_changed": return "Во время проверки поменялась сеть или VPN — запустите её заново"
        case "probe.timeout": return "Проверка не уложилась в полторы минуты — попробуйте ещё раз"
        case "service.timeout": return "kl!ck долго не отвечает — попробуйте ещё раз"
        case "service.stopping": return "Движок kl!ck перезапускается — попробуйте ещё раз"
        case "list.duplicate": return "Уже в списке"
        case "list.bad_index", "list.not_found": return "Правило уже удалено"
        case "input.bad_domain": return "Такого домена не бывает"
        case "input.bad_ip": return "Нужен адрес или подсеть, например 149.154.160.0/20"
        // Неизвестный код — не показываем его человеку как текст: смысла в нём нет.
        default: return "Что-то пошло не так"
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

    // Форматы — как ui/src/lib/format.ts.
    private static let GB = 1_073_741_824.0, MB = 1_048_576.0

    /// «5.08 Mb/s».
    static func rate(_ bytesPerSecond: Double) -> String {
        let mbit = bytesPerSecond * 8 / 1e6
        return mbit >= 1000 ? String(format: "%.2f Gb/s", mbit / 1000) : String(format: "%.2f Mb/s", mbit)
    }

    /// «0.6 МБ», «1.25 ГБ».
    static func bytes(_ b: Double) -> String {
        b >= GB ? String(format: "%.2f ГБ", b / GB) : String(format: "%.1f МБ", b / MB)
    }

    /// Потрачено без лимита: «393.7 ГБ», «512.4 МБ».
    static func used(_ b: Double) -> String {
        b >= GB ? String(format: "%.1f ГБ", b / GB) : String(format: "%.1f МБ", b / MB)
    }

    /// «42.6 из 200 ГБ».
    static func traffic(_ used: Double, _ total: Double) -> String {
        let t = total / GB
        let r = (t * 10).rounded() / 10
        let tt = r == r.rounded() ? String(Int(t.rounded())) : String(format: "%.1f", t)
        return String(format: "%.1f из ", used / GB) + tt + " ГБ"
    }

    /// «52.3 Мбит/с», «187 Мбит/с».
    static func mbps(_ value: Double) -> String {
        value >= 100 ? String(format: "%.0f Мбит/с", value) : String(format: "%.1f Мбит/с", value)
    }

    /// «0:42:05».
    static func timer(since: Double?, now: Date) -> String {
        guard let since, since > 0 else { return "0:00:00" }
        let s = max(0, Int(now.timeIntervalSince1970 - since))
        return String(format: "%d:%02d:%02d", s / 3600, s % 3600 / 60, s % 60)
    }

    /// «14 декабря».
    static func date(_ unix: Double) -> String {
        let f = DateFormatter()
        f.locale = Locale(identifier: "ru_RU")
        f.dateFormat = "d MMMM"
        return f.string(from: Date(timeIntervalSince1970: unix))
    }

    static func daysLeft(_ unix: Double) -> Int {
        max(0, Int(((unix - Date().timeIntervalSince1970) / 86_400).rounded()))
    }

    static func ping(_ delay: Int?) -> String {
        delay.map { "\($0) мс" } ?? "нет ответа"
    }

    /// «3 сервиса», «1 сайт».
    static func plural(_ n: Int, _ forms: (String, String, String)) -> String {
        let a = n % 10, b = n % 100
        let w = a == 1 && b != 11 ? forms.0 : (2...4).contains(a) && !(12...14).contains(b) ? forms.1 : forms.2
        return "\(n) \(w)"
    }

    /// Ссылка klick://add?url=… из QR со страницы подписки — сама подписка.
    static func fromDeepLink(_ text: String) -> String {
        guard let url = URLComponents(string: text), url.scheme == "klick",
              let inner = url.queryItems?.first(where: { $0.name == "url" })?.value else { return text }
        return inner
    }

    /// Что вставили в поле «Добавить» (ui/src/lib/source.ts): подписка, одиночная ссылка, список ссылок или
    /// конфигурация текстом (`text` — добавляется как файл), пусто или не то.
    struct Detect {
        var kind: String
        var title: String
        var text: String
        var color: Color
        var name: String?
    }

    private static let linkSchemes = ["vless", "vmess", "trojan", "ss", "ssr", "socks", "socks5", "hysteria", "hysteria2", "hy2", "hysteria2+realm", "hy2+realm", "tuic", "wireguard", "wg", "anytls"]

    private static func scheme(_ s: String) -> String {
        guard let r = s.range(of: "://") else { return "" }
        return s[..<r.lowerBound].lowercased()
    }

    private static func isLink(_ s: String) -> Bool {
        linkSchemes.contains(scheme(s)) && !s.contains(where: \.isNewline)
            && !(s.split(separator: "#", maxSplits: 1, omittingEmptySubsequences: false).first ?? "").contains(where: \.isWhitespace)
    }

    static func detect(_ input: String) -> Detect {
        let s = input.trimmingCharacters(in: .whitespacesAndNewlines)
        if s.isEmpty { return Detect(kind: "empty", title: "Ожидаем ссылку", text: "Скопируйте её у провайдера VPN и вставьте выше — тип определится сам.", color: K.dim) }
        let sch = scheme(s)
        let spaced = s.contains(where: \.isWhitespace)
        if (sch == "https" || sch == "http") && !spaced {
            return Detect(kind: "sub", title: "Ссылка на подписку", text: "Загрузим список серверов, лимит трафика и срок действия. Будет обновляться автоматически.", color: K.accent)
        }
        if sch == "ssconf" && !spaced {
            return Detect(kind: "sub", title: "Ключ доступа Outline", text: "Загрузим сервер Shadowsocks по ключу и будем обновлять его автоматически.", color: K.accent)
        }
        if isLink(s) {
            return Detect(kind: "link", title: "Одиночная конфигурация · \(protocolName(sch))", text: "Прямое соединение с одним сервером. Без лимитов и срока — только адрес и ключ.", color: K.accent)
        }
        let byLine = s.split(whereSeparator: \.isNewline).map { $0.trimmingCharacters(in: .whitespaces) }.filter { !$0.isEmpty }
        let lines = byLine.count > 1 && byLine.allSatisfy(isLink) ? byLine : s.split(whereSeparator: \.isWhitespace).map(String.init)
        if lines.count > 1 && lines.allSatisfy({ linkSchemes.contains(scheme($0)) }) {
            return Detect(kind: "text", title: "Список серверов · \(lines.count)", text: "Добавим одно подключение со всеми этими серверами.", color: K.accent, name: "Мои серверы")
        }
        if s.hasPrefix("{") || s.hasPrefix("[")
            || s.components(separatedBy: .newlines).contains(where: { $0.hasPrefix("[Interface]") || $0.hasPrefix("proxies:") }) {
            return Detect(kind: "text", title: "Конфигурация текстом", text: "Clash / mihomo, sing-box, Xray или WireGuard — возьмём из неё серверы.", color: K.accent, name: "Конфигурация")
        }
        return Detect(kind: "bad", title: "Формат не распознан", text: "Ожидается https://… или vless://, vmess://, trojan://, ss://, hysteria2:// — или конфигурация целиком", color: K.red)
    }
}
