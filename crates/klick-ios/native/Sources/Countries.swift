import Foundation

// Страна сервера по его имени из подписки и подпись для списка «по странам» — как ui/src/lib/countries.ts.
// Имена придумывает панель: «🇧🇾 BY · HY2», «[NL] Amsterdam», «Германия · Frankfurt», «AWG2».

/// Сервер, разобранный для списка по странам.
struct ParsedServer {
    /// Код страны ISO (`DE`); nil — страну не узнали, сервер пойдёт в «Другие».
    var code: String?
    /// Остаток имени без страны, по-человечески: «Hysteria2 · WARP», «Amsterdam». Пусто — в имени была только страна.
    var title: String
    /// Короткая метка протокола для строки страны: HY2, VLESS, AWG…
    var tag: String
    /// Протокол уже назван в `title` — второй раз его не показываем.
    var named: Bool
}

struct CountryGroup {
    /// Код страны; `""` — «Другие».
    var key: String
    var code: String?
    var name: String
    var items: [(server: Server, p: ParsedServer)]

    /// Метки протоколов без повторов, в порядке подписки.
    var tags: [String] {
        var seen = Set<String>()
        return items.map(\.p.tag).filter { seen.insert($0).inserted }
    }
}

enum Countries {
    private static let ru = Locale(identifier: "ru")
    private static let en = Locale(identifier: "en")

    /// Где имя системы длинное или неудачное.
    private static let ruName = ["US": "США", "GB": "Великобритания", "AE": "ОАЭ", "HK": "Гонконг", "KR": "Южная Корея", "EU": "Евросоюз"]

    /// Двухбуквенные коды, которые в начале имени чаще значат протокол, чем страну.
    private static let notCountry: Set<String> = ["SS"]

    /// Название страны по-русски: «Беларусь», «США».
    static func name(_ code: String?) -> String {
        guard let code else { return "Другие" }
        if let n = ruName[code] { return n }
        guard let n = ru.localizedString(forRegionCode: code) else { return code }
        return n.replacingOccurrences(of: "\\s*\\(.*\\)$", with: "", options: .regularExpression)
    }

    private static let regions = Set(Locale.isoRegionCodes)
    private static func isRegion(_ code: String) -> Bool { !notCountry.contains(code) && regions.contains(code) }

    // MARK: Страна по названию

    /// Страны, которые узнаём по названию (по флагу и коду — любые).
    private static let namedCodes = "RU BY UA KZ UZ KG AM GE AZ MD DE NL FI FR GB US PL SE NO DK IS IE BE LU PT ES IT CH AT CZ SK SI HU RO BG RS HR GR CY MT TR IL AE SA QA IN JP KR CN HK TW SG TH VN MY ID PH AU NZ CA MX BR AR CL ZA EG LV LT EE".split(separator: " ").map(String.init)

    private static let aliases: [(String, String)] = [
        ("США", "US"), ("USA", "US"), ("America", "US"), ("Америка", "US"),
        ("UK", "GB"), ("England", "GB"), ("Britain", "GB"), ("Англия", "GB"), ("Британия", "GB"),
        ("Holland", "NL"), ("Голландия", "NL"), ("Turkey", "TR"), ("Emirates", "AE"), ("Эмираты", "AE"),
        ("Korea", "KR"), ("Корея", "KR"), ("Czech", "CZ"), ("Белоруссия", "BY"),
    ]

    /// Города: по ним узнаём страну, но из подписи не убираем — «Amsterdam» и есть имя сервера.
    private static let cities: [(String, String)] = [
        ("NL", "Amsterdam Амстердам Rotterdam"),
        ("DE", "Frankfurt Франкфурт Berlin Берлин Munich Мюнхен Nuremberg Falkenstein"),
        ("FI", "Helsinki Хельсинки"),
        ("FR", "Paris Париж Gravelines Roubaix"),
        ("GB", "London Лондон Manchester"),
        ("US", "New_York Нью-Йорк Los_Angeles Лос-Анджелес Miami Майами Chicago Dallas Seattle Ashburn"),
        ("TR", "Istanbul Стамбул Ankara"),
        ("JP", "Tokyo Токио Osaka"),
        ("SE", "Stockholm Стокгольм"),
        ("PL", "Warsaw Варшава"),
        ("KZ", "Almaty Алматы Astana Астана"),
        ("LV", "Riga Рига"),
        ("LT", "Vilnius Вильнюс"),
        ("EE", "Tallinn Таллин"),
        ("CH", "Zurich Цюрих Geneva"),
        ("AT", "Vienna Вена"),
        ("CZ", "Prague Прага"),
        ("IT", "Milan Милан Rome"),
        ("ES", "Madrid Мадрид Barcelona"),
        ("CA", "Toronto Торонто Montreal Vancouver"),
        ("AE", "Dubai Дубай"),
        ("RU", "Moscow Москва Мск Петербург Petersburg"),
        ("BY", "Minsk Минск"),
        ("UA", "Kyiv Kiev Киев"),
        ("GE", "Tbilisi Тбилиси"),
        ("AM", "Yerevan Ереван"),
        ("RS", "Belgrade Белград"),
        ("HU", "Budapest Будапешт"),
        ("NO", "Oslo Осло"),
        ("IL", "Tel_Aviv Тель-Авив"),
        ("KR", "Seoul Сеул"),
        ("AU", "Sydney Сидней"),
    ].flatMap { code, list in list.split(separator: " ").map { (String($0).replacingOccurrences(of: "_", with: " "), code) } }

    private static func norm(_ s: String) -> String { s.lowercased().replacingOccurrences(of: "ё", with: "е") }

    private struct Pattern {
        let re: NSRegularExpression
        let code: String
        /// Название страны (убираем из начала подписи), а не город.
        let country: Bool
    }

    /// Русское название ловим по основе с падежным окончанием: «Германия», «в Германии» — но не «Индиго».
    private static func pattern(_ name: String, _ code: String, _ country: Bool) -> Pattern? {
        var n = norm(name.replacingOccurrences(of: "\\s*\\(.*\\)$", with: "", options: .regularExpression))
        let stem = n.range(of: "[а-я]", options: .regularExpression) != nil && n.count > 4
        if stem, let last = n.last, "аяыиьй".contains(last) { n.removeLast() }
        let tail = (stem ? "(?:[аеиоуыьюяй]|ами|ам|ах|ов|ом|ой|ей|ью|ею|ии)?" : "") + "(?!\\p{L})"
        guard let re = try? NSRegularExpression(pattern: "(?:^|[^\\p{L}])(\(NSRegularExpression.escapedPattern(for: n))\(tail))") else { return nil }
        return Pattern(re: re, code: code, country: country)
    }

    private static let patterns: [Pattern] = {
        var seen = Set<String>()
        var out: [(String, String, Bool)] = []
        func add(_ name: String?, _ code: String, _ country: Bool) {
            guard let name, name != code, seen.insert(norm(name)).inserted else { return }
            out.append((name, code, country))
        }
        for code in namedCodes {
            add(ru.localizedString(forRegionCode: code), code, true)
            add(en.localizedString(forRegionCode: code), code, true)
        }
        for (n, code) in aliases { add(n, code, true) }
        for (n, code) in cities { add(n, code, false) }
        // Длинные раньше коротких: «Южная Корея» прежде «Корея»; страны прежде городов.
        out.sort { a, b in a.2 != b.2 ? a.2 : a.0.count > b.0.count }
        return out.compactMap { pattern($0.0, $0.1, $0.2) }
    }()

    private static func byName(_ text: String) -> (code: String, at: Int, len: Int, country: Bool)? {
        let t = norm(text) as NSString
        for p in patterns {
            if let m = p.re.firstMatch(in: t as String, range: NSRange(location: 0, length: t.length)) {
                let r = m.range(at: 1)
                return (p.code, r.location, r.length, p.country)
            }
        }
        return nil
    }

    // MARK: Протокол

    private static let proto: [String: (String, String)] = [
        "hysteria2": ("HY2", "Hysteria2"), "hy2": ("HY2", "Hysteria2"), "hysteria": ("HY", "Hysteria"),
        "vless": ("VLESS", "VLESS"), "vmess": ("VMess", "VMess"), "trojan": ("Trojan", "Trojan"),
        "ss": ("SS", "Shadowsocks"), "shadowsocks": ("SS", "Shadowsocks"), "ssr": ("SSR", "ShadowsocksR"),
        "tuic": ("TUIC", "TUIC"), "wireguard": ("WG", "WireGuard"), "anytls": ("AnyTLS", "AnyTLS"),
        "socks5": ("SOCKS5", "SOCKS5"), "http": ("HTTP", "HTTP"),
    ]

    private static func replace(_ s: String, _ pattern: String, _ template: String, caseless: Bool = true) -> String {
        s.replacingOccurrences(of: pattern, with: template, options: caseless ? [.regularExpression, .caseInsensitive] : .regularExpression)
    }

    /// Сокращения протоколов в имени — полными словами: «HY2» → «Hysteria2», «AWG3.1» → «AmneziaWG 3.1».
    private static func humanize(_ s: String) -> String {
        var s = replace(s, "\\bHY(?:STERIA)?2\\b", "Hysteria2")
        s = replace(s, "\\bA(?:MNEZIA)?WG[ -]?(\\d+(?:\\.\\d+)?)\\b", "AmneziaWG $1")
        s = replace(s, "\\bAWG\\b", "AmneziaWG")
        s = replace(s, "\\bWG\\b", "WireGuard", caseless: false)
        s = replace(s, "\\bVLESS\\b", "VLESS")
        return replace(s, "\\bVMESS\\b", "VMess")
    }

    private static let sep = "^[\\s·•|\\-–—_:,/\\\\]+|[\\s·•|\\-–—_:,/\\\\]+$"
    private static func trim(_ s: String) -> String { s.replacingOccurrences(of: sep, with: "", options: .regularExpression) }

    /// Код в начале имени: «BY · …», «DE-2», «[NL] …», «(US) …».
    private static let lead = try! NSRegularExpression(pattern: "^[\\[(]?([A-Z]{2})[\\])]?(?=$|[\\s·•|\\-–—_:,./\\\\\\d])")

    private static func isFlagLetter(_ u: Unicode.Scalar) -> Bool { (0x1F1E6...0x1F1FF).contains(u.value) }

    /// Флаг-эмодзи в начале названия: код страны и остаток.
    static func splitFlag(_ name: String) -> (code: String?, rest: String) {
        let trimmed = name.drop { $0 == " " }
        let scalars = Array(trimmed.unicodeScalars.prefix(2))
        guard scalars.count == 2, scalars.allSatisfy(isFlagLetter) else { return (nil, name) }
        let code = String(scalars.map { Character(Unicode.Scalar($0.value - 0x1F1E6 + 65)!) })
        let rest = String(String.UnicodeScalarView(trimmed.unicodeScalars.dropFirst(2))).trimmingCharacters(in: .whitespaces)
        return (code, rest)
    }

    /// Разбор одного имени — один раз: экран перерисовывается каждую секунду.
    private static var cache: [String: ParsedServer] = [:]

    static func parse(_ name: String, _ kind: String) -> ParsedServer {
        let key = "\(kind)\n\(name)"
        if let p = cache[key] { return p }
        let p = parseUncached(name, kind)
        cache[key] = p
        return p
    }

    private static func parseUncached(_ name: String, _ kind: String) -> ParsedServer {
        // Флаг — два символа regional indicator: 🇧🇾 = B + Y.
        let flag = name.unicodeScalars.filter(isFlagLetter)
        var code: String? = flag.count >= 2 ? String(flag.prefix(2).map { Character(Unicode.Scalar($0.value - 0x1F1E6 + 65)!) }) : nil
        var rest = trim(String(String.UnicodeScalarView(name.unicodeScalars.filter { !isFlagLetter($0) })))

        let ns = rest as NSString
        if let m = lead.firstMatch(in: rest, range: NSRange(location: 0, length: ns.length)) {
            let c = ns.substring(with: m.range(at: 1))
            if isRegion(c) && (code == nil || code == c) {
                code = c
                rest = trim(ns.substring(from: m.range.length))
            }
        }

        if let found = byName(rest), code == nil || code == found.code {
            code = found.code
            // «Германия · Frankfurt» → «Frankfurt»; город и название в середине имени оставляем.
            if found.country && found.at == 0 { rest = trim((rest as NSString).substring(from: found.len)) }
        }

        var k = kind.lowercased()
        if k.hasSuffix("+realm") { k.removeLast("+realm".count) }
        var (tag, human) = proto[k] ?? (kind.uppercased(), kind)
        if k == "wireguard" && name.range(of: "\\b(?:AWG|amnezia)", options: [.regularExpression, .caseInsensitive]) != nil {
            (tag, human) = ("AWG", "AmneziaWG")
        }
        let title = humanize(rest)
        return ParsedServer(code: code, title: title, tag: tag, named: title.lowercased().contains(human.lowercased()))
    }

    /// Серверы по странам: `pin` — сверху, дальше по алфавиту, «Другие» — в конце. Внутри страны — порядок подписки.
    static func group(_ list: [Server], pin: String?) -> [CountryGroup] {
        var map: [String: CountryGroup] = [:]
        var order: [String] = []
        for s in list {
            let p = parse(s.name, s.kind)
            let key = p.code ?? ""
            if map[key] == nil {
                map[key] = CountryGroup(key: key, code: p.code, name: name(p.code), items: [])
                order.append(key)
            }
            map[key]!.items.append((s, p))
        }
        let ruLocale = Locale(identifier: "ru")
        return order.compactMap { map[$0] }.sorted { a, b in
            if (a.key == pin) != (b.key == pin) { return a.key == pin }
            if (a.key == "") != (b.key == "") { return b.key == "" }
            return a.name.compare(b.name, locale: ruLocale) == .orderedAscending
        }
    }
}
